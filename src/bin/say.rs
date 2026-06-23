//! `say` — macOS-style text-to-speech for Windows.
//!
//! Uses the Windows Runtime `Windows.Media.SpeechSynthesis` engine so that the
//! natural ("OneCore") voices available on Windows 10/11 are used when present.
//! The synthesized audio is a standard PCM WAV, played back through the classic
//! wave-out API. When playback is not possible (e.g. no audio device in the
//! session) `--output FILE` writes the WAV instead.
//!
//! On non-Windows targets the binary compiles but prints an error, matching the
//! behaviour of the other commands in this crate.

use std::env;
use std::ffi::OsString;
use std::io;
use std::process::exit;

/// Voice metadata used for `--list-voices` output and for selecting a voice.
/// Kept free of WinRT types so the selection logic can be unit-tested.
#[derive(Debug, Clone, Default)]
struct VoiceFields {
    display: String,
    id: String,
    language: String,
    description: String,
}

#[derive(Debug, Default)]
struct Options {
    list_voices: bool,
    voice: Option<OsString>,
    voice_id: Option<OsString>,
    rate: Option<i32>,
    volume: Option<u32>,
    file: Option<OsString>,
    output: Option<OsString>,
    text: Vec<OsString>,
}

fn main() {
    let options = match parse_args(env::args_os().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("say: {message}");
            print_usage();
            exit(2);
        }
    };

    // Argument parsing is cross-platform so usage errors are reported before we
    // touch stdin; the platform-specific work lives in `run` so unsupported
    // targets bail out instead of (e.g.) blocking on a terminal read.
    run(options);
}

#[cfg(windows)]
fn run(options: Options) {
    // macOS `say` uses words-per-minute; SAPI/WinRT use an engine-specific rate.
    // We expose a signed -10..10 scale (0 = normal) and map it per engine.
    let rate = options.rate.map(clamp_rate).unwrap_or(0);
    let volume = options.volume.map(clamp_volume).unwrap_or(100);

    if options.list_voices {
        if let Err(error) = engine::list_voices() {
            eprintln!("say: {error}");
            exit(1);
        }
        exit(0);
    }

    let text = match gather_text(&options) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
            eprintln!("say: {error}");
            print_usage();
            exit(2);
        }
        Err(error) => {
            eprintln!("say: {error}");
            exit(1);
        }
    };
    if text.trim().is_empty() {
        exit(0);
    }

    match engine::speak(
        &text,
        options.voice_id.as_deref(),
        options.voice.as_deref(),
        rate,
        volume,
        options.output.as_deref(),
    ) {
        Ok(()) => {}
        Err(error) => {
            eprintln!("say: {error}");
            exit(1);
        }
    }
}

#[cfg(not(windows))]
fn run(_options: Options) {
    eprintln!("say: this command is only supported on Windows");
    exit(1);
}

fn gather_text(options: &Options) -> io::Result<String> {
    if let Some(path) = &options.file {
        let bytes = std::fs::read(path)
            .map_err(|e| io::Error::new(e.kind(), format!("cannot read {}: {e}", path.to_string_lossy())))?;
        return Ok(decode_bytes(&bytes));
    }

    if !options.text.is_empty() {
        let joined = options
            .text
            .iter()
            .map(|part| part.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        return Ok(joined);
    }

    // No explicit text: read from stdin, but only when it is redirected.
    // Reading a console interactively would block forever.
    #[cfg(windows)]
    if !stdin_redirected() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "no text given; pass arguments, a file with -f, or pipe input on stdin",
        ));
    }

    let mut bytes = Vec::new();
    io::Read::read_to_end(&mut io::stdin(), &mut bytes)?;
    Ok(decode_bytes(&bytes))
}

/// Clamp the user-supplied rate into the documented -10..10 range.
fn clamp_rate(rate: i32) -> i32 {
    rate.clamp(-10, 10)
}

/// Clamp the user-supplied volume into the documented 0..100 range.
fn clamp_volume(volume: u32) -> u32 {
    volume.min(100)
}

fn parse_args<I>(args: I) -> Result<Options, String>
where
    I: IntoIterator<Item = OsString>,
{
    let mut options = Options::default();
    let mut parse_flags = true;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        if parse_flags && arg == "--" {
            parse_flags = false;
            continue;
        }

        if parse_flags {
            if let Some(text) = arg.to_str() {
                if let Some(rest) = text.strip_prefix("--voice=") {
                    options.voice = Some(OsString::from(rest));
                    continue;
                }
                if let Some(rest) = text.strip_prefix("--voice-id=") {
                    options.voice_id = Some(OsString::from(rest));
                    continue;
                }
                if let Some(rest) = text.strip_prefix("--rate=") {
                    options.rate = Some(parse_i32(rest, "rate")?);
                    continue;
                }
                if let Some(rest) = text.strip_prefix("--volume=") {
                    options.volume = Some(parse_u32(rest, "volume")?);
                    continue;
                }
                if let Some(rest) = text.strip_prefix("--file=") {
                    options.file = Some(OsString::from(rest));
                    continue;
                }
                if let Some(rest) = text.strip_prefix("--output=") {
                    options.output = Some(OsString::from(rest));
                    continue;
                }

                match text {
                    "-l" | "--list-voices" => {
                        options.list_voices = true;
                        continue;
                    }
                    "-v" | "--voice" => {
                        options.voice = Some(take_value(&mut iter, "--voice")?);
                        continue;
                    }
                    "--voice-id" => {
                        options.voice_id = Some(take_value(&mut iter, "--voice-id")?);
                        continue;
                    }
                    "-r" | "--rate" => {
                        let value = take_value(&mut iter, "--rate")?;
                        options.rate = Some(parse_i32_os(&value, "rate")?);
                        continue;
                    }
                    "--volume" => {
                        let value = take_value(&mut iter, "--volume")?;
                        options.volume = Some(parse_u32_os(&value, "volume")?);
                        continue;
                    }
                    "-f" | "--file" => {
                        options.file = Some(take_value(&mut iter, "--file")?);
                        continue;
                    }
                    "-o" | "--output" => {
                        options.output = Some(take_value(&mut iter, "--output")?);
                        continue;
                    }
                    "-h" | "--help" => return Err("help requested".to_string()),
                    other if other.starts_with('-') && other != "-" => {
                        return Err(format!("unknown option: {other}"));
                    }
                    _ => {}
                }
            }
        }

        options.text.push(arg);
    }

    Ok(options)
}

fn take_value<I>(iter: &mut I, name: &str) -> Result<OsString, String>
where
    I: Iterator<Item = OsString>,
{
    iter.next()
        .ok_or_else(|| format!("option {name} requires an argument"))
}

fn parse_i32_os(value: &OsString, name: &str) -> Result<i32, String> {
    parse_i32(&value.to_string_lossy(), name)
}

fn parse_u32_os(value: &OsString, name: &str) -> Result<u32, String> {
    parse_u32(&value.to_string_lossy(), name)
}

fn parse_i32(text: &str, name: &str) -> Result<i32, String> {
    text.parse::<i32>()
        .map_err(|_| format!("option {name} expects an integer, got {text:?}"))
}

fn parse_u32(text: &str, name: &str) -> Result<u32, String> {
    text.parse::<u32>()
        .map_err(|_| format!("option {name} expects a non-negative integer, got {text:?}"))
}

fn print_usage() {
    eprintln!("usage: say [-v voice] [-r rate] [--volume n] [-f file | -o out.wav] [text ...]");
    eprintln!("       say --list-voices");
    eprintln!("       with no text, reads from a redirected standard input");
}

/// Decode raw bytes to text, honouring a UTF-8 / UTF-16LE / UTF-16BE BOM and
/// falling back to lossy UTF-8 otherwise.
fn decode_bytes(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(&bytes[3..]).into_owned()
    } else if bytes.starts_with(&[0xFF, 0xFE]) {
        utf16_from(bytes[2..].chunks_exact(2), |c| u16::from_le_bytes([c[0], c[1]]))
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        utf16_from(bytes[2..].chunks_exact(2), |c| u16::from_be_bytes([c[0], c[1]]))
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

fn utf16_from<'a, F>(chunks: std::slice::ChunksExact<'a, u8>, endian: F) -> String
where
    F: Fn(&[u8]) -> u16,
{
    let units: Vec<u16> = chunks.map(endian).collect();
    String::from_utf16_lossy(&units)
}

/// Map the documented -10..10 rate onto the WinRT `SpeakingRate` double
/// (1.0 = normal tempo).
fn map_rate(rate: i32) -> f64 {
    let scaled = 1.0 + rate as f64 * 0.2;
    scaled.clamp(0.5, 6.0)
}

/// Pick a voice index. `--voice-id` matches the Id exactly; `-v` matches any
/// field by case-insensitive substring; `None` means "use the default voice".
fn select_index(
    voices: &[VoiceFields],
    voice_id: Option<&str>,
    voice: Option<&str>,
) -> Option<usize> {
    if let Some(query) = voice_id {
        return voices
            .iter()
            .position(|v| v.id.eq_ignore_ascii_case(query));
    }

    if let Some(query) = voice {
        let needle = query.to_ascii_lowercase();
        if needle.is_empty() {
            return None;
        }
        return voices.iter().position(|v| {
            v.display.to_ascii_lowercase().contains(&needle)
                || v.id.to_ascii_lowercase().contains(&needle)
                || v.language.to_ascii_lowercase().contains(&needle)
                || v.description.to_ascii_lowercase().contains(&needle)
        });
    }

    None
}

#[cfg(windows)]
mod engine {
    use super::{map_rate, select_index, VoiceFields};
    use std::ffi::OsStr;
    use std::io;
    use windows::core::HSTRING;
    use windows::Media::SpeechSynthesis::{SpeechSynthesizer, VoiceGender};
    use windows::Storage::Streams::DataReader;
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

    #[link(name = "winmm")]
    extern "system" {
        fn PlaySoundW(psz: *const u16, hmod: *const core::ffi::c_void, fdw_sound: u32) -> i32;
        fn waveOutGetNumDevs() -> u32;
    }

    // SND_MEMORY: play a waveform held in memory; default (no SND_ASYNC) is synchronous.
    const SND_MEMORY: u32 = 0x0000_0004;

    pub fn list_voices() -> io::Result<()> {
        ensure_initialized()?;
        let voices = SpeechSynthesizer::AllVoices().map_err(win_err)?;
        let count = voices.Size().map_err(win_err)?;

        if count == 0 {
            eprintln!("(no voices are installed)");
        }

        for index in 0..count {
            let voice = voices.GetAt(index).map_err(win_err)?;
            let display = voice.DisplayName().map(hstring_to_string).unwrap_or_default();
            let language = voice.Language().map(hstring_to_string).unwrap_or_default();
            let gender = voice.Gender().map(gender_label).unwrap_or_default();
            let id = voice.Id().map(hstring_to_string).unwrap_or_default();
            println!("{display}\t{language}\t{gender}\t{id}");
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn speak(
        text: &str,
        voice_id: Option<&OsStr>,
        voice: Option<&OsStr>,
        rate: i32,
        volume: u32,
        output: Option<&OsStr>,
    ) -> io::Result<()> {
        ensure_initialized()?;
        let synth = SpeechSynthesizer::new().map_err(win_err)?;

        let voice_id_str = voice_id.map(|s| s.to_string_lossy().into_owned());
        let voice_str = voice.map(|s| s.to_string_lossy().into_owned());
        let selector_given = voice_id_str.is_some() || voice_str.is_some();
        match select_index(
            &collect_fields()?,
            voice_id_str.as_deref(),
            voice_str.as_deref(),
        ) {
            Some(index) => {
                let voices = SpeechSynthesizer::AllVoices().map_err(win_err)?;
                let chosen = voices.GetAt(index as u32).map_err(win_err)?;
                synth.SetVoice(&chosen).map_err(win_err)?;
            }
            None if selector_given => {
                return Err(io::Error::other(
                    "the requested voice was not found; run `say --list-voices` to list voices",
                ));
            }
            None => {}
        }

        let options = synth.Options().map_err(win_err)?;
        options.SetSpeakingRate(map_rate(rate)).map_err(win_err)?;
        options
            .SetAudioVolume(volume as f64 / 100.0)
            .map_err(win_err)?;

        let handle = HSTRING::from(text);
        let stream = synth
            .SynthesizeTextToStreamAsync(&handle)
            .map_err(win_err)?
            .join()
            .map_err(win_err)?;

        let size = stream.Size().map_err(win_err)? as u32;
        let reader = DataReader::CreateDataReader(&stream).map_err(win_err)?;
        reader.LoadAsync(size).map_err(win_err)?.join().map_err(win_err)?;
        let mut bytes = vec![0u8; size as usize];
        reader.ReadBytes(&mut bytes).map_err(win_err)?;

        if let Some(path) = output {
            std::fs::write(path, &bytes).map_err(|e| {
                io::Error::new(e.kind(), format!("cannot write {}: {e}", path.to_string_lossy()))
            })?;
            return Ok(());
        }

        if unsafe { waveOutGetNumDevs() } == 0 {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "no audio output device is available; use -o to write a WAV file instead",
            ));
        }

        // PlaySoundW blocks until playback finishes when SND_ASYNC is not set.
        let played = unsafe { PlaySoundW(bytes.as_ptr() as *const u16, core::ptr::null(), SND_MEMORY) };
        if played == 0 {
            return Err(io::Error::other(
                "playback failed (the audio device may be in use by another process)",
            ));
        }
        Ok(())
    }

    fn collect_fields() -> io::Result<Vec<VoiceFields>> {
        let voices = SpeechSynthesizer::AllVoices().map_err(win_err)?;
        let count = voices.Size().map_err(win_err)?;
        let mut fields = Vec::with_capacity(count as usize);
        for index in 0..count {
            let voice = voices.GetAt(index).map_err(win_err)?;
            fields.push(VoiceFields {
                display: voice.DisplayName().map(hstring_to_string).unwrap_or_default(),
                id: voice.Id().map(hstring_to_string).unwrap_or_default(),
                language: voice.Language().map(hstring_to_string).unwrap_or_default(),
                description: voice.Description().map(hstring_to_string).unwrap_or_default(),
            });
        }
        Ok(fields)
    }

    fn ensure_initialized() -> io::Result<()> {
        // RoInitialize is idempotent within a thread when the apartment matches;
        // RPC_E_CHANGED_MODE (already initialized as STA) is reported as an error.
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.map_err(win_err)?;
        Ok(())
    }

    fn win_err<E: std::fmt::Display>(error: E) -> io::Error {
        io::Error::other(error.to_string())
    }

    fn hstring_to_string(value: HSTRING) -> String {
        value.to_string()
    }

    fn gender_label(gender: VoiceGender) -> &'static str {
        match gender {
            VoiceGender::Male => "male",
            VoiceGender::Female => "female",
            _ => "unknown",
        }
    }
}

#[cfg(windows)]
fn stdin_redirected() -> bool {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetStdHandle(n_std_handle: u32) -> isize;
        fn GetFileType(handle: isize) -> u32;
    }

    const STD_INPUT_HANDLE: u32 = 0xFFFF_FFF6u32; // -10
    const INVALID_HANDLE_VALUE: isize = -1;
    const FILE_TYPE_CHAR: u32 = 2;

    unsafe {
        let handle = GetStdHandle(STD_INPUT_HANDLE);
        if handle == 0 || handle == INVALID_HANDLE_VALUE {
            return false;
        }
        GetFileType(handle) != FILE_TYPE_CHAR
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn parses_list_flag() {
        let opts = parse_args([OsString::from("--list-voices")].into_iter()).unwrap();
        assert!(opts.list_voices);
        assert!(opts.text.is_empty());
    }

    #[test]
    fn parses_voice_and_text() {
        let opts = parse_args(
            [
                OsString::from("-v"),
                OsString::from("Haruka"),
                OsString::from("hello"),
                OsString::from("world"),
            ]
            .into_iter(),
        )
        .unwrap();
        assert_eq!(opts.voice.as_deref(), Some(OsStr::new("Haruka")));
        assert_eq!(
            opts.text,
            vec![OsString::from("hello"), OsString::from("world")]
        );
    }

    #[test]
    fn parses_voice_equals_form() {
        let opts =
            parse_args([OsString::from("--voice=ja-JP"), OsString::from("hi")].into_iter()).unwrap();
        assert_eq!(opts.voice.as_deref(), Some(OsStr::new("ja-JP")));
    }

    #[test]
    fn parses_rate_and_volume() {
        let opts = parse_args(
            [
                OsString::from("--rate=5"),
                OsString::from("--volume"),
                OsString::from("80"),
                OsString::from("hi"),
            ]
            .into_iter(),
        )
        .unwrap();
        assert_eq!(opts.rate, Some(5));
        assert_eq!(opts.volume, Some(80));
    }

    #[test]
    fn rejects_bad_rate() {
        assert!(parse_args([OsString::from("--rate=fast")].into_iter()).is_err());
        assert!(parse_args([OsString::from("-r")].into_iter()).is_err());
    }

    #[test]
    fn parses_file_and_output() {
        let opts = parse_args(
            [
                OsString::from("-f"),
                OsString::from("in.txt"),
                OsString::from("-o"),
                OsString::from("out.wav"),
            ]
            .into_iter(),
        )
        .unwrap();
        assert_eq!(opts.file.as_deref(), Some(OsStr::new("in.txt")));
        assert_eq!(opts.output.as_deref(), Some(OsStr::new("out.wav")));
    }

    #[test]
    fn double_dash_treats_rest_as_text() {
        let opts = parse_args(
            ["--".to_string(), "-v".to_string()]
                .into_iter()
                .map(OsString::from),
        )
        .unwrap();
        assert!(opts.voice.is_none());
        assert_eq!(opts.text, vec![OsString::from("-v")]);
    }

    #[test]
    fn rejects_unknown_option() {
        assert!(parse_args([OsString::from("--bogus")].into_iter()).is_err());
    }

    #[test]
    fn join_text_uses_spaces() {
        let opts = parse_args(
            ["hello".to_string(), "world".to_string()]
                .into_iter()
                .map(OsString::from),
        )
        .unwrap();
        assert_eq!(gather_text(&opts).unwrap(), "hello world");
    }

    #[test]
    fn decode_utf8_with_bom() {
        let bytes = [0xEF, 0xBB, 0xBF, b'h', b'i'];
        assert_eq!(decode_bytes(&bytes), "hi");
    }

    #[test]
    fn decode_utf16le_with_bom() {
        // FF FE 'h' 00 'i' 00
        let bytes = [0xFF, 0xFE, b'h', 0x00, b'i', 0x00];
        assert_eq!(decode_bytes(&bytes), "hi");
    }

    #[test]
    fn decode_utf16be_with_bom() {
        // FE FF 00 'h' 00 'i'
        let bytes = [0xFE, 0xFF, 0x00, b'h', 0x00, b'i'];
        assert_eq!(decode_bytes(&bytes), "hi");
    }

    #[test]
    fn decode_plain_ascii() {
        assert_eq!(decode_bytes(b"plain text"), "plain text");
    }

    #[test]
    fn decode_invalid_utf8_is_lossy() {
        // 0x80 is a lone continuation byte with no BOM, so it becomes the
        // replacement character rather than panicking.
        assert_eq!(decode_bytes(&[0x80]), "\u{fffd}");
    }

    #[test]
    fn rate_is_clamped() {
        assert_eq!(clamp_rate(100), 10);
        assert_eq!(clamp_rate(-100), -10);
        assert_eq!(clamp_rate(3), 3);
    }

    #[test]
    fn volume_is_clamped() {
        assert_eq!(clamp_volume(999), 100);
        assert_eq!(clamp_volume(42), 42);
    }

    #[test]
    fn map_rate_keeps_normal_at_zero() {
        assert!((map_rate(0) - 1.0).abs() < f64::EPSILON);
        assert!(map_rate(-10) >= 0.5);
        assert!(map_rate(10) <= 6.0);
    }

    fn sample_voices() -> Vec<VoiceFields> {
        vec![
            VoiceFields {
                display: "Microsoft Ayumi".into(),
                id: "MSTTS_V110_jaJP_AyumiM".into(),
                language: "ja-JP".into(),
                description: "Microsoft Ayumi - Japanese (Japan)".into(),
            },
            VoiceFields {
                display: "Microsoft Haruka".into(),
                id: "MSTTS_V110_jaJP_HarukaM".into(),
                language: "ja-JP".into(),
                description: "Microsoft Haruka - Japanese (Japan)".into(),
            },
            VoiceFields {
                display: "Microsoft Zira".into(),
                id: "MSTTS_V110_enUS_ZiraM".into(),
                language: "en-US".into(),
                description: "Microsoft Zira - English (United States)".into(),
            },
        ]
    }

    #[test]
    fn selects_by_exact_voice_id() {
        let voices = sample_voices();
        let idx = select_index(&voices, Some("MSTTS_V110_jaJP_HarukaM"), None);
        assert_eq!(idx, Some(1));
    }

    #[test]
    fn selects_by_display_substring() {
        let voices = sample_voices();
        let idx = select_index(&voices, None, Some("haruka"));
        assert_eq!(idx, Some(1));
    }

    #[test]
    fn selects_by_language() {
        let voices = sample_voices();
        let idx = select_index(&voices, None, Some("en-US"));
        assert_eq!(idx, Some(2));
    }

    #[test]
    fn returns_none_for_default_voice() {
        let voices = sample_voices();
        assert_eq!(select_index(&voices, None, None), None);
    }

    #[test]
    fn missing_voice_is_none() {
        let voices = sample_voices();
        assert_eq!(select_index(&voices, None, Some("nobody")), None);
    }
}

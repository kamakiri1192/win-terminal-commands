//! `gzip` / `gunzip` engine.
//!
//! Implements the subset of GNU gzip needed for everyday use: file and
//! stream compression/decompression, the common flags (`-c -d -f -k -t`),
//! and compression levels. Output is a standard gzip (RFC 1952) stream,
//! so it interoperates with the system `gunzip`, `tar -z`, etc.
//!
//! `gzip` and `gunzip` are the same engine; `gunzip` simply defaults to
//! decompress mode (see [`run`]).

use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use flate2::read::MultiGzDecoder;
use flate2::write::GzEncoder;
use flate2::{Compression, GzBuilder};

/// Default compression level when none is requested (matches GNU gzip).
const DEFAULT_LEVEL: u32 = 6;

#[derive(Debug, Default)]
struct Options {
    stdout: bool,
    decompress: bool,
    force: bool,
    keep: bool,
    test: bool,
    level: Option<u32>,
    help: bool,
    version: bool,
    files: Vec<OsString>,
}

/// Entry point shared by the `gzip` and `gunzip` binaries.
///
/// `program` is used in diagnostics. `default_decompress` is `true` for
/// `gunzip` so that it decompresses without `-d`.
pub fn run(program: &str, default_decompress: bool) -> i32 {
    let args: Vec<OsString> = env::args_os().skip(1).collect();

    let opts = match parse_args(&args) {
        Ok(opts) => opts,
        Err(message) => {
            eprintln!("{program}: {message}");
            eprintln!("Try '{program} --help' for more information.");
            return 2;
        }
    };

    if opts.help {
        print_help(program);
        return 0;
    }
    if opts.version {
        println!(
            "{program} (win-terminal-commands) {}",
            env!("CARGO_PKG_VERSION")
        );
        return 0;
    }

    let level = opts.level.unwrap_or(DEFAULT_LEVEL);
    // `-t` decompresses to /dev/null, so it implies decompress mode.
    let decompress = opts.decompress || default_decompress || opts.test;

    let mut status = 0;

    if opts.files.is_empty() {
        if let Err(error) = stream_stdin(decompress, opts.test, level) {
            eprintln!("{program}: {error}");
            status = 1;
        }
        return status;
    }

    for file in &opts.files {
        let result = if file == OsStr::new("-") {
            stream_stdin(decompress, opts.test, level).map_err(|e| e.to_string())
        } else {
            process_path(file, decompress, &opts, level)
        };

        if let Err(message) = result {
            eprintln!("{program}: {message}");
            status = 1;
        }
    }

    status
}

fn process_path(arg: &OsStr, decompress: bool, opts: &Options, level: u32) -> Result<(), String> {
    let path = Path::new(arg);
    if decompress {
        process_decompress(path, opts)
    } else {
        process_compress(path, opts, level)
    }
}

fn process_compress(path: &Path, opts: &Options, level: u32) -> Result<(), String> {
    if path.is_dir() {
        return Err(format!("{}: Is a directory", path.display()));
    }
    if !path.is_file() {
        return Err(format!("{}: No such file or directory", path.display()));
    }

    if opts.stdout {
        compress_file_to_stdout(path, level).map_err(|e| format!("{}: {e}", path.display()))?;
        return Ok(());
    }

    if !opts.force && has_gz_suffix(path) {
        // Mirror gzip: by default refuse to double-compress a `.gz` file.
        // `-f` overrides this guard.
        eprintln!(
            "gzip: {} already has .gz suffix -- unchanged",
            path.display()
        );
        return Ok(());
    }

    let output = compressed_name(path);
    if output.exists() && !opts.force {
        return Err(format!(
            "{} already exists -- not overwritten",
            output.display()
        ));
    }

    compress_file(path, &output, level).map_err(|e| format!("{}: {e}", path.display()))?;
    copy_mode(path, &output);

    if !opts.keep {
        if let Err(error) = remove_source(path) {
            return Err(format!("cannot remove {}: {error}", path.display()));
        }
    }

    Ok(())
}

fn process_decompress(path: &Path, opts: &Options) -> Result<(), String> {
    if path.is_dir() {
        return Err(format!("{}: Is a directory", path.display()));
    }
    if !path.is_file() {
        return Err(format!("{}: No such file or directory", path.display()));
    }

    if opts.test {
        test_file(path).map_err(|e| format!("{}: {e}", path.display()))?;
        return Ok(());
    }

    if opts.stdout {
        decompress_file_to_stdout(path).map_err(|e| format!("{}: {e}", path.display()))?;
        return Ok(());
    }

    let output = match decompressed_name(path) {
        Some(name) => name,
        None => {
            if opts.force {
                forced_name(path)
            } else {
                eprintln!("gzip: {}: unknown suffix -- ignored", path.display());
                return Ok(());
            }
        }
    };

    if output.exists() && !opts.force {
        return Err(format!(
            "{} already exists -- not overwritten",
            output.display()
        ));
    }

    decompress_file(path, &output).map_err(|e| format!("{}: {e}", path.display()))?;
    // Restore the time stamp while the output is still writable, then copy
    // the archive's mode last — a read-only archive would otherwise make the
    // timestamp restore fail on Windows.
    if let Some(mtime) = read_gzip_mtime(path) {
        if let Err(error) = restore_mtime(&output, mtime) {
            eprintln!(
                "gzip: {}: cannot restore time stamp: {error}",
                output.display()
            );
        }
    }
    copy_mode(path, &output);

    if !opts.keep {
        if let Err(error) = remove_source(path) {
            return Err(format!("cannot remove {}: {error}", path.display()));
        }
    }

    Ok(())
}

/// Compress/decompress/test the standard input to the standard output.
fn stream_stdin(decompress: bool, test: bool, level: u32) -> io::Result<()> {
    if test {
        let stdin = io::stdin();
        let mut decoder = MultiGzDecoder::new(stdin.lock());
        io::copy(&mut decoder, &mut io::sink())?;
        return Ok(());
    }

    if decompress {
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut decoder = MultiGzDecoder::new(stdin.lock());
        let mut out = stdout.lock();
        io::copy(&mut decoder, &mut out)?;
        out.flush()?;
        return Ok(());
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut encoder = GzEncoder::new(stdout.lock(), Compression::new(level));
    let mut input = stdin.lock();
    io::copy(&mut input, &mut encoder)?;
    // finish() flushes and returns the locked stdout; drop it explicitly so
    // the archive footer is written before the lock releases.
    drop(encoder.finish()?);
    Ok(())
}

fn compress_file(input: &Path, output: &Path, level: u32) -> io::Result<()> {
    let mtime = file_mtime(input);
    let mut infile = File::open(input)?;
    let mut outfile = File::create(output)?;

    let result = {
        let mut builder = GzBuilder::new().mtime(mtime);
        if let Some(name) = input.file_name().and_then(|n| n.to_str()) {
            // Store the original file name in the gzip header so that
            // `gunzip` can restore it. An owned String avoids borrowing the
            // name into the encoder (the header bytes are copied internally).
            builder = builder.filename(name.to_string());
        }
        let mut encoder = builder.read(&mut infile, Compression::new(level));
        io::copy(&mut encoder, &mut outfile)
    };

    match result {
        Ok(_) => {
            outfile.flush()?;
            Ok(())
        }
        Err(error) => {
            // Never leave a half-written archive behind.
            let _ = fs::remove_file(output);
            Err(error)
        }
    }
}

fn compress_file_to_stdout(input: &Path, level: u32) -> io::Result<()> {
    let mtime = file_mtime(input);
    let mut infile = File::open(input)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let mut builder = GzBuilder::new().mtime(mtime);
    if let Some(name) = input.file_name().and_then(|n| n.to_str()) {
        builder = builder.filename(name.to_string());
    }
    let mut encoder = builder.read(&mut infile, Compression::new(level));
    io::copy(&mut encoder, &mut out)?;
    out.flush()?;
    Ok(())
}

fn decompress_file(input: &Path, output: &Path) -> io::Result<()> {
    let mut infile = File::open(input)?;
    let mut outfile = File::create(output)?;

    let result = {
        // MultiGzDecoder reads every member of a (possibly concatenated)
        // gzip stream, so `cat a.gz b.gz` decompresses in full.
        let mut decoder = MultiGzDecoder::new(&mut infile);
        io::copy(&mut decoder, &mut outfile)
    };

    match result {
        Ok(_) => {
            outfile.flush()?;
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(output);
            Err(error)
        }
    }
}

fn decompress_file_to_stdout(input: &Path) -> io::Result<()> {
    let mut infile = File::open(input)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut decoder = MultiGzDecoder::new(&mut infile);
    io::copy(&mut decoder, &mut out)?;
    out.flush()?;
    Ok(())
}

fn test_file(input: &Path) -> io::Result<()> {
    let mut infile = File::open(input)?;
    let mut decoder = MultiGzDecoder::new(&mut infile);
    io::copy(&mut decoder, &mut io::sink())?;
    Ok(())
}

fn file_mtime(path: &Path) -> u32 {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as u32)
        .unwrap_or(0)
}

fn copy_mode(from: &Path, to: &Path) {
    if let Ok(metadata) = fs::metadata(from) {
        let _ = fs::set_permissions(to, metadata.permissions());
    }
}

/// Read the modification time stored in a gzip header.
///
/// `MTIME` always sits at bytes 4..8 (little-endian u32) regardless of the
/// optional fields, so there's no need to parse the variable-length part.
/// Returns `None` for non-gzip data or a header with `MTIME == 0` (the
/// "no timestamp" value).
fn read_gzip_mtime(path: &Path) -> Option<u32> {
    let mut file = File::open(path).ok()?;
    let mut header = [0u8; 8];
    file.read_exact(&mut header).ok()?;
    if header[0] != 0x1f || header[1] != 0x8b {
        return None;
    }
    let mtime = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
    (mtime != 0).then_some(mtime)
}

/// Restore the gzip header's mtime onto the decompressed file, mirroring
/// `gunzip` (which applies the original timestamp unless `-n` is given).
fn restore_mtime(path: &Path, mtime: u32) -> io::Result<()> {
    let time = UNIX_EPOCH + Duration::from_secs(mtime as u64);
    let times = fs::FileTimes::new().set_modified(time);
    OpenOptions::new().write(true).open(path)?.set_times(times)
}

/// Delete the source file after a successful (de)compression.
///
/// Windows refuses to delete files that carry the read-only attribute, so
/// on failure we clear that bit and retry once — the same dance `gzip`
/// performs on Windows.
#[cfg(windows)]
#[allow(clippy::permissions_set_readonly_false)] // Windows-only: clears the read-only attribute.
fn remove_source(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(initial) => {
            if let Ok(metadata) = fs::metadata(path) {
                let mut permissions = metadata.permissions();
                if permissions.readonly() {
                    permissions.set_readonly(false);
                    let _ = fs::set_permissions(path, permissions);
                }
            }
            match fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(retry) => Err(io::Error::new(retry.kind(), format!("{initial}; {retry}"))),
            }
        }
    }
}

#[cfg(not(windows))]
fn remove_source(path: &Path) -> io::Result<()> {
    fs::remove_file(path)
}

fn has_gz_suffix(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase().ends_with(".gz"))
        .unwrap_or(false)
}

fn compressed_name(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".gz");
    PathBuf::from(name)
}

/// Strips a recognized gzip suffix. Returns `None` when no suffix matches,
/// leaving the caller to decide what to do (skip, or fall back to `-f`).
fn decompressed_name(path: &Path) -> Option<PathBuf> {
    let file_name = path.file_name()?.to_str()?;
    let lower = file_name.to_ascii_lowercase();

    // `(suffix_length, replace_with_tar)` for each recognized suffix.
    let (drop, replace_with_tar) = if lower.ends_with(".tgz") || lower.ends_with(".taz") {
        (4, true)
    } else if lower.ends_with(".gz") {
        (3, false)
    } else if lower.ends_with(".z") {
        (2, false)
    } else {
        return None;
    };

    // The suffixes are ASCII, so byte-slicing at len - drop is a valid char
    // boundary and preserves the original casing of the stem.
    let stem = &file_name[..file_name.len() - drop];
    if stem.is_empty() {
        return None;
    }

    let new_name = if replace_with_tar {
        format!("{stem}.tar")
    } else {
        stem.to_string()
    };
    Some(path.with_file_name(new_name))
}

/// Fallback output name used when decompressing a file with an unknown
/// suffix under `--force` (mirrors gzip's safe `.out` behavior).
fn forced_name(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".out");
    PathBuf::from(name)
}

fn parse_args(args: &[OsString]) -> Result<Options, String> {
    let mut opts = Options::default();
    let mut only_files = false;

    for arg in args {
        if only_files {
            opts.files.push(arg.clone());
            continue;
        }

        let text = arg.to_string_lossy();
        let token = text.as_ref();

        match token {
            "--" => only_files = true,
            "-" => opts.files.push(arg.clone()),
            _ if token.starts_with("--") => match token {
                "--stdout" | "--to-stdout" => opts.stdout = true,
                "--decompress" | "--uncompress" => opts.decompress = true,
                "--force" => opts.force = true,
                "--keep" => opts.keep = true,
                "--test" => opts.test = true,
                "--fast" => opts.level = Some(1),
                "--best" => opts.level = Some(9),
                "--help" => opts.help = true,
                "--version" => opts.version = true,
                other => return Err(format!("unknown option: {other}")),
            },
            _ if token.starts_with('-') => {
                // A cluster of short flags, e.g. `-cf9` or `-d`.
                for char in token[1..].chars() {
                    match char {
                        'c' => opts.stdout = true,
                        'd' => opts.decompress = true,
                        'f' => opts.force = true,
                        'k' => opts.keep = true,
                        't' => opts.test = true,
                        'h' => opts.help = true,
                        'V' => opts.version = true,
                        '0'..='9' => opts.level = Some(char.to_digit(10).unwrap()),
                        other => return Err(format!("unknown option: -{other}")),
                    }
                }
            }
            _ => opts.files.push(arg.clone()),
        }
    }

    Ok(opts)
}

fn print_help(program: &str) {
    println!("Usage: {program} [OPTION]... [FILE]...");
    println!("Compress FILEs (by default, each gets a .gz suffix).");
    println!("With no FILE, or when FILE is -, read standard input.");
    println!();
    println!("Options:");
    println!("  -c, --stdout       write to standard output, keep original files");
    println!("  -d, --decompress   decompress");
    println!("  -f, --force        force overwrite of output file");
    println!("  -k, --keep         keep (don't delete) input files");
    println!("  -t, --test         test compressed file integrity");
    println!("  -1 .. -9           compression level (1=fast, 9=best, default 6)");
    println!("      --fast         alias for -1");
    println!("      --best         alias for -9");
    println!("  -h, --help         display this help and exit");
    println!("  -V, --version      print version");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("wtc-gzip-{label}-{nonce}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn opts(level: Option<u32>) -> Options {
        Options {
            level,
            ..Default::default()
        }
    }

    #[test]
    fn parses_short_level_flags() {
        let o = parse_args(&[OsString::from("-9")]).unwrap();
        assert_eq!(o.level, Some(9));
        assert!(o.files.is_empty());
    }

    #[test]
    fn parses_combined_short_flags() {
        let o = parse_args(&[OsString::from("-cfk"), OsString::from("a.txt")]).unwrap();
        assert!(o.stdout && o.force && o.keep);
        assert_eq!(o.files, vec![OsString::from("a.txt")]);
    }

    #[test]
    fn parses_long_flags_and_level_in_cluster() {
        let o = parse_args(&[OsString::from("--decompress"), OsString::from("-c6")]).unwrap();
        assert!(o.decompress && o.stdout);
        assert_eq!(o.level, Some(6));
    }

    #[test]
    fn double_dash_treats_rest_as_files() {
        let o = parse_args(&[OsString::from("--"), OsString::from("-weird")]).unwrap();
        assert_eq!(o.files, vec![OsString::from("-weird")]);
    }

    #[test]
    fn unknown_option_is_rejected() {
        assert!(parse_args(&[OsString::from("--nope")]).is_err());
        assert!(parse_args(&[OsString::from("-Z")]).is_err());
    }

    #[test]
    fn compressed_name_appends_gz() {
        assert_eq!(
            compressed_name(Path::new("dir/a.txt")),
            PathBuf::from("dir/a.txt.gz")
        );
    }

    #[test]
    fn detects_gz_suffix_case_insensitively() {
        assert!(has_gz_suffix(Path::new("a.GZ")));
        assert!(has_gz_suffix(Path::new("a.gz")));
        assert!(!has_gz_suffix(Path::new("a.tar")));
    }

    #[test]
    fn decompressed_name_strips_suffixes() {
        assert_eq!(
            decompressed_name(Path::new("dir/a.txt.gz")),
            Some(PathBuf::from("dir/a.txt"))
        );
        assert_eq!(
            decompressed_name(Path::new("a.Z")),
            Some(PathBuf::from("a"))
        );
        assert_eq!(
            decompressed_name(Path::new("archive.tgz")),
            Some(PathBuf::from("archive.tar"))
        );
        assert_eq!(decompressed_name(Path::new("plain.txt")), None);
    }

    #[test]
    fn round_trip_compress_then_decompress() {
        let dir = temp_dir("roundtrip");
        let src = dir.join("data.txt");
        let payload = b"the quick brown fox\n".repeat(500);
        fs::write(&src, &payload).unwrap();

        process_compress(&src, &opts(Some(6)), 6).expect("compress");
        let gz = compressed_name(&src);
        assert!(gz.is_file(), "archive should exist");
        assert!(!src.exists(), "source should be removed by default");

        // Gzip magic header.
        let mut head = [0u8; 2];
        use std::io::Read as _;
        File::open(&gz).unwrap().read_exact(&mut head).unwrap();
        assert_eq!(head, [0x1f, 0x8b]);

        process_decompress(&gz, &opts(None)).expect("decompress");
        assert_eq!(fs::read(&src).unwrap(), payload);
        assert!(!gz.exists(), "archive should be removed after decompress");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn keep_flag_preserves_source() {
        let dir = temp_dir("keep");
        let src = dir.join("keep.txt");
        fs::write(&src, b"keep me").unwrap();

        let mut keep_opts = opts(Some(1));
        keep_opts.keep = true;
        process_compress(&src, &keep_opts, 1).expect("compress");

        assert!(src.is_file(), "source kept with -k");
        assert!(compressed_name(&src).is_file());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_flag_validates_archive() {
        let dir = temp_dir("test");
        let src = dir.join("t.txt");
        fs::write(&src, b"validate me").unwrap();
        process_compress(&src, &opts(Some(9)), 9).expect("compress");
        let gz = compressed_name(&src);

        let mut test_opts = opts(None);
        test_opts.test = true;
        process_decompress(&gz, &test_opts).expect("test should pass on a valid archive");

        let corrupt = dir.join("broken.gz");
        fs::write(&corrupt, b"not gzip").unwrap();
        assert!(process_decompress(&corrupt, &test_opts).is_err());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn refuses_to_overwrite_without_force() {
        let dir = temp_dir("overwrite");
        let src = dir.join("o.txt");
        fs::write(&src, b"first").unwrap();
        process_compress(&src, &opts(Some(6)), 6).expect("compress");

        // Recreate the source so we can try compressing into the existing .gz.
        fs::write(&src, b"second").unwrap();
        let err = process_compress(&src, &opts(Some(6)), 6).unwrap_err();
        assert!(err.contains("already exists"));

        // With force it should succeed and replace the archive.
        let mut force_opts = opts(Some(6));
        force_opts.force = true;
        process_compress(&src, &force_opts, 6).expect("force compress");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn decompresses_concatenated_members() {
        // A gzip stream may contain several members (e.g. `cat a.gz b.gz`).
        // The whole stream must decode, not just the first member.
        let dir = temp_dir("multi");
        let a = dir.join("a.txt");
        let b = dir.join("b.txt");
        fs::write(&a, b"AAA\n").unwrap();
        fs::write(&b, b"BBB\n").unwrap();

        process_compress(&a, &opts(Some(6)), 6).unwrap();
        process_compress(&b, &opts(Some(6)), 6).unwrap();

        let mut combined = fs::read(compressed_name(&a)).unwrap();
        combined.extend(fs::read(compressed_name(&b)).unwrap());
        let multi = dir.join("multi.gz");
        fs::write(&multi, &combined).unwrap();

        let out = dir.join("multi.out");
        decompress_file(&multi, &out).unwrap();
        assert_eq!(fs::read(&out).unwrap(), b"AAA\nBBB\n");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn force_overrides_gz_suffix_guard() {
        let dir = temp_dir("forcegz");
        let src = dir.join("data.gz"); // already has .gz suffix
        fs::write(&src, b"pretend this is an archive").unwrap();

        // Without -f the guard kicks in: no new file.
        process_compress(&src, &opts(Some(6)), 6).unwrap();
        assert!(!compressed_name(&src).exists(), "no .gz.gz without -f");

        let mut force_opts = opts(Some(6));
        force_opts.force = true;
        process_compress(&src, &force_opts, 6).expect("force should recompress");
        assert!(compressed_name(&src).exists(), "force creates data.gz.gz");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn decompression_restores_header_mtime() {
        let dir = temp_dir("mtime");
        let src = dir.join("m.txt");
        fs::write(&src, b"timestamped").unwrap();

        // Pin the source to a known old mtime (seconds granularity).
        let pinned_secs: u64 = 1_000_000_000;
        let pinned = UNIX_EPOCH + Duration::from_secs(pinned_secs);
        let handle = fs::OpenOptions::new().write(true).open(&src).unwrap();
        handle
            .set_times(fs::FileTimes::new().set_modified(pinned))
            .unwrap();
        drop(handle);

        process_compress(&src, &opts(Some(6)), 6).unwrap();
        let gz = compressed_name(&src);
        assert_eq!(read_gzip_mtime(&gz), Some(pinned_secs as u32));

        process_decompress(&gz, &opts(None)).unwrap();
        let restored = fs::metadata(&src)
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(restored, pinned_secs);

        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(windows)]
    #[test]
    fn removes_readonly_source() {
        let dir = temp_dir("ro");
        let src = dir.join("ro.txt");
        fs::write(&src, b"readonly").unwrap();
        let mut permissions = fs::metadata(&src).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&src, permissions).unwrap();

        process_compress(&src, &opts(Some(6)), 6).expect("compress read-only source");
        assert!(!src.exists(), "read-only source should be removed");
        assert!(compressed_name(&src).is_file(), "archive should be created");

        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(windows)]
    #[test]
    fn restores_mtime_from_readonly_archive() {
        let dir = temp_dir("romtime");
        let src = dir.join("m.txt");
        fs::write(&src, b"x").unwrap();
        let pinned_secs: u64 = 1_234_567_890;
        let handle = fs::OpenOptions::new().write(true).open(&src).unwrap();
        handle
            .set_times(
                fs::FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(pinned_secs)),
            )
            .unwrap();
        drop(handle);

        process_compress(&src, &opts(Some(6)), 6).unwrap();
        let gz = compressed_name(&src);

        // Mark the archive read-only to prove mtime is restored before the
        // mode is copied onto the output.
        let mut permissions = fs::metadata(&gz).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&gz, permissions).unwrap();

        process_decompress(&gz, &opts(None)).unwrap();
        let restored = fs::metadata(&src)
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(restored, pinned_secs);

        let _ = fs::remove_dir_all(dir);
    }
}

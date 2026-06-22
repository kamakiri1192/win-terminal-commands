//! `md5` — compute or verify MD5 checksums, macOS/BSD `md5` compatible.
//!
//! Implemented in pure Rust with no external dependencies so it stays consistent
//! with the rest of this crate. Mirrors the common subset of macOS/BSD `md5`:
//! file hashing, stdin hashing, `-s` string, `-c` check, `-q`, `-r`, `-p`.

use std::env;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

const USAGE: &str = "\
usage: md5 [-pqr] [-c file] [-s string] [file ...]

Compute or check MD5 checksums (macOS / BSD `md5` compatible).

With no options and no files, the checksum of standard input is printed.
With file arguments, each file is hashed as `MD5 (file) = <hash>`.

Options:
  -s, --string string   Hash the given string literal
  -c, --check file      Read checksums from a file and verify them
  -q, --quiet           Print only the checksum (suppress OK lines in check mode)
  -r, --reverse         Print the checksum before the name (BSD/text format)
  -p, --print           Echo standard input to stdout and append the checksum
  -h, --help            Show this help message

Exit codes: 0 = success, 1 = a file could not be read or a checksum did not
match, 2 = usage error.";

#[derive(Debug, Default)]
struct Options {
    string: Option<OsString>,
    check: Option<OsString>,
    quiet: bool,
    reverse: bool,
    print: bool,
    files: Vec<OsString>,
}

fn main() {
    let args: Vec<OsString> = env::args_os().skip(1).collect();

    let options = match parse_args(args) {
        Ok(options) => options,
        Err(message) if message == "help requested" => {
            print!("{USAGE}");
            io::stdout().flush().ok();
            std::process::exit(0);
        }
        Err(message) => {
            eprintln!("md5: {message}");
            eprint!("{USAGE}");
            std::process::exit(2);
        }
    };

    let code = run(&options);
    std::process::exit(code);
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
                // Long-form options.
                match text {
                    "--help" => return Err("help requested".to_string()),
                    "--quiet" => {
                        options.quiet = true;
                        continue;
                    }
                    "--reverse" => {
                        options.reverse = true;
                        continue;
                    }
                    "--print" => {
                        options.print = true;
                        continue;
                    }
                    "--string" => {
                        options.string = Some(
                            iter.next()
                                .ok_or_else(|| "option -s requires an argument".to_string())?,
                        );
                        continue;
                    }
                    "--check" => {
                        options.check = Some(
                            iter.next()
                                .ok_or_else(|| "option -c requires an argument".to_string())?,
                        );
                        continue;
                    }
                    _ => {}
                }
                if let Some(rest) = text.strip_prefix("--string=") {
                    options.string = Some(OsString::from(rest));
                    continue;
                }
                if let Some(rest) = text.strip_prefix("--check=") {
                    options.check = Some(OsString::from(rest));
                    continue;
                }

                // Short-flag cluster: a single leading dash, not `--`, not just `-`.
                if text.starts_with('-') && !text.starts_with("--") && text != "-" {
                    handle_short_cluster(text, &mut iter, &mut options)?;
                    continue;
                }

                // Any other token beginning with `-` is an unknown option.
                if text.starts_with('-') {
                    return Err(format!("unknown option: {text}"));
                }
            }
        }

        options.files.push(arg);
    }

    Ok(options)
}

/// Expand a clustered short-flag token such as `-qr` or `-s hello`.
/// A flag that takes an argument (`-s` / `-c`) consumes the remainder of the
/// cluster as its value, or the next argument when it is the last flag.
fn handle_short_cluster<I>(text: &str, iter: &mut I, options: &mut Options) -> Result<(), String>
where
    I: Iterator<Item = OsString>,
{
    // `text` starts with a single ASCII `-`, so byte index 1 is a char boundary.
    let mut chars = text[1..].chars();
    while let Some(ch) = chars.next() {
        match ch {
            'q' => options.quiet = true,
            'r' => options.reverse = true,
            'p' => options.print = true,
            'h' => return Err("help requested".to_string()),
            's' | 'c' => {
                let rest: String = chars.collect();
                let value = if !rest.is_empty() {
                    OsString::from(rest)
                } else {
                    iter.next()
                        .ok_or_else(|| format!("option -{ch} requires an argument"))?
                };
                if ch == 's' {
                    options.string = Some(value);
                } else {
                    options.check = Some(value);
                }
                return Ok(());
            }
            other => return Err(format!("unknown option: -{other}")),
        }
    }
    Ok(())
}

fn run(options: &Options) -> i32 {
    // Mode precedence, matching macOS `md5`:
    //   -s string  >  -c check  >  -p print  >  files  >  stdin
    if let Some(string) = &options.string {
        return run_string(string, options.quiet, options.reverse);
    }

    if let Some(check_file) = &options.check {
        return run_check(check_file, options.quiet);
    }

    if options.print {
        return run_print();
    }

    if !options.files.is_empty() {
        return run_files(&options.files, options.quiet, options.reverse);
    }

    run_stdin()
}

fn run_string(string: &OsString, quiet: bool, reverse: bool) -> i32 {
    let text = string.to_string_lossy();
    let digest = Md5::digest(text.as_bytes());
    let hash = hex(&digest);
    let label = format!("\"{text}\"");
    println!("{}", format_output(&label, &hash, quiet, reverse));
    0
}

fn run_files(files: &[OsString], quiet: bool, reverse: bool) -> i32 {
    let mut failed = false;
    for file in files {
        match hash_path(Path::new(file)) {
            Ok(digest) => {
                let hash = hex(&digest);
                let label = file.to_string_lossy();
                println!("{}", format_output(&label, &hash, quiet, reverse));
            }
            Err(error) => {
                eprintln!("md5: {}: {error}", file.to_string_lossy());
                failed = true;
            }
        }
    }
    if failed {
        1
    } else {
        0
    }
}

fn run_stdin() -> i32 {
    let stdin = io::stdin();
    let mut lock = stdin.lock();
    match hash_reader(&mut lock) {
        Ok(digest) => {
            println!("{}", hex(&digest));
            0
        }
        Err(error) => {
            eprintln!("md5: {error}");
            1
        }
    }
}

/// Echo standard input to standard output, then append the checksum.
fn run_print() -> i32 {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();

    let mut hasher = Md5::new();
    let mut buffer = [0u8; 65536];
    loop {
        match input.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                hasher.update(&buffer[..n]);
                if let Err(error) = output.write_all(&buffer[..n]) {
                    eprintln!("md5: {error}");
                    return 1;
                }
            }
            Err(error) => {
                eprintln!("md5: {error}");
                return 1;
            }
        }
    }

    if let Err(error) = output.flush() {
        eprintln!("md5: {error}");
        return 1;
    }

    println!("{}", hex(&hasher.finalize()));
    0
}

/// Read checksums from `check_file` and verify each referenced file.
/// Supports both the GNU/coreutils `md5sum` text format
/// (`<hash>  <name>`) and the BSD format (`MD5 (name) = <hash>`).
///
/// A malformed (non-empty, unparseable) line is treated as a verification
/// failure — matching GNU `md5sum`, a corrupted checksum file never reports
/// success.
fn run_check(check_file: &OsString, quiet: bool) -> i32 {
    let contents = match std::fs::read_to_string(check_file) {
        Ok(contents) => contents,
        Err(error) => {
            eprintln!("md5: {}: {error}", check_file.to_string_lossy());
            return 1;
        }
    };

    let mut total = 0usize;
    let mut failed = 0usize;
    let mut malformed = 0usize;

    for line in contents.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let (name, expected) = match parse_checksum_line(line) {
            Some(entry) => entry,
            None => {
                eprintln!("md5: WARNING: {line} is improperly formatted");
                malformed += 1;
                continue;
            }
        };

        total += 1;
        match hash_path(Path::new(&name)) {
            Ok(digest) => {
                let actual = hex(&digest);
                if actual.eq_ignore_ascii_case(&expected) {
                    if !quiet {
                        println!("{name}: OK");
                    }
                } else {
                    println!("{name}: FAILED");
                    failed += 1;
                }
            }
            Err(_) => {
                println!("{name}: FAILED open or read");
                failed += 1;
            }
        }
    }

    if failed > 0 {
        eprintln!("md5: WARNING: {failed} of {total} computed checksums did NOT match");
    }
    if malformed > 0 {
        let plural = if malformed == 1 { "line is" } else { "lines are" };
        eprintln!("md5: WARNING: {malformed} {plural} improperly formatted");
    }

    if total == 0 {
        if malformed == 0 {
            eprintln!(
                "md5: {}: no properly formatted checksum lines found",
                check_file.to_string_lossy()
            );
        }
        return 1;
    }

    if failed > 0 || malformed > 0 {
        1
    } else {
        0
    }
}

fn hash_path(path: &Path) -> io::Result<[u8; 16]> {
    let mut file = File::open(path)?;
    hash_reader(&mut file)
}

fn hash_reader<R: Read>(reader: &mut R) -> io::Result<[u8; 16]> {
    let mut hasher = Md5::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hasher.finalize())
}

/// Render a digest in the style selected by the options.
fn format_output(label: &str, hash: &str, quiet: bool, reverse: bool) -> String {
    if quiet {
        hash.to_string()
    } else if reverse {
        format!("{hash}  {label}")
    } else {
        format!("MD5 ({label}) = {hash}")
    }
}

/// Parse one checksum line in either GNU or BSD format.
/// Returns `(name, lowercase_hex_hash)` when the line is recognized.
///
/// Parsing is strict: the line is taken verbatim (line terminators are already
/// stripped by the caller). Nothing is trimmed, so a name keeps its exact bytes
/// and a line with stray leading/trailing whitespace is reported as malformed
/// rather than silently massaged into a valid entry.
fn parse_checksum_line(line: &str) -> Option<(String, String)> {
    if line.is_empty() {
        return None;
    }

    // BSD format, case-insensitive prefix: `MD5 (name) = <hash>`.
    if has_md5_prefix(line) {
        let rest = &line[5..];
        let close = rest.find(") = ")?;
        let name = &rest[..close];
        let hash = &rest[close + 4..];
        if is_hex32(hash) {
            return Some((name.to_string(), hash.to_ascii_lowercase()));
        }
        return None;
    }

    // GNU/coreutils format: `<32-hex><space><indicator><name>`,
    // where the indicator is ` ` (text mode) or `*` (binary mode).
    let bytes = line.as_bytes();
    if bytes.len() >= 34 && bytes[32] == b' ' {
        let hash = &line[..32];
        let indicator = bytes[33];
        if is_hex32(hash) && (indicator == b' ' || indicator == b'*') {
            let name = &line[34..];
            return Some((name.to_string(), hash.to_ascii_lowercase()));
        }
    }

    None
}

/// True when `s` starts with `MD5 (` ignoring the case of `MD`.
fn has_md5_prefix(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() >= 5
        && bytes[0].eq_ignore_ascii_case(&b'M')
        && bytes[1].eq_ignore_ascii_case(&b'D')
        && bytes[2] == b'5'
        && bytes[3] == b' '
        && bytes[4] == b'('
}

fn is_hex32(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn hex(digest: &[u8; 16]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(32);
    for &byte in digest {
        out.push(TABLE[(byte >> 4) as usize] as char);
        out.push(TABLE[(byte & 0x0f) as usize] as char);
    }
    out
}

// ---------------------------------------------------------------------------
// MD5 (RFC 1321) — streaming implementation, no external dependencies.
// ---------------------------------------------------------------------------

/// Per-round shift amounts.
const S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9,
    14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15,
    21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

/// Per-round constants: floor(2**32 * abs(sin(i + 1))).
const K: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

struct Md5 {
    state: [u32; 4],
    buffer: [u8; 64],
    buffer_len: usize,
    /// Count of bytes absorbed, used for the trailing length field.
    length: u64,
}

impl Md5 {
    fn new() -> Self {
        Md5 {
            state: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476],
            buffer: [0; 64],
            buffer_len: 0,
            length: 0,
        }
    }

    fn digest(data: &[u8]) -> [u8; 16] {
        let mut hasher = Md5::new();
        hasher.update(data);
        hasher.finalize()
    }

    fn update(&mut self, mut data: &[u8]) {
        self.length = self.length.wrapping_add(data.len() as u64);

        if self.buffer_len > 0 {
            let need = 64 - self.buffer_len;
            if data.len() < need {
                self.buffer[self.buffer_len..self.buffer_len + data.len()].copy_from_slice(data);
                self.buffer_len += data.len();
                return;
            }
            self.buffer[self.buffer_len..].copy_from_slice(&data[..need]);
            digest_block(&mut self.state, &self.buffer);
            self.buffer_len = 0;
            data = &data[need..];
        }

        while data.len() >= 64 {
            digest_block(&mut self.state, &data[..64]);
            data = &data[64..];
        }

        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
            self.buffer_len = data.len();
        }
    }

    fn finalize(mut self) -> [u8; 16] {
        let bit_length = self.length.wrapping_mul(8);

        // Append the 0x80 bit, then zero-pad.
        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;

        if self.buffer_len > 56 {
            for slot in &mut self.buffer[self.buffer_len..64] {
                *slot = 0;
            }
            digest_block(&mut self.state, &self.buffer);
            self.buffer_len = 0;
        }

        for slot in &mut self.buffer[self.buffer_len..56] {
            *slot = 0;
        }
        self.buffer[56..64].copy_from_slice(&bit_length.to_le_bytes());

        digest_block(&mut self.state, &self.buffer);

        let mut out = [0u8; 16];
        for (i, word) in self.state.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        out
    }
}

/// Compress one 64-byte little-endian block into the running state.
fn digest_block(state: &mut [u32; 4], block: &[u8]) {
    let mut m = [0u32; 16];
    for (i, chunk) in block.chunks_exact(4).enumerate() {
        m[i] = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }

    let [mut a, mut b, mut c, mut d] = *state;

    for i in 0..64 {
        let (f, g) = if i < 16 {
            ((b & c) | (!b & d), i)
        } else if i < 32 {
            ((d & b) | (!d & c), (5 * i + 1) % 16)
        } else if i < 48 {
            (b ^ c ^ d, (3 * i + 5) % 16)
        } else {
            (c ^ (b | !d), (7 * i) % 16)
        };

        let temp = d;
        d = c;
        c = b;
        b = b.wrapping_add(
            a.wrapping_add(f)
                .wrapping_add(K[i])
                .wrapping_add(m[g])
                .rotate_left(S[i]),
        );
        a = temp;
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    // RFC 1321 / MD5 reference test vectors.
    #[test]
    fn rfc1321_empty_string() {
        assert_eq!(
            hex(&Md5::digest(b"")),
            "d41d8cd98f00b204e9800998ecf8427e"
        );
    }

    #[test]
    fn rfc1321_single_a() {
        assert_eq!(
            hex(&Md5::digest(b"a")),
            "0cc175b9c0f1b6a831c399e269772661"
        );
    }

    #[test]
    fn rfc1321_abc() {
        assert_eq!(
            hex(&Md5::digest(b"abc")),
            "900150983cd24fb0d6963f7d28e17f72"
        );
    }

    #[test]
    fn rfc1321_message_digest() {
        assert_eq!(
            hex(&Md5::digest(b"message digest")),
            "f96b697d7cb7938d525a2f31aaf161d0"
        );
    }

    #[test]
    fn rfc1321_alphabet() {
        assert_eq!(
            hex(&Md5::digest(b"abcdefghijklmnopqrstuvwxyz")),
            "c3fcd3d76192e4007dfb496cca67e13b"
        );
    }

    #[test]
    fn rfc1321_mixed_alnum() {
        assert_eq!(
            hex(&Md5::digest(
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
            )),
            "d174ab98d277d9f5a5611c2c9f419d9f"
        );
    }

    #[test]
    fn rfc1321_long_digits() {
        let input = "1234567890".repeat(8);
        assert_eq!(
            hex(&Md5::digest(input.as_bytes())),
            "57edf4a22be3c955ac49da2e2107b67a"
        );
    }

    /// A block boundary at exactly 56 bytes forces a second padding block,
    /// exercising the `buffer_len > 56` branch in `finalize`.
    #[test]
    fn input_spanning_block_boundary() {
        // Reference values cross-checked against `Get-FileHash -Algorithm MD5`.
        let input = vec![b'a'; 56];
        assert_eq!(
            hex(&Md5::digest(&input)),
            "3b0c8ac703f828b04c6c197006d17218"
        );

        let input = vec![b'a'; 64];
        assert_eq!(
            hex(&Md5::digest(&input)),
            "014842d480b571495a4a0363793f7367"
        );
    }

    /// Stream the same data in many small chunks; the result must match.
    #[test]
    fn streaming_matches_one_shot() {
        let data = b"The quick brown fox jumps over the lazy dog";
        let one_shot = hex(&Md5::digest(data));

        let mut hasher = Md5::new();
        for byte in data {
            hasher.update(std::slice::from_ref(byte));
        }
        assert_eq!(hex(&hasher.finalize()), one_shot);
    }

    #[test]
    fn hex_is_lowercase() {
        let digest = Md5::digest(b"abc");
        assert_eq!(hex(&digest), "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn format_default_bsd_style() {
        assert_eq!(
            format_output("file.txt", "abc123", false, false),
            "MD5 (file.txt) = abc123"
        );
    }

    #[test]
    fn format_quiet_is_hash_only() {
        assert_eq!(format_output("file.txt", "abc123", true, false), "abc123");
    }

    #[test]
    fn format_reverse_is_hash_first() {
        assert_eq!(
            format_output("file.txt", "abc123", false, true),
            "abc123  file.txt"
        );
    }

    #[test]
    fn parse_gnu_text_format() {
        let (name, hash) = parse_checksum_line(
            "900150983cd24fb0d6963f7d28e17f72  hello.txt",
        )
        .expect("GNU text line");
        assert_eq!(name, "hello.txt");
        assert_eq!(hash, "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn parse_gnu_binary_format() {
        let (name, hash) =
            parse_checksum_line("900150983cd24fb0d6963f7d28e17f72 *hello.txt").unwrap();
        assert_eq!(name, "hello.txt");
        assert_eq!(hash, "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn parse_bsd_format() {
        let (name, hash) =
            parse_checksum_line("MD5 (hello.txt) = 900150983cd24fb0d6963f7d28e17f72").unwrap();
        assert_eq!(name, "hello.txt");
        assert_eq!(hash, "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn parse_bsd_format_case_insensitive_prefix() {
        let (name, _) = parse_checksum_line(
            "md5 (hello.txt) = 900150983cd24fb0d6963f7d28e17f72",
        )
        .unwrap();
        assert_eq!(name, "hello.txt");
    }

    #[test]
    fn parse_uppercase_hex_is_lowercased() {
        let (_, hash) =
            parse_checksum_line("MD5 (x) = 900150983CD24FB0D6963F7D28E17F72").unwrap();
        assert_eq!(hash, "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn parse_name_with_spaces_is_preserved() {
        let (name, _) =
            parse_checksum_line("MD5 (my file.txt) = 900150983cd24fb0d6963f7d28e17f72").unwrap();
        assert_eq!(name, "my file.txt");
    }

    #[test]
    fn parse_rejects_malformed_lines() {
        assert!(parse_checksum_line("").is_none());
        assert!(parse_checksum_line("not a checksum").is_none());
        assert!(parse_checksum_line("ZZ0150983cd24fb0d6963f7d28e17f72  x").is_none());
        assert!(parse_checksum_line("MD5 (x) = tooShort").is_none());
    }

    #[test]
    fn parse_keeps_internal_spaces_verbatim() {
        // Names are taken verbatim; surrounding whitespace is not trimmed away.
        let (name, _) =
            parse_checksum_line("900150983cd24fb0d6963f7d28e17f72  my  file .txt").unwrap();
        assert_eq!(name, "my  file .txt");
    }

    #[test]
    fn parse_rejects_leading_whitespace() {
        // Leading whitespace shifts the hash off column 0, so the line is malformed.
        assert!(parse_checksum_line(
            " 900150983cd24fb0d6963f7d28e17f72  hello.txt"
        )
        .is_none());
    }

    #[test]
    fn parse_rejects_trailing_whitespace_after_hash() {
        // A stray trailing space changes the hash field length -> malformed.
        assert!(parse_checksum_line(
            "MD5 (x) = 900150983cd24fb0d6963f7d28e17f72 "
        )
        .is_none());
    }

    #[test]
    fn parse_string_takes_next_argument() {
        let opts =
            parse_args([OsString::from("-s"), OsString::from("hello")]).unwrap();
        assert_eq!(opts.string.as_deref(), Some(OsStr::new("hello")));
    }

    /// When `-s` is set, string mode takes precedence and file arguments are
    /// collected but never hashed.
    #[test]
    fn string_mode_has_precedence_over_files() {
        let opts = parse_args([
            OsString::from("-s"),
            OsString::from("hello"),
            OsString::from("ignored.txt"),
        ])
        .unwrap();
        assert_eq!(opts.string.as_deref(), Some(OsStr::new("hello")));
        assert_eq!(opts.files, vec![OsString::from("ignored.txt")]);

        assert_eq!(run(&opts), 0);
    }

    #[test]
    fn parse_string_equals_form() {
        let opts = parse_args(std::iter::once(OsString::from("--string=hi"))).unwrap();
        assert_eq!(opts.string.as_deref(), Some(OsStr::new("hi")));
    }

    #[test]
    fn parse_check_takes_next_argument() {
        let opts =
            parse_args([OsString::from("-c"), OsString::from("sums.md5")]).unwrap();
        assert_eq!(opts.check.as_deref(), Some(OsStr::new("sums.md5")));
    }

    #[test]
    fn parse_missing_string_value_errors() {
        assert!(parse_args(std::iter::once(OsString::from("-s"))).is_err());
    }

    #[test]
    fn parse_unknown_option_errors() {
        assert!(parse_args(std::iter::once(OsString::from("-z"))).is_err());
    }

    #[test]
    fn parse_double_dash_treats_rest_as_files() {
        let opts = parse_args(
            ["--".to_string(), "-q".to_string()]
                .into_iter()
                .map(OsString::from),
        )
        .unwrap();
        assert!(!opts.quiet);
        assert_eq!(opts.files, vec![OsString::from("-q")]);
    }

    #[test]
    fn parse_flag_then_files() {
        let opts = parse_args([
            OsString::from("-q"),
            OsString::from("a.txt"),
            OsString::from("b.txt"),
        ])
        .unwrap();
        assert!(opts.quiet);
        assert_eq!(
            opts.files,
            vec![OsString::from("a.txt"), OsString::from("b.txt")]
        );
    }

    #[test]
    fn parse_clusters_short_flags() {
        let opts = parse_args([OsString::from("-qr"), OsString::from("a.txt")]).unwrap();
        assert!(opts.quiet);
        assert!(opts.reverse);
        assert_eq!(opts.files, vec![OsString::from("a.txt")]);
    }

    #[test]
    fn parse_short_flag_with_attached_argument() {
        let opts = parse_args([OsString::from("-shello")]).unwrap();
        assert_eq!(opts.string.as_deref(), Some(OsStr::new("hello")));
    }

    #[test]
    fn parse_short_flag_consuming_next_argument() {
        let opts =
            parse_args([OsString::from("-qs"), OsString::from("hello")]).unwrap();
        assert!(opts.quiet);
        assert_eq!(opts.string.as_deref(), Some(OsStr::new("hello")));
    }

    #[test]
    fn parse_unknown_char_in_cluster_errors() {
        assert!(parse_args([OsString::from("-qz")]).is_err());
    }

    /// End-to-end check mode against a temp directory, exercising the real
    /// file hashing path and the OK / FAILED reporting.
    #[test]
    fn check_mode_reports_matches_and_mismatches() {
        let root = test_dir("check");
        std::fs::create_dir_all(&root).unwrap();
        let good = root.join("good.txt");
        let bad = root.join("bad.txt");
        std::fs::write(&good, b"abc").unwrap();
        std::fs::write(&bad, b"abc").unwrap();

        let good_hash = hex(&Md5::digest(b"abc")); // 900150983cd24fb0d6963f7d28e17f72
        let wrong_hash = "00000000000000000000000000000000";

        let sums = root.join("sums.md5");
        let content = format!(
            "MD5 ({}) = {good_hash}\n\
             MD5 ({}) = {wrong_hash}\n",
            good.display(),
            bad.display()
        );
        std::fs::write(&sums, content).unwrap();

        let opts = Options {
            check: Some(sums.into_os_string()),
            quiet: false,
            ..Options::default()
        };
        let code = run(&opts);
        assert_eq!(code, 1, "one mismatch should yield exit code 1");

        std::fs::remove_dir_all(root).unwrap();
    }

    /// A malformed line mixed with a matching entry must still fail.
    #[test]
    fn check_mode_fails_on_malformed_lines() {
        let root = test_dir("malformed");
        std::fs::create_dir_all(&root).unwrap();
        let good = root.join("good.txt");
        std::fs::write(&good, b"abc").unwrap();
        let good_hash = hex(&Md5::digest(b"abc"));

        let sums = root.join("sums.md5");
        let content = format!(
            "MD5 ({}) = {good_hash}\n\
             this is not a checksum line\n",
            good.display()
        );
        std::fs::write(&sums, content).unwrap();

        let opts = Options {
            check: Some(sums.into_os_string()),
            ..Options::default()
        };
        assert_eq!(run(&opts), 1, "a malformed line must fail verification");

        std::fs::remove_dir_all(root).unwrap();
    }

    /// All matching entries and no malformed lines -> exit 0.
    #[test]
    fn check_mode_passes_when_all_match() {
        let root = test_dir("allmatch");
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("ok.txt");
        std::fs::write(&file, b"abc").unwrap();
        let hash = hex(&Md5::digest(b"abc"));

        let sums = root.join("sums.md5");
        std::fs::write(&sums, format!("MD5 ({}) = {hash}\n", file.display())).unwrap();

        let opts = Options {
            check: Some(sums.into_os_string()),
            ..Options::default()
        };
        assert_eq!(run(&opts), 0);

        std::fs::remove_dir_all(root).unwrap();
    }

    fn test_dir(name: &str) -> std::path::PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("md5-test-{name}-{nonce}"))
    }
}

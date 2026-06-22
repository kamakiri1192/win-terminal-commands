use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
struct Options {
    app: Option<OsString>,
    targets: Vec<OsString>,
}

fn main() {
    let options = match parse_args(env::args_os().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("open: {message}");
            print_usage();
            std::process::exit(2);
        }
    };

    let mut failed = false;

    match options.app {
        Some(app) => {
            // Launch the chosen application, passing every target as a parameter.
            let resolved = resolve_app(&app).unwrap_or(app);
            let params = if options.targets.is_empty() {
                None
            } else {
                Some(build_parameters(&options.targets))
            };
            if !launch(&resolved, None, params.as_deref()) {
                failed = true;
            }
        }
        None => {
            if options.targets.is_empty() {
                // Mirror macOS `open` with no arguments: open the current directory.
                let here = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                if !launch(here.as_os_str(), Some("open"), None) {
                    failed = true;
                }
            } else {
                for target in &options.targets {
                    if !launch(target, Some("open"), None) {
                        failed = true;
                    }
                }
            }
        }
    }

    std::process::exit(if failed { 1 } else { 0 });
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
            if arg == "-a" || arg == "--app" {
                let value = iter
                    .next()
                    .ok_or_else(|| "option -a requires an argument".to_string())?;
                options.app = Some(value);
                continue;
            }

            if let Some(text) = arg.to_str() {
                if let Some(rest) = text.strip_prefix("--app=") {
                    options.app = Some(OsString::from(rest));
                    continue;
                }

                if text == "-h" || text == "--help" {
                    return Err("help requested".to_string());
                }

                if text.starts_with('-') && text != "-" {
                    return Err(format!("unknown option: {text}"));
                }
            }
        }

        options.targets.push(arg);
    }

    Ok(options)
}

fn print_usage() {
    eprintln!("usage: open [-a app|--app app|--app=app] [file ...]");
    eprintln!("       with no arguments, opens the current directory");
}

/// Join multiple targets into a single command-line parameter string,
/// quoting each one the way the launched application will re-parse it.
fn build_parameters(targets: &[OsString]) -> String {
    targets
        .iter()
        .map(|target| quote_arg(&target.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Quote an argument following the MSDN "Parsing C Command-Line Arguments"
/// rules so that the target application receives it verbatim.
fn quote_arg(input: &str) -> String {
    let needs_quotes = input.is_empty()
        || input
            .chars()
            .any(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '"'));

    if !needs_quotes {
        return input.to_string();
    }

    let mut out = String::from('"');
    let mut backslashes = 0usize;

    for ch in input.chars() {
        match ch {
            // Accumulate backslashes; emit them once we know what follows.
            '\\' => backslashes += 1,
            '"' => {
                // Doubled backslashes plus one that escapes the quote itself.
                for _ in 0..backslashes * 2 + 1 {
                    out.push('\\');
                }
                out.push('"');
                backslashes = 0;
            }
            other => {
                for _ in 0..backslashes {
                    out.push('\\');
                }
                backslashes = 0;
                out.push(other);
            }
        }
    }

    // Trailing backslashes must be doubled so they cannot escape the closing quote.
    for _ in 0..backslashes * 2 {
        out.push('\\');
    }
    out.push('"');
    out
}

/// Resolve a bare application name against `PATH` and `PATHEXT`.
/// Returns `None` for explicit paths or when nothing is found, leaving the
/// original name for `ShellExecuteW` to handle.
fn resolve_app(app: &OsStr) -> Option<OsString> {
    let path = Path::new(app);
    if has_path_separator(path) {
        return None;
    }

    let path_entries: Vec<PathBuf> = env::var_os("PATH")
        .map(|paths| env::split_paths(&paths).collect())
        .unwrap_or_default();

    for entry in &path_entries {
        let direct = entry.join(app);
        if direct.is_file() {
            return Some(direct.into_os_string());
        }

        for ext in path_exts() {
            let mut name = app.to_os_string();
            name.push(ext);
            let candidate = entry.join(&name);
            if candidate.is_file() {
                return Some(candidate.into_os_string());
            }
        }
    }

    None
}

fn path_exts() -> Vec<OsString> {
    let raw = env::var_os("PATHEXT").unwrap_or_else(|| OsString::from(".COM;.EXE;.BAT;.CMD"));
    env::split_paths(&raw)
        .filter_map(|ext| {
            let text = ext.as_os_str().to_string_lossy();
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else if trimmed.starts_with('.') {
                Some(OsString::from(trimmed))
            } else {
                Some(OsString::from(format!(".{trimmed}")))
            }
        })
        .collect()
}

fn has_path_separator(path: &Path) -> bool {
    let text = path.as_os_str().to_string_lossy();
    text.contains('/') || text.contains('\\')
}

#[cfg(windows)]
#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        hwnd: isize,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show_cmd: i32,
    ) -> isize;
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn GetLastError() -> u32;
}

#[cfg(windows)]
fn launch(file: &OsStr, verb: Option<&str>, params: Option<&str>) -> bool {
    use std::os::windows::ffi::OsStrExt;

    fn wide(input: &OsStr) -> Vec<u16> {
        input.encode_wide().chain(std::iter::once(0)).collect()
    }

    fn wide_from_str(input: &str) -> Vec<u16> {
        input.encode_utf16().chain(std::iter::once(0)).collect()
    }

    let file_w = wide(file);
    let verb_w = verb.map(wide_from_str);
    let params_w = params.map(wide_from_str);

    let verb_ptr = verb_w.as_ref().map_or(std::ptr::null(), |v| v.as_ptr());
    let params_ptr = params_w.as_ref().map_or(std::ptr::null(), |v| v.as_ptr());

    // SW_SHOWNORMAL = 1
    let result = unsafe {
        ShellExecuteW(0, verb_ptr, file_w.as_ptr(), params_ptr, std::ptr::null(), 1)
    };

    if (result as usize) <= 32 {
        let err = unsafe { GetLastError() };
        eprintln!(
            "open: cannot open {}: Windows error {}",
            file.to_string_lossy(),
            err
        );
        false
    } else {
        true
    }
}

#[cfg(not(windows))]
fn launch(_file: &OsStr, _verb: Option<&str>, _params: Option<&str>) -> bool {
    eprintln!("open: this command is only supported on Windows");
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_plain_argument() {
        assert_eq!(quote_arg("abc"), "abc");
    }

    #[test]
    fn quote_argument_with_space() {
        assert_eq!(quote_arg("a b"), "\"a b\"");
    }

    #[test]
    fn quote_empty_argument() {
        assert_eq!(quote_arg(""), "\"\"");
    }

    #[test]
    fn quote_argument_with_embedded_quote() {
        assert_eq!(quote_arg("a\"b"), "\"a\\\"b\"");
    }

    #[test]
    fn quote_argument_without_spaces_is_untouched() {
        // No spaces, so no quoting is needed even with a trailing backslash.
        assert_eq!(quote_arg("C:\\dir\\"), "C:\\dir\\");
    }

    #[test]
    fn quote_argument_doubles_trailing_backslash() {
        assert_eq!(quote_arg("C:\\my dir\\"), "\"C:\\my dir\\\\\"");
    }

    #[test]
    fn parse_app_option_takes_next_argument() {
        let opts = parse_args(
            [
                OsString::from("-a"),
                OsString::from("notepad"),
                OsString::from("a.txt"),
            ]
            .into_iter(),
        )
        .unwrap();
        assert_eq!(opts.app.as_deref(), Some(OsStr::new("notepad")));
        assert_eq!(opts.targets, vec![OsString::from("a.txt")]);
    }

    #[test]
    fn parse_app_equals_form() {
        let opts = parse_args(
            [OsString::from("--app=notepad"), OsString::from("a.txt")].into_iter(),
        )
        .unwrap();
        assert_eq!(opts.app.as_deref(), Some(OsStr::new("notepad")));
    }

    #[test]
    fn parse_missing_app_value_errors() {
        assert!(parse_args(std::iter::once(OsString::from("-a"))).is_err());
    }

    #[test]
    fn parse_double_dash_treats_rest_as_targets() {
        let opts = parse_args(
            ["--".to_string(), "-a".to_string()]
                .into_iter()
                .map(OsString::from),
        )
        .unwrap();
        assert!(opts.app.is_none());
        assert_eq!(opts.targets, vec![OsString::from("-a")]);
    }
}

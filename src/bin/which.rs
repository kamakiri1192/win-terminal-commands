use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
struct Options {
    all: bool,
    silent: bool,
    commands: Vec<OsString>,
}

fn main() {
    let options = match parse_args(env::args_os().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("which: {message}");
            print_usage();
            std::process::exit(2);
        }
    };

    if options.commands.is_empty() {
        print_usage();
        std::process::exit(2);
    }

    let path_entries: Vec<PathBuf> = env::var_os("PATH")
        .map(|paths| env::split_paths(&paths).collect())
        .unwrap_or_default();
    let path_exts = path_exts();

    let mut missing = false;
    for command in &options.commands {
        let matches = find_command(command, &path_entries, &path_exts, options.all);

        if matches.is_empty() {
            missing = true;
            continue;
        }

        if !options.silent {
            for path in matches {
                println!("{}", path.display());
            }
        }
    }

    std::process::exit(if missing { 1 } else { 0 });
}

fn parse_args<I>(args: I) -> Result<Options, String>
where
    I: IntoIterator<Item = OsString>,
{
    let mut options = Options::default();
    let mut parse_flags = true;

    for arg in args {
        if parse_flags && arg == "--" {
            parse_flags = false;
            continue;
        }

        if parse_flags {
            if arg == "-a" || arg == "--all" {
                options.all = true;
                continue;
            }

            if arg == "-s" || arg == "--silent" {
                options.silent = true;
                continue;
            }

            if arg == "-h" || arg == "--help" {
                return Err("help requested".to_string());
            }

            if let Some(text) = arg.to_str() {
                if text.starts_with('-') && text != "-" {
                    return Err(format!("unknown option: {text}"));
                }
            }
        }

        options.commands.push(arg);
    }

    Ok(options)
}

fn print_usage() {
    eprintln!("usage: which [-a|--all] [-s|--silent] command ...");
}

fn find_command(
    command: &OsString,
    path_entries: &[PathBuf],
    path_exts: &[OsString],
    all: bool,
) -> Vec<PathBuf> {
    let command_path = Path::new(command);
    let candidates = if has_path_separator(command_path) {
        candidate_paths(command_path, path_exts)
    } else {
        path_entries
            .iter()
            .flat_map(|entry| candidate_paths(&entry.join(command_path), path_exts))
            .collect()
    };

    let mut seen = HashSet::new();
    let mut matches = Vec::new();

    for candidate in candidates {
        if !is_runnable_file(&candidate) {
            continue;
        }

        let key = normalize_for_dedupe(&candidate);
        if !seen.insert(key) {
            continue;
        }

        matches.push(candidate);
        if !all {
            break;
        }
    }

    matches
}

fn candidate_paths(base: &Path, path_exts: &[OsString]) -> Vec<PathBuf> {
    let mut candidates = vec![base.to_path_buf()];

    if base.extension().is_none() {
        for ext in path_exts {
            let mut name = base.as_os_str().to_os_string();
            name.push(ext);
            candidates.push(PathBuf::from(name));
        }
    }

    candidates
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
fn is_runnable_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(not(windows))]
fn is_runnable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(windows)]
fn normalize_for_dedupe(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

#[cfg(not(windows))]
fn normalize_for_dedupe(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn finds_first_matching_path_with_pathext() {
        let root = test_dir("first");
        let bin = root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("tool.EXE"), "").unwrap();

        let matches = find_command(
            &OsString::from("tool"),
            std::slice::from_ref(&bin),
            &[OsString::from(".EXE")],
            false,
        );

        assert_eq!(matches, vec![bin.join("tool.EXE")]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn all_returns_every_matching_path() {
        let root = test_dir("all");
        let one = root.join("one");
        let two = root.join("two");
        fs::create_dir_all(&one).unwrap();
        fs::create_dir_all(&two).unwrap();
        fs::write(one.join("tool.cmd"), "").unwrap();
        fs::write(two.join("tool.cmd"), "").unwrap();

        let matches = find_command(
            &OsString::from("tool"),
            &[one.clone(), two.clone()],
            &[OsString::from(".cmd")],
            true,
        );

        assert_eq!(matches, vec![one.join("tool.cmd"), two.join("tool.cmd")]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn explicit_path_is_checked_without_path_search() {
        let root = test_dir("explicit");
        fs::create_dir_all(&root).unwrap();
        let executable = root.join("tool.exe");
        fs::write(&executable, "").unwrap();

        let matches = find_command(
            &OsString::from(root.join("tool").as_os_str()),
            &[PathBuf::from("unused")],
            &[OsString::from(".exe")],
            false,
        );

        assert_eq!(matches, vec![executable]);
        fs::remove_dir_all(root).unwrap();
    }

    fn test_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("which-test-{name}-{nonce}"))
    }
}

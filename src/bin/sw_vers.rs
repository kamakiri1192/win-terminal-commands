use std::env;
use std::ffi::OsString;

/// Which single line of version information to print.
///
/// On macOS these flags are mutually exclusive: `sw_vers` lets you ask for just
/// one field instead of the default three-line summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    ProductName,
    ProductVersion,
    ProductVersionExtra,
    BuildVersion,
}

#[derive(Debug, Default)]
struct Options {
    field: Option<Field>,
    help: bool,
}

/// Resolved version information, formatted the way the macOS-style fields expect.
#[derive(Debug, Clone, PartialEq, Eq)]
struct VersionInfo {
    product_name: String,
    product_version: String,
    build_version: String,
    product_version_extra: String,
}

fn main() {
    let options = match parse_args(env::args_os().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("sw_vers: {message}");
            eprint!("{}", usage());
            std::process::exit(1);
        }
    };

    if options.help {
        // macOS `sw_vers` has no documented help flag, but `-h`/`--help` is a
        // harmless convenience that many users expect.
        print!("{}", usage());
        std::process::exit(0);
    }

    let info = match gather_version_info() {
        Ok(info) => info,
        Err(message) => {
            eprintln!("sw_vers: {message}");
            std::process::exit(1);
        }
    };

    print!("{}", render(options.field, &info));
    std::process::exit(0);
}

fn parse_args<I>(args: I) -> Result<Options, String>
where
    I: IntoIterator<Item = OsString>,
{
    let mut options = Options::default();

    for arg in args {
        let text = arg.to_str();
        match text {
            Some("-productName") => set_field(&mut options, Field::ProductName)?,
            Some("-productVersion") => set_field(&mut options, Field::ProductVersion)?,
            Some("-productVersionExtra") => set_field(&mut options, Field::ProductVersionExtra)?,
            Some("-buildVersion") => set_field(&mut options, Field::BuildVersion)?,
            Some("-h") | Some("-help") | Some("--help") => options.help = true,
            Some(other) if other.starts_with('-') && other != "-" => {
                return Err(format!("unknown option: {other}"));
            }
            Some(_) => {
                return Err(format!("unexpected argument: {}", arg.to_string_lossy()));
            }
            None => return Err("argument is not valid Unicode".to_string()),
        }
    }

    Ok(options)
}

fn set_field(options: &mut Options, field: Field) -> Result<(), String> {
    if options.field.is_some() {
        return Err(
            "only one of -productName, -productVersion, -productVersionExtra or \
                    -buildVersion may be specified"
                .to_string(),
        );
    }
    options.field = Some(field);
    Ok(())
}

/// Build the text to write to stdout for the chosen field (or the full summary).
fn render(field: Option<Field>, info: &VersionInfo) -> String {
    let mut out = String::new();
    match field {
        Some(Field::ProductName) => {
            out.push_str(&info.product_name);
            out.push('\n');
        }
        Some(Field::ProductVersion) => {
            out.push_str(&info.product_version);
            out.push('\n');
        }
        Some(Field::ProductVersionExtra) => {
            out.push_str(&info.product_version_extra);
            out.push('\n');
        }
        Some(Field::BuildVersion) => {
            out.push_str(&info.build_version);
            out.push('\n');
        }
        None => {
            // macOS prints the default summary with a literal tab between each
            // label and its value, e.g. `ProductName:\tmacOS`.
            out.push_str(&format!("ProductName:\t{}\n", info.product_name));
            out.push_str(&format!("ProductVersion:\t{}\n", info.product_version));
            out.push_str(&format!("BuildVersion:\t{}\n", info.build_version));
        }
    }
    out
}

fn usage() -> String {
    String::from(
        "usage: sw_vers [-productName | -productVersion | -productVersionExtra | -buildVersion]\n",
    )
}

#[cfg(windows)]
fn gather_version_info() -> Result<VersionInfo, String> {
    use windows as win;

    // macOS exposes four fields. Windows has no direct equivalents, so each is
    // mapped to the closest authoritative source:
    //
    //   ProductName          -> "Windows" (the OS family, mirroring macOS's "macOS")
    //   ProductVersion       -> Major.Minor.Build from RtlGetVersion (e.g. "10.0.22631")
    //   BuildVersion         -> Build.UBR, matching `winver`'s "OS Build" (e.g. "22631.4890")
    //   productVersionExtra  -> DisplayVersion / feature update (e.g. "23H2").
    //                          Windows has no Rapid Security Response, so the feature
    //                          release is the closest "extra version" qualifier.

    // RtlGetVersion returns the true OS version regardless of the application
    // manifest (unlike GetVersionEx, which under-reports without a manifest).
    let (major, minor, build) = win::os_version_numbers()
        .ok_or_else(|| "could not determine the Windows version via RtlGetVersion".to_string())?;

    let product_version = format!("{major}.{minor}.{build}");

    let build_version = match win::reg_dword("UBR") {
        Some(ubr) => format!("{build}.{ubr}"),
        None => build.to_string(),
    };

    // `DisplayVersion` (e.g. "23H2") only exists from Windows 10 2004 onward;
    // older builds and some LTSC images use `ReleaseId` (e.g. "21H2", "1909")
    // for the same purpose, so fall back to it before settling on an empty value.
    let product_version_extra = win::reg_string("DisplayVersion")
        .or_else(|| win::reg_string("ReleaseId"))
        .unwrap_or_default();

    Ok(VersionInfo {
        product_name: "Windows".to_string(),
        product_version,
        build_version,
        product_version_extra,
    })
}

#[cfg(not(windows))]
fn gather_version_info() -> Result<VersionInfo, String> {
    Err("this command is only supported on Windows".to_string())
}

#[cfg(windows)]
mod windows {
    //! Minimal Win32 FFI to read the OS version without pulling in crates.
    //!
    //! `RtlGetVersion` (ntdll) gives the accurate major/minor/build; the registry
    //! supplies the update build revision (UBR) and display version that the API
    //! does not expose.

    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    const HKEY_LOCAL_MACHINE: isize = 0x8000_0002u32 as isize;

    // RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ, so either string kind is accepted and
    // REG_EXPAND_SZ is expanded automatically by RegGetValueW.
    const REG_STRING_FLAGS: u32 = 0x0000_0002 | 0x0000_0004;
    const REG_DWORD_FLAGS: u32 = 0x0000_0010;

    const CURRENT_VERSION_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

    #[repr(C)]
    struct OsVersionInfoExW {
        os_version_info_size: u32,
        major_version: u32,
        minor_version: u32,
        build_number: u32,
        sz_csd_version: [u16; 128],
        service_pack_major: u16,
        service_pack_minor: u16,
        suite_mask: u16,
        product_type: u8,
        reserved: u8,
    }

    impl Default for OsVersionInfoExW {
        fn default() -> Self {
            Self {
                os_version_info_size: 0,
                major_version: 0,
                minor_version: 0,
                build_number: 0,
                sz_csd_version: [0u16; 128],
                service_pack_major: 0,
                service_pack_minor: 0,
                suite_mask: 0,
                product_type: 0,
                reserved: 0,
            }
        }
    }

    impl OsVersionInfoExW {
        // Matches sizeof(OSVERSIONINFOEXW) on Windows (280 bytes).
        const SIZE: u32 = std::mem::size_of::<Self>() as u32;
    }

    #[link(name = "ntdll")]
    extern "system" {
        fn RtlGetVersion(info: *mut OsVersionInfoExW) -> i32;
    }

    #[link(name = "advapi32")]
    extern "system" {
        fn RegGetValueW(
            hkey: isize,
            sub_key: *const u16,
            value: *const u16,
            flags: u32,
            kind: *mut u32,
            data: *mut u8,
            size: *mut u32,
        ) -> i32;
    }

    fn wide(input: &str) -> Vec<u16> {
        input.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// `(major, minor, build)` from `RtlGetVersion`, or `None` on failure.
    pub fn os_version_numbers() -> Option<(u32, u32, u32)> {
        let mut info = OsVersionInfoExW {
            os_version_info_size: OsVersionInfoExW::SIZE,
            ..Default::default()
        };
        // STATUS_SUCCESS == 0
        let status = unsafe { RtlGetVersion(&mut info) };
        if status == 0 {
            Some((info.major_version, info.minor_version, info.build_number))
        } else {
            None
        }
    }

    /// Read a `REG_SZ`/`REG_EXPAND_SZ` value from `CurrentVersion`.
    pub fn reg_string(value: &str) -> Option<String> {
        let sub_key = wide(CURRENT_VERSION_KEY);
        let value_name = wide(value);

        // Enough for the short string values we read here (build, display version, ...).
        let mut buffer = [0u8; 1024];
        let mut size = buffer.len() as u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                sub_key.as_ptr(),
                value_name.as_ptr(),
                REG_STRING_FLAGS,
                std::ptr::null_mut(),
                buffer.as_mut_ptr(),
                &mut size,
            )
        };
        if status != 0 {
            return None;
        }

        let units: Vec<u16> = (0..size as usize / 2)
            .map(|i| u16::from_le_bytes([buffer[i * 2], buffer[i * 2 + 1]]))
            .collect();
        let end = units.iter().position(|&c| c == 0).unwrap_or(units.len());
        OsString::from_wide(&units[..end])
            .into_string()
            .ok()
            .filter(|s| !s.is_empty())
    }

    /// Read a `REG_DWORD` value from `CurrentVersion`.
    pub fn reg_dword(value: &str) -> Option<u32> {
        let sub_key = wide(CURRENT_VERSION_KEY);
        let value_name = wide(value);

        let mut data = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                sub_key.as_ptr(),
                value_name.as_ptr(),
                REG_DWORD_FLAGS,
                std::ptr::null_mut(),
                &mut data as *mut u32 as *mut u8,
                &mut size,
            )
        };
        if status == 0 {
            Some(data)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> VersionInfo {
        VersionInfo {
            product_name: "Windows".to_string(),
            product_version: "10.0.22631".to_string(),
            build_version: "22631.4890".to_string(),
            product_version_extra: "23H2".to_string(),
        }
    }

    fn parse(args: &[&str]) -> Result<Options, String> {
        parse_args(args.iter().map(|s| OsString::from(*s)))
    }

    #[test]
    fn render_summary_uses_tabs_like_macos() {
        let out = render(None, &sample());
        assert_eq!(
            out,
            "ProductName:\tWindows\nProductVersion:\t10.0.22631\nBuildVersion:\t22631.4890\n"
        );
    }

    #[test]
    fn render_single_field_appends_newline() {
        let info = sample();
        assert_eq!(render(Some(Field::ProductName), &info), "Windows\n");
        assert_eq!(render(Some(Field::ProductVersion), &info), "10.0.22631\n");
        assert_eq!(render(Some(Field::ProductVersionExtra), &info), "23H2\n");
        assert_eq!(render(Some(Field::BuildVersion), &info), "22631.4890\n");
    }

    #[test]
    fn parse_each_option_sets_its_field() {
        assert_eq!(
            parse(&["-productName"]).unwrap().field,
            Some(Field::ProductName)
        );
        assert_eq!(
            parse(&["-productVersion"]).unwrap().field,
            Some(Field::ProductVersion)
        );
        assert_eq!(
            parse(&["-productVersionExtra"]).unwrap().field,
            Some(Field::ProductVersionExtra)
        );
        assert_eq!(
            parse(&["-buildVersion"]).unwrap().field,
            Some(Field::BuildVersion)
        );
    }

    #[test]
    fn parse_no_arguments_prints_summary() {
        assert_eq!(parse(&[]).unwrap().field, None);
    }

    #[test]
    fn parse_multiple_options_are_rejected() {
        assert!(parse(&["-productName", "-buildVersion"]).is_err());
        assert!(parse(&["-productVersion", "-productVersionExtra"]).is_err());
    }

    #[test]
    fn parse_unknown_option_is_rejected() {
        assert!(parse(&["-version"]).is_err());
        assert!(parse(&["--bad"]).is_err());
    }

    #[test]
    fn parse_unexpected_positional_is_rejected() {
        assert!(parse(&["foo"]).is_err());
    }

    #[test]
    fn parse_help_flags() {
        assert!(parse(&["-h"]).unwrap().help);
        assert!(parse(&["-help"]).unwrap().help);
        assert!(parse(&["--help"]).unwrap().help);
    }

    #[cfg(windows)]
    #[test]
    fn windows_version_info_is_populated() {
        let info = gather_version_info().expect("version info should resolve on Windows");
        assert_eq!(info.product_name, "Windows");
        // Major.Minor.Build, e.g. 10.0.22631
        assert!(
            info.product_version.split('.').count() >= 3,
            "product version should look like Major.Minor.Build, got {}",
            info.product_version
        );
        // Build (and UBR when present), e.g. 22631 or 22631.4890
        assert!(info.build_version.chars().next().unwrap().is_ascii_digit());
    }
}

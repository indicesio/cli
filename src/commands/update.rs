use std::env;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};
use tracing::instrument;

use crate::cli::UpdateArgs;
use crate::errors::CliError;

const GITHUB_REPO: &str = "indicesio/cli";
const USER_AGENT: &str = concat!("indices-cli/", env!("CARGO_PKG_VERSION"));
const DOWNLOAD_TIMEOUT_SECONDS: u64 = 180;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Version {
    major: u64,
    minor: u64,
    patch: u64,
}

impl Version {
    fn parse(input: &str) -> Result<Self, CliError> {
        let trimmed = input.trim();
        let without_prefix = trimmed.strip_prefix('v').unwrap_or(trimmed);
        let mut parts = without_prefix.split('.');
        let major = parse_version_part(parts.next(), input)?;
        let minor = parse_version_part(parts.next(), input)?;
        let patch = parse_version_part(parts.next(), input)?;
        if parts.next().is_some()
            || without_prefix
                .chars()
                .any(|c| !c.is_ascii_digit() && c != '.')
        {
            return Err(invalid_version(input));
        }

        Ok(Self {
            major,
            minor,
            patch,
        })
    }

    fn current() -> Result<Self, CliError> {
        Self::parse(env!("CARGO_PKG_VERSION"))
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Platform {
    os: &'static str,
    arch: &'static str,
}

impl Platform {
    fn current() -> Result<Self, CliError> {
        platform_for(env::consts::OS, env::consts::ARCH)
    }

    fn asset_name(&self, version: Version) -> String {
        format!(
            "indices_{version}_{}_{}.{}",
            self.os,
            self.arch,
            self.archive_extension()
        )
    }

    fn archive_extension(&self) -> &'static str {
        if self.os == "windows" {
            "zip"
        } else {
            "tar.gz"
        }
    }

    fn binary_name(&self) -> &'static str {
        if self.os == "windows" {
            "indices.exe"
        } else {
            "indices"
        }
    }
}

#[derive(Debug, Serialize)]
struct UpdateOutput {
    current_version: String,
    target_version: String,
    up_to_date: bool,
    updated: bool,
    path: String,
}

#[instrument(name = "cli.update", skip_all, err)]
pub async fn run(args: &UpdateArgs, timeout_seconds: u64, json: bool) -> Result<(), CliError> {
    let current_version = Version::current()?;
    let install_path = current_install_path()?;
    let client = http_client(timeout_seconds)?;
    let target_version = match args.version.as_deref() {
        Some(version) => Version::parse(version)?,
        None => latest_release_version(&client).await?,
    };
    let up_to_date = current_version == target_version
        || (args.version.is_none() && current_version > target_version);

    if args.check || up_to_date {
        return print_output(
            UpdateOutput {
                current_version: current_version.to_string(),
                target_version: target_version.to_string(),
                up_to_date,
                updated: false,
                path: install_path.display().to_string(),
            },
            args.check,
            json,
        );
    }

    if is_development_binary(&install_path) {
        return Err(CliError::Message(format!(
            "cannot update a development build at `{}`\nInstall a release with:\n  curl -fsSL https://get.indices.io | sh",
            install_path.display()
        )));
    }

    let platform = Platform::current()?;
    let bytes = download_release_binary(&client, target_version, &platform).await?;
    replace_executable(&install_path, &bytes)?;

    print_output(
        UpdateOutput {
            current_version: current_version.to_string(),
            target_version: target_version.to_string(),
            up_to_date: false,
            updated: true,
            path: install_path.display().to_string(),
        },
        false,
        json,
    )
}

fn print_output(output: UpdateOutput, check: bool, json: bool) -> Result<(), CliError> {
    if json {
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    print_human(&output, check);
    Ok(())
}

fn print_human(output: &UpdateOutput, check: bool) {
    const GREEN: &str = "\x1b[32m";
    const CYAN: &str = "\x1b[36m";
    const GREY: &str = "\x1b[90m";
    const RESET: &str = "\x1b[0m";

    if output.updated {
        println!(
            "{GREEN}✔{RESET} Updated Indices CLI from {} to {}",
            output.current_version, output.target_version
        );
        println!("{GREY}Installed to {CYAN}{}{RESET}", output.path);
        return;
    }

    if output.up_to_date {
        println!(
            "{GREEN}✔{RESET} Indices CLI is already up to date ({})",
            output.current_version
        );
        return;
    }

    if check {
        println!(
            "Update available: {} → {}",
            output.current_version, output.target_version
        );
        println!("{GREY}Run {CYAN}indices update{GREY} to install it{RESET}");
    }
}

fn parse_version_part(part: Option<&str>, original: &str) -> Result<u64, CliError> {
    let part = part.ok_or_else(|| invalid_version(original))?;
    if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
        return Err(invalid_version(original));
    }
    part.parse::<u64>().map_err(|_| invalid_version(original))
}

fn invalid_version(input: &str) -> CliError {
    CliError::Message(format!(
        "invalid version `{input}`; expected X.Y.Z or vX.Y.Z"
    ))
}

fn platform_for(os: &str, arch: &str) -> Result<Platform, CliError> {
    let os = match os {
        "linux" => "linux",
        "macos" => "darwin",
        "windows" => "windows",
        other => {
            return Err(CliError::Message(format!(
                "unsupported operating system: {other}"
            )));
        }
    };
    let arch = match arch {
        "x86_64" => "x86_64",
        "aarch64" => "arm64",
        other => {
            return Err(CliError::Message(format!(
                "unsupported architecture: {other}"
            )));
        }
    };
    if os == "windows" && arch != "x86_64" {
        return Err(CliError::Message(format!(
            "unsupported platform: {os}_{arch}"
        )));
    }

    Ok(Platform { os, arch })
}

fn http_client(timeout_seconds: u64) -> Result<reqwest::Client, CliError> {
    Ok(reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(
            timeout_seconds.max(DOWNLOAD_TIMEOUT_SECONDS),
        ))
        .build()?)
}

async fn latest_release_version(client: &reqwest::Client) -> Result<Version, CliError> {
    let response = client
        .get(format!("https://github.com/{GITHUB_REPO}/releases/latest"))
        .send()
        .await?;
    let status = response.status();
    let final_url = response.url().clone();
    if !status.is_success() {
        return Err(CliError::Message(format!(
            "failed to resolve latest release ({status})"
        )));
    }

    let tag = final_url
        .path_segments()
        .and_then(|segments| segments.last())
        .filter(|tag| !tag.is_empty())
        .ok_or_else(|| CliError::Message("failed to resolve latest release tag".to_string()))?;
    Version::parse(tag)
}

async fn download_release_binary(
    client: &reqwest::Client,
    version: Version,
    platform: &Platform,
) -> Result<Vec<u8>, CliError> {
    let tag = format!("v{version}");
    let asset_name = platform.asset_name(version);
    let checksums_name = format!("indices_{version}_checksums.txt");
    let base_url = format!("https://github.com/{GITHUB_REPO}/releases/download/{tag}");

    let checksums = download_text(client, &format!("{base_url}/{checksums_name}")).await?;
    let expected = expected_checksum(&checksums, &asset_name)
        .ok_or_else(|| CliError::Message(format!("no checksum entry found for {asset_name}")))?;
    let archive = download_bytes(client, &format!("{base_url}/{asset_name}")).await?;
    let actual = sha256_hex(&archive);
    if !actual.eq_ignore_ascii_case(&expected) {
        return Err(CliError::Message(format!(
            "checksum verification failed for {asset_name}\nExpected: {expected}\nActual:   {actual}"
        )));
    }

    extract_binary(&archive, platform)
}

async fn download_bytes(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, CliError> {
    let response = client.get(url).send().await?;
    let status = response.status();
    if status.as_u16() == 404 {
        return Err(CliError::Message(format!("release asset not found: {url}")));
    }
    if !status.is_success() {
        return Err(CliError::Message(format!(
            "failed to download {url} ({status})"
        )));
    }

    Ok(response.bytes().await?.to_vec())
}

async fn download_text(client: &reqwest::Client, url: &str) -> Result<String, CliError> {
    let bytes = download_bytes(client, url).await?;
    String::from_utf8(bytes)
        .map_err(|_| CliError::Message(format!("invalid UTF-8 in download from {url}")))
}

fn expected_checksum(checksums: &str, asset_name: &str) -> Option<String> {
    for line in checksums.lines() {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let file = parts.next()?.trim_start_matches('*');
        if file == asset_name {
            return Some(hash.to_string());
        }
    }
    None
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn extract_binary(archive: &[u8], platform: &Platform) -> Result<Vec<u8>, CliError> {
    if platform.os == "windows" {
        extract_zip_binary(archive, platform.binary_name())
    } else {
        extract_tar_gz_binary(archive, platform.binary_name())
    }
}

fn extract_tar_gz_binary(archive: &[u8], binary_name: &str) -> Result<Vec<u8>, CliError> {
    let decoder = flate2::read::GzDecoder::new(Cursor::new(archive));
    let mut tar = tar::Archive::new(decoder);
    for entry in tar.entries().map_err(archive_error)? {
        let mut entry = entry.map_err(archive_error)?;
        if entry
            .path()
            .map_err(archive_error)?
            .file_name()
            .and_then(|name| name.to_str())
            != Some(binary_name)
        {
            continue;
        }

        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        if bytes.is_empty() {
            return Err(CliError::Message(format!(
                "release archive contained an empty `{binary_name}`"
            )));
        }
        return Ok(bytes);
    }

    Err(CliError::Message(format!(
        "release archive did not contain `{binary_name}`"
    )))
}

fn extract_zip_binary(archive: &[u8], binary_name: &str) -> Result<Vec<u8>, CliError> {
    let mut zip = zip::ZipArchive::new(Cursor::new(archive))
        .map_err(|error| CliError::Message(format!("failed to read zip archive: {error}")))?;
    for index in 0..zip.len() {
        let mut file = zip.by_index(index).map_err(|error| {
            CliError::Message(format!("failed to read zip archive entry: {error}"))
        })?;
        let name = Path::new(file.name())
            .file_name()
            .and_then(|name| name.to_str());
        if name != Some(binary_name) {
            continue;
        }

        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        if bytes.is_empty() {
            return Err(CliError::Message(format!(
                "release archive contained an empty `{binary_name}`"
            )));
        }
        return Ok(bytes);
    }

    Err(CliError::Message(format!(
        "release archive did not contain `{binary_name}`"
    )))
}

fn archive_error(error: impl std::fmt::Display) -> CliError {
    CliError::Message(format!("failed to read release archive: {error}"))
}

fn current_install_path() -> Result<PathBuf, CliError> {
    let exe = env::current_exe()?;
    Ok(exe.canonicalize().unwrap_or(exe))
}

fn is_development_binary(path: &Path) -> bool {
    let parts: Vec<_> = path
        .iter()
        .filter_map(|component| component.to_str())
        .collect();
    parts
        .windows(2)
        .any(|pair| pair[0] == "target" && matches!(pair[1], "debug" | "release"))
}

fn replace_executable(dest: &Path, bytes: &[u8]) -> Result<(), CliError> {
    let file_name = dest
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            CliError::Message("could not determine the current binary name".to_string())
        })?;
    let tmp = dest.with_file_name(format!(".{file_name}.new"));
    let backup = dest.with_file_name(format!(".{file_name}.old"));

    if let Err(error) = write_executable(&tmp, bytes) {
        let _ = fs::remove_file(&tmp);
        return Err(replace_error(dest, error));
    }

    let replace_result = match fs::rename(&tmp, dest) {
        Ok(()) => Ok(()),
        Err(_) => {
            let _ = fs::remove_file(&backup);
            if let Err(error) = fs::rename(dest, &backup) {
                let _ = fs::remove_file(&tmp);
                return Err(replace_error(dest, error));
            }
            if let Err(error) = fs::rename(&tmp, dest) {
                let _ = fs::rename(&backup, dest);
                let _ = fs::remove_file(&tmp);
                return Err(replace_error(dest, error));
            }
            let _ = fs::remove_file(&backup);
            Ok(())
        }
    };

    if let Err(error) = replace_result {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }

    Ok(())
}

fn write_executable(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    fs::write(path, bytes)?;
    set_executable_permissions(path)?;
    Ok(())
}

fn replace_error(dest: &Path, error: std::io::Error) -> CliError {
    CliError::Message(format!(
        "failed to replace `{}`: {error}\nRe-run with sufficient permissions, or install to a writable directory:\n  curl -fsSL https://get.indices.io | sh",
        dest.display()
    ))
}

#[cfg(unix)]
fn set_executable_permissions(path: &Path) -> Result<(), std::io::Error> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn set_executable_permissions(_path: &Path) -> Result<(), std::io::Error> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_and_compares_versions() {
        assert_eq!(
            Version::parse("0.2.1").unwrap(),
            Version::parse("v0.2.1").unwrap()
        );
        assert!(Version::parse("0.2.10").unwrap() > Version::parse("0.2.9").unwrap());
        assert!(Version::parse("1.0.0").unwrap() > Version::parse("0.9.9").unwrap());
        assert!(Version::parse("v").is_err());
        assert!(Version::parse("1.2").is_err());
        assert!(Version::parse("1.2.3.4").is_err());
        assert!(Version::parse("1.2.3-beta").is_err());
    }

    #[test]
    fn maps_supported_platforms() {
        let linux = platform_for("linux", "x86_64").unwrap();
        assert_eq!(
            linux.asset_name(Version::parse("0.2.1").unwrap()),
            "indices_0.2.1_linux_x86_64.tar.gz"
        );
        assert_eq!(linux.binary_name(), "indices");

        let mac = platform_for("macos", "aarch64").unwrap();
        assert_eq!(
            mac.asset_name(Version::parse("0.2.0").unwrap()),
            "indices_0.2.0_darwin_arm64.tar.gz"
        );

        let windows = platform_for("windows", "x86_64").unwrap();
        assert_eq!(
            windows.asset_name(Version::parse("0.2.0").unwrap()),
            "indices_0.2.0_windows_x86_64.zip"
        );
        assert_eq!(windows.binary_name(), "indices.exe");

        assert!(platform_for("freebsd", "x86_64").is_err());
        assert!(platform_for("windows", "aarch64").is_err());
    }

    #[test]
    fn parses_checksum_entries() {
        let checksums = "\
abc  indices_0.2.1_linux_x86_64.tar.gz
def *indices_0.2.1_darwin_arm64.tar.gz
";
        assert_eq!(
            expected_checksum(checksums, "indices_0.2.1_linux_x86_64.tar.gz").as_deref(),
            Some("abc")
        );
        assert_eq!(
            expected_checksum(checksums, "indices_0.2.1_darwin_arm64.tar.gz").as_deref(),
            Some("def")
        );
        assert_eq!(
            expected_checksum(checksums, "indices_0.2.1_windows_x86_64.zip"),
            None
        );
    }

    #[test]
    fn extracts_tar_gz_binary() {
        let archive = make_tar_gz("indices", b"new-binary");
        let bytes = extract_tar_gz_binary(&archive, "indices").unwrap();
        assert_eq!(bytes, b"new-binary");
    }

    #[test]
    fn extracts_zip_binary() {
        let archive = make_zip("indices.exe", b"windows-binary");
        let bytes = extract_zip_binary(&archive, "indices.exe").unwrap();
        assert_eq!(bytes, b"windows-binary");
    }

    #[test]
    fn detects_development_binaries() {
        assert!(is_development_binary(Path::new(
            "/workspace/target/debug/indices"
        )));
        assert!(is_development_binary(Path::new(
            "C:/src/cli/target/release/indices.exe"
        )));
        assert!(!is_development_binary(Path::new(
            "/home/user/.local/bin/indices"
        )));
        assert!(!is_development_binary(Path::new(
            "/home/user/.cargo/bin/indices"
        )));
    }

    #[test]
    fn replaces_executable_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("indices");
        fs::write(&dest, b"old").unwrap();

        replace_executable(&dest, b"new").unwrap();

        assert_eq!(fs::read(&dest).unwrap(), b"new");
        assert!(!dest.with_file_name(".indices.new").exists());
        assert!(!dest.with_file_name(".indices.old").exists());
    }

    fn make_tar_gz(filename: &str, contents: &[u8]) -> Vec<u8> {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder
            .append_data(&mut header, filename, contents)
            .unwrap();
        builder.into_inner().unwrap().finish().unwrap()
    }

    fn make_zip(filename: &str, contents: &[u8]) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            writer
                .start_file(
                    filename,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .unwrap();
            writer.write_all(contents).unwrap();
            writer.finish().unwrap();
        }
        cursor.into_inner()
    }
}

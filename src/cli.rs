//! Flags that do not start the TUI: version, history, update.

use std::cmp::Ordering;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

pub const REPO_SLUG: &str = "itzMRZ/TWS-Battery-Stress-Tester-TUI";
pub const REPO_URL: &str = "https://github.com/itzMRZ/TWS-Battery-Stress-Tester-TUI";

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn print_version() {
    println!("tws-tester {}", version());
}

pub const HELP: &str = concat!(
    "tws-tester ",
    env!("CARGO_PKG_VERSION"),
    "\n\
\n\
  tws-tester              interactive TUI\n\
  tws-tester probe        capture host + Device facts for new hardware\n\
  tws-tester --history    open soak and probe folders on disk\n\
  tws-tester --update     install the latest GitHub release (SHA-256 checked)\n\
  tws-tester --version\n\
  tws-tester --help\n"
);

pub fn open_history() -> Result<()> {
    let dir = crate::pack::library_root();
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    println!("{}", dir.display());
    match open::that(&dir) {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("could not open a file manager ({e})");
            bail!("open {}", dir.display());
        }
    }
}

pub fn update() -> Result<()> {
    let dest = std::env::current_exe()
        .context("current executable")?
        .canonicalize()
        .context("resolve current executable")?;
    if dest.starts_with("/proc") {
        bail!("cannot replace {}", dest.display());
    }

    let asset = asset_name_for(std::env::consts::OS, std::env::consts::ARCH)?;
    let url = format!("{REPO_URL}/releases/latest/download/{asset}");
    let sum_url = format!("{url}.sha256");

    let work = tempfile_dir()?;
    let _guard = DeleteDir(work.clone());
    let bin = work.join(asset);
    let sum_path = work.join(format!("{asset}.sha256"));

    println!("downloading {url}");
    curl_to_file(&url, &bin)?;
    curl_to_file(&sum_url, &sum_path)?;

    let want = parse_sha256_file(&fs::read_to_string(&sum_path).context("read checksum")?)?;
    let got = sha256_file(&bin)?;
    if got != want {
        bail!("SHA-256 mismatch (got {got}, expected {want}). Left the installed binary alone.");
    }
    looks_like_native_binary(&bin)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755))?;
    }
    let reported = confirm_tws_tester(&bin)?;
    println!("SHA-256 ok");

    // Compare against the version the downloaded binary itself reports,
    // not a separate GitHub API call: one less network dependency, and
    // nothing to rate-limit or block on a restrictive network.
    let downloaded = reported
        .strip_prefix("tws-tester ")
        .and_then(parse_semver)
        .with_context(|| {
            format!("downloaded binary reported an unparseable version ({reported})")
        })?;
    let current = parse_semver(version()).expect("CARGO_PKG_VERSION is semver");
    match downloaded.cmp(&current) {
        Ordering::Equal => {
            println!("tws-tester {} is current", version());
            return Ok(());
        }
        Ordering::Less => {
            println!(
                "this binary is {} (newer than {reported}); not downgrading",
                version()
            );
            return Ok(());
        }
        Ordering::Greater => {}
    }
    println!("{reported}");

    match replace_exe(&bin, &dest) {
        Ok(()) => {
            println!("installed {}", dest.display());
            Ok(())
        }
        Err(e) => {
            let sidecar = new_sidecar(&dest);
            fs::copy(&bin, &sidecar).ok();
            bail!(
                "could not replace {} ({e}). New binary left at {}. Move it into place after this process exits.",
                dest.display(),
                sidecar.display()
            );
        }
    }
}

pub fn asset_name_for(os: &str, arch: &str) -> Result<&'static str> {
    match (os, arch) {
        ("linux", "x86_64") => Ok("tws-tester-x86_64-unknown-linux-gnu"),
        ("windows", "x86_64") => Ok("tws-tester-x86_64-pc-windows-msvc.exe"),
        _ => bail!("no GitHub binary for {os}/{arch}. Build from source: cargo build --release"),
    }
}

pub fn parse_semver(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches('v');
    let core = s.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    Some((major, minor, patch))
}

pub fn parse_sha256_file(text: &str) -> Result<String> {
    let token = text
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if token.len() != 64 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("checksum file is not a SHA-256 hex digest");
    }
    Ok(token)
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut f = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut h = Sha256::new();
    io::copy(&mut f, &mut h)?;
    Ok(format!("{:x}", h.finalize()))
}

fn looks_like_native_binary(path: &Path) -> Result<()> {
    let mut hdr = [0u8; 4];
    File::open(path)?.read_exact(&mut hdr)?;
    match std::env::consts::OS {
        "linux" if hdr == *b"\x7fELF" => Ok(()),
        "windows" if hdr[0] == b'M' && hdr[1] == b'Z' => Ok(()),
        os => bail!("downloaded file is not a {os} executable"),
    }
}

fn confirm_tws_tester(path: &Path) -> Result<String> {
    let out = Command::new(path)
        .arg("--version")
        .output()
        .context("run downloaded binary")?;
    if !out.status.success() {
        bail!("downloaded binary --version failed");
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !text.starts_with("tws-tester ") {
        bail!("downloaded binary did not print tws-tester --version");
    }
    Ok(text)
}

/// Windows Defender (and similar) can briefly hold a lock on a just-written
/// file, turning a normal open or rename into a transient "access denied" or
/// "sharing violation" error. Retry for a moment before giving up, so an
/// update does not fail just because a scanner won the race.
fn retry_io<T>(mut op: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let attempts = 20;
    let mut last_err = None;
    for attempt in 0..attempts {
        match op() {
            Ok(v) => return Ok(v),
            Err(e) if is_transient_lock(&e) && attempt + 1 < attempts => {
                last_err = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => return Err(e),
        }
    }
    Err(last_err.expect("loop always sets an error before exhausting attempts"))
}

fn is_transient_lock(e: &io::Error) -> bool {
    matches!(e.kind(), io::ErrorKind::PermissionDenied) || e.raw_os_error() == Some(32)
    // ERROR_SHARING_VIOLATION on Windows
}

fn replace_exe(src: &Path, dest: &Path) -> Result<()> {
    let parent = dest.parent().context("executable directory")?;
    let tmp = parent.join(format!(".tws-tester.new.{}", std::process::id()));
    let bak = parent.join(format!(".tws-tester.bak.{}", std::process::id()));
    {
        // A read-only handle cannot be flushed on Windows (fails with
        // access denied), so copy through a writable handle and sync
        // that same handle rather than reopening the file afterward.
        let mut source = File::open(src).with_context(|| format!("open {}", src.display()))?;
        let mut sink =
            retry_io(|| File::create(&tmp)).with_context(|| format!("write {}", tmp.display()))?;
        io::copy(&mut source, &mut sink).with_context(|| format!("write {}", tmp.display()))?;
        sink.sync_all()
            .with_context(|| format!("write {}", tmp.display()))?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
    }
    if dest.exists() {
        retry_io(|| fs::rename(dest, &bak))
            .with_context(|| format!("move {} aside (need write access)", dest.display()))?;
    }
    match retry_io(|| fs::rename(&tmp, dest)) {
        Ok(()) => {
            let _ = fs::remove_file(&bak);
            Ok(())
        }
        Err(e) => {
            if bak.exists() {
                let _ = fs::rename(&bak, dest);
            }
            let _ = fs::remove_file(&tmp);
            Err(e).with_context(|| format!("replace {}", dest.display()))
        }
    }
}

fn new_sidecar(dest: &Path) -> PathBuf {
    match dest.file_name().and_then(|s| s.to_str()) {
        Some(name) if name.ends_with(".exe") => {
            dest.with_file_name(format!("{}.new.exe", name.trim_end_matches(".exe")))
        }
        Some(name) => dest.with_file_name(format!("{name}.new")),
        None => dest.with_extension("new"),
    }
}

fn curl_bin() -> &'static str {
    if cfg!(windows) {
        "curl.exe"
    } else {
        "curl"
    }
}

fn curl_ua() -> String {
    format!("tws-tester/{} (+{REPO_URL})", version())
}

fn curl_base(cmd: &mut Command) -> &mut Command {
    cmd.args([
        "-fsSL",
        "--proto",
        "=https",
        "--tlsv1.2",
        "--retry",
        "3",
        "--max-time",
        "120",
        "--max-filesize",
        "104857600",
        "-A",
    ])
    .arg(curl_ua())
}

fn curl_fail(url: &str, err: &str) -> anyhow::Error {
    if err.contains("404") {
        anyhow::anyhow!(
            "no GitHub release yet. Tag v{} and wait for the Release workflow.",
            version()
        )
    } else {
        anyhow::anyhow!("download failed ({url}): {err}")
    }
}

fn curl_to_file(url: &str, dest: &Path) -> Result<()> {
    let mut cmd = Command::new(curl_bin());
    let out = curl_base(&mut cmd)
        .arg("-o")
        .arg(dest)
        .arg(url)
        .output()
        .context("curl (needed to download the GitHub release)")?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(curl_fail(url, &err));
    }
    let meta = fs::metadata(dest).with_context(|| format!("stat {}", dest.display()))?;
    if meta.len() == 0 {
        bail!("download was empty: {url}");
    }
    Ok(())
}

fn tempfile_dir() -> Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("tws-tester-update-{}", std::process::id()));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

struct DeleteDir(PathBuf);

impl Drop for DeleteDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_is_0_1_1() {
        assert_eq!(version(), "0.1.1");
    }

    #[test]
    fn semver_strips_v_and_orders() {
        assert_eq!(parse_semver("v1.0.0"), Some((1, 0, 0)));
        assert_eq!(parse_semver("1.0.1"), Some((1, 0, 1)));
        assert!(parse_semver("v1.0.1").unwrap() > parse_semver("1.0.0").unwrap());
        assert_eq!(parse_semver("not-a-version"), None);
    }

    #[test]
    fn sha256_file_rejects_junk() {
        assert!(parse_sha256_file("").is_err());
        assert!(parse_sha256_file("deadbeef").is_err());
        let hex = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(parse_sha256_file(&format!("{hex}  foo.bin")).unwrap(), hex);
        assert_eq!(
            parse_sha256_file(&format!("{}\r\n", hex.to_uppercase())).unwrap(),
            hex
        );
    }

    #[test]
    fn empty_sha256_vector() {
        let mut h = Sha256::new();
        h.update([]);
        assert_eq!(
            format!("{:x}", h.finalize()),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn github_assets_match_release_workflow() {
        assert_eq!(
            asset_name_for("linux", "x86_64").unwrap(),
            "tws-tester-x86_64-unknown-linux-gnu"
        );
        assert_eq!(
            asset_name_for("windows", "x86_64").unwrap(),
            "tws-tester-x86_64-pc-windows-msvc.exe"
        );
        assert!(asset_name_for("linux", "aarch64").is_err());
        assert!(asset_name_for("macos", "x86_64").is_err());
    }

    #[test]
    fn sidecar_keeps_windows_exe_suffix() {
        let p = PathBuf::from("/bin/tws-tester.exe");
        assert_eq!(new_sidecar(&p).file_name().unwrap(), "tws-tester.new.exe");
        let p = PathBuf::from("/home/u/.local/bin/tws-tester");
        assert_eq!(new_sidecar(&p).file_name().unwrap(), "tws-tester.new");
    }

    #[test]
    fn sha256_file_hashes_bytes() {
        let dir = std::env::temp_dir().join(format!("tws-tester-sha-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("empty");
        fs::write(&p, b"").unwrap();
        assert_eq!(
            sha256_file(&p).unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let dest = dir.join("tws-tester");
        let src = dir.join("new");
        fs::write(&dest, b"old").unwrap();
        fs::write(&src, b"new").unwrap();
        replace_exe(&src, &dest).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"new");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn help_lists_flags() {
        assert!(HELP.contains("--history"));
        assert!(HELP.contains("--update"));
        assert!(HELP.contains("--version"));
        assert!(HELP.contains(version()));
    }
}

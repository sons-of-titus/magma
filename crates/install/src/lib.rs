//! Download-and-verify installation of language servers and toolchains.
//!
//! A plugin manifest pins a payload with `{:url :sha256 :format :bin}`;
//! this crate fetches it ([`fetch`]), checks the checksum before anything
//! is executed, unpacks, and lands an executable at a deterministic path.
//! Everything after the bytes are on disk lives in [`install_from_file`]
//! so it is testable without network.

mod fetch;

pub use fetch::{
    decompress_gz, fetch_to_file, finalize_exec, sha256_hex, untar_gz, verify_sha256,
};

use std::fs;
use std::path::{Path, PathBuf};

/// How the downloaded payload unpacks.  `Raw` is the file itself, `Gz` a
/// single gzip member (rust-analyzer release style), `TarGz` an archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Raw,
    Gz,
    TarGz,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim() {
            "raw" => Ok(Format::Raw),
            "gz" => Ok(Format::Gz),
            "tar.gz" | "tgz" => Ok(Format::TarGz),
            other => Err(format!(
                "unknown install format '{other}' (expected raw, gz, or tar.gz)"
            )),
        }
    }
}

/// One pinned payload from a plugin manifest.
#[derive(Debug, Clone)]
pub struct InstallSpec {
    pub url: String,
    pub sha256: String,
    pub format: Format,
    /// Executable name to land, e.g. "taplo" or "rust-analyzer".
    pub bin: String,
}

/// The versioned landing directory for one install:
/// `<downloads>/<kind>/<id>/<sha8-prefix>`.
pub fn version_dir(downloads_root: &Path, kind: &str, id: &str, sha256: &str) -> PathBuf {
    let sha8: String = sha256.trim().chars().take(8).collect();
    downloads_root.join(kind).join(id).join(sha8)
}

/// Unpack an already-downloaded payload into `dest_dir` and return the
/// executable's path.  A payload whose checksum does not match the pin is
/// never unpacked, let alone executed.
pub fn install_from_file(
    payload: &Path,
    spec: &InstallSpec,
    dest_dir: &Path,
) -> Result<PathBuf, String> {
    verify_sha256(payload, &spec.sha256)?;
    if dest_dir.exists() {
        fs::remove_dir_all(dest_dir).map_err(|e| format!("cannot clear {}: {e}", dest_dir.display()))?;
    }
    fs::create_dir_all(dest_dir).map_err(|e| format!("cannot create {}: {e}", dest_dir.display()))?;
    match spec.format {
        Format::Raw => {
            let dest = dest_dir.join(&spec.bin);
            fs::copy(payload, &dest).map_err(|e| format!("cannot install {}: {e}", dest.display()))?;
            finalize_exec(&dest)?;
            Ok(dest)
        }
        Format::Gz => {
            let dest = dest_dir.join(&spec.bin);
            decompress_gz(payload, &dest)?;
            finalize_exec(&dest)?;
            Ok(dest)
        }
        Format::TarGz => {
            untar_gz(payload, dest_dir)?;
            let exe = find_installed(dest_dir, &spec.bin)?;
            finalize_exec(&exe)?;
            Ok(exe)
        }
    }
}

/// Fetch, verify, and land `spec` under `downloads_root`.  A previous
/// install of the same pin short-circuits without network.
pub fn install(
    spec: &InstallSpec,
    downloads_root: &Path,
    kind: &str,
    id: &str,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<PathBuf, String> {
    let dest_dir = version_dir(downloads_root, kind, id, &spec.sha256);
    let cached = dest_dir.join(&spec.bin);
    if cached.is_file() {
        return Ok(cached);
    }
    let payload_dir = dest_dir.with_extension("magma-fetch");
    fs::create_dir_all(&payload_dir)
        .map_err(|e| format!("cannot create {}: {e}", payload_dir.display()))?;
    let payload = payload_dir.join("payload");
    fetch_to_file(&spec.url, &payload, progress)?;
    let exe = install_from_file(&payload, spec, &dest_dir);
    fs::remove_dir_all(&payload_dir).ok();
    exe
}

/// Where did `bin` end up inside an unpacked tree?  Archives nest a
/// top-level directory ("taplo-0.10.0/bin/taplo"), so look in the root,
/// `bin/`, and one nested level deep.  The same scan finds a previously
/// installed binary under its version directory.
pub fn find_installed(dir: &Path, bin: &str) -> Result<PathBuf, String> {
    let direct = dir.join(bin);
    if direct.is_file() {
        return Ok(direct);
    }
    let bin_dir = dir.join("bin").join(bin);
    if bin_dir.is_file() {
        return Ok(bin_dir);
    }
    let entries = fs::read_dir(dir).map_err(|e| format!("cannot list {}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let nested = entry.path().join(bin);
        if nested.is_file() {
            return Ok(nested);
        }
        let nested_bin = entry.path().join("bin").join(bin);
        if nested_bin.is_file() {
            return Ok(nested_bin);
        }
    }
    Err(format!("executable '{bin}' not found in archive"))
}

/// Find `bin` under a download slot (`…/servers/<lang>/`): scan the
/// version directories, newest first.  This is the read side of [`install`]
/// — the same layout, one lookup.
pub fn find_in_slot(slot: &Path, bin: &str) -> Option<PathBuf> {
    let Ok(entries) = fs::read_dir(slot) else {
        return None;
    };
    let mut versions: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .map(|e| {
            let modified = e
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            (modified, e.path())
        })
        .collect();
    versions.sort_by(|a, b| b.0.cmp(&a.0));
    versions
        .iter()
        .find_map(|(_, dir)| find_installed(dir, bin).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("magma_install_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn install_from_file_verifies_before_landing_an_executable() {
        let dir = tmp_dir("raw");
        let payload = dir.join("payload");
        fs::write(&payload, b"#!/bin/sh\necho server\n").unwrap();
        let spec = InstallSpec {
            url: String::new(),
            sha256: sha256_hex(&payload).unwrap(),
            format: Format::Raw,
            bin: "fake-ra".into(),
        };
        let dest = dir.join("servers").join("zz").join("deadbeef");
        let exe = install_from_file(&payload, &spec, &dest).unwrap();
        assert_eq!(exe, dest.join("fake-ra"));
        assert!(exe.is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_ne!(
                fs::metadata(&exe).unwrap().permissions().mode() & 0o111,
                0,
                "installed binary must be executable"
            );
        }
        // A wrong pin must not land anything.
        let bad = InstallSpec {
            sha256: "00".repeat(32),
            ..spec
        };
        let dest2 = dir.join("servers").join("zz").join("badbadbad");
        assert!(install_from_file(&payload, &bad, &dest2).is_err());
        assert!(!dest2.join("fake-ra").exists());
    }

    #[test]
    fn install_short_circuits_a_cached_pin_without_fetching() {
        let dir = tmp_dir("cache");
        let root = dir.join("downloads");
        let spec = InstallSpec {
            url: "https://invalid.invalid/no-such-payload".into(),
            sha256: "ab".repeat(32),
            format: Format::Raw,
            bin: "tool".into(),
        };
        let cached = version_dir(&root, "servers", "zz", &spec.sha256).join("tool");
        fs::create_dir_all(cached.parent().unwrap()).unwrap();
        fs::write(&cached, b"cached").unwrap();
        let mut no_progress = |_d: u64, _t: u64| {};
        let exe = install(&spec, &root, "servers", "zz", &mut no_progress).unwrap();
        assert_eq!(exe, cached);
    }

    #[test]
    fn find_in_slot_scans_versions_newest_first() {
        use std::time::{Duration, SystemTime};
        let dir = tmp_dir("slot");
        let slot = dir.join("servers").join("zz");
        let old = slot.join("aaaa");
        let new = slot.join("zzzz");
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&new).unwrap();
        fs::write(old.join("tool"), b"old").unwrap();
        fs::write(new.join("tool"), b"new").unwrap();
        let set_mtime = |p: &Path, t: SystemTime| filetime::set_file_mtime(p, t.into()).unwrap();
        // find_in_slot sorts by the version DIRECTORY's mtime; set both
        // explicitly — coarse filesystem stamp granularity must not tie
        // them and hand the decision to read_dir order.
        set_mtime(&old, SystemTime::UNIX_EPOCH + Duration::from_secs(1));
        set_mtime(&new, SystemTime::now());
        let found = find_in_slot(&slot, "tool").unwrap();
        assert_eq!(found, new.join("tool"), "newest version must win");
        assert!(find_in_slot(&slot, "absent").is_none());
        assert!(find_in_slot(&dir.join("missing"), "tool").is_none());
    }

    #[test]
    fn format_parse_is_strict() {
        assert_eq!(Format::parse("raw").unwrap(), Format::Raw);
        assert_eq!(Format::parse("tar.gz").unwrap(), Format::TarGz);
        assert_eq!(Format::parse("tgz").unwrap(), Format::TarGz);
        assert!(Format::parse("rar").is_err());
    }
}

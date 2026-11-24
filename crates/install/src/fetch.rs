//! Byte-level acquisition: streaming HTTP download, checksums, unpacking.
//!
//! Everything here works on paths — no install-layout knowledge — so it is
//! testable without network except the fetch itself.

use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use sha2::{Digest, Sha256};

/// Stream `url` to `dest`, reporting `(bytes_done, bytes_total)` — total is
/// 0 when the server sends no Content-Length.  Writes go through a temp
/// sibling so a killed download never leaves a half file at `dest`.
/// Only the connect phase is time-bounded: an overall timeout would cap
/// the transfer, and a stalled read surfaces as the user's cancel.
pub fn fetch_to_file(
    url: &str,
    dest: &Path,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .build();
    let resp = agent
        .get(url)
        .call()
        .map_err(|e| format!("download failed: {e}"))?;
    let total = resp
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let mut reader = resp.into_reader().take(1024 * 1024 * 1024);

    let tmp = dest.with_extension("magma-part");
    let mut out =
        fs::File::create(&tmp).map_err(|e| format!("cannot create {}: {e}", tmp.display()))?;
    let mut done: u64 = 0;
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let n = reader
            .read(&mut chunk)
            .map_err(|e| format!("download read: {e}"))?;
        if n == 0 {
            break;
        }
        out.write_all(&chunk[..n])
            .map_err(|e| format!("download write: {e}"))?;
        done += n as u64;
        progress(done, total);
    }
    out.sync_all().ok();
    fs::rename(&tmp, dest).map_err(|e| format!("cannot land {}: {e}", dest.display()))?;
    Ok(())
}

/// SHA-256 of `path` as lowercase hex.
pub fn sha256_hex(path: &Path) -> Result<String, String> {
    let mut f = fs::File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut f, &mut hasher).map_err(|e| format!("hash read: {e}"))?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// The manifest's hex must match the payload exactly — case and whitespace
/// insensitive, because release notes hand-write these.
pub fn verify_sha256(path: &Path, expected: &str) -> Result<(), String> {
    let got = sha256_hex(path)?;
    let want = expected.trim().to_ascii_lowercase();
    if got == want {
        return Ok(());
    }
    Err(format!(
        "checksum mismatch for {}: expected sha256 {want}, got {got}",
        path.display()
    ))
}

/// Gunzip `src` into the single file `dest` (parent dir created).
pub fn decompress_gz(src: &Path, dest: &Path) -> Result<(), String> {
    let input = fs::File::open(src).map_err(|e| format!("cannot open {}: {e}", src.display()))?;
    let mut decoder = flate2::read::GzDecoder::new(input);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    let mut out =
        fs::File::create(dest).map_err(|e| format!("cannot create {}: {e}", dest.display()))?;
    std::io::copy(&mut decoder, &mut out).map_err(|e| format!("gunzip: {e}"))?;
    Ok(())
}

/// Unpack a tar.gz archive into `dest_dir` (created).
pub fn untar_gz(src: &Path, dest_dir: &Path) -> Result<(), String> {
    let input = fs::File::open(src).map_err(|e| format!("cannot open {}: {e}", src.display()))?;
    let gz = flate2::read::GzDecoder::new(input);
    let mut archive = tar::Archive::new(gz);
    fs::create_dir_all(dest_dir).map_err(|e| format!("cannot create {}: {e}", dest_dir.display()))?;
    archive
        .unpack(dest_dir)
        .map_err(|e| format!("unpack: {e}"))
}

/// Make a file executable (no-op without permission bits on non-Unix).
pub fn finalize_exec(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)
            .map_err(|e| format!("cannot stat {}: {e}", path.display()))?
            .permissions();
        perms.set_mode(perms.mode() | 0o755);
        fs::set_permissions(path, perms)
            .map_err(|e| format!("cannot chmod {}: {e}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;

    fn tmp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("magma_fetch_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn checksum_accepts_the_matching_payload_and_rejects_others() {
        let dir = tmp_dir("sha");
        let file = dir.join("payload");
        fs::write(&file, b"magma install payload").unwrap();
        let hex = sha256_hex(&file).unwrap();
        verify_sha256(&file, &hex.to_ascii_uppercase()).unwrap();
        let err = verify_sha256(
            &file,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap_err();
        assert!(err.contains("checksum mismatch"), "unexpected: {err}");
    }

    #[test]
    fn gz_roundtrip_lands_the_single_member() {
        let dir = tmp_dir("gz");
        let raw = dir.join("tool");
        fs::write(&raw, b"#!/bin/sh\necho hi\n").unwrap();
        let gz = dir.join("tool.gz");
        let mut enc = flate2::write::GzEncoder::new(
            fs::File::create(&gz).unwrap(),
            flate2::Compression::default(),
        );
        enc.write_all(&fs::read(&raw).unwrap()).unwrap();
        enc.finish().unwrap();

        let out = dir.join("out").join("taplo");
        decompress_gz(&gz, &out).unwrap();
        assert_eq!(fs::read(&out).unwrap(), b"#!/bin/sh\necho hi\n");
    }

    #[test]
    fn tar_gz_unpacks_nested_paths() {
        let dir = tmp_dir("targz");
        let payload = dir.join("payload.tar.gz");
        {
            let enc = flate2::write::GzEncoder::new(
                fs::File::create(&payload).unwrap(),
                flate2::Compression::default(),
            );
            let mut builder = tar::Builder::new(enc);
            let nested = dir.join("taplo-0.10.0");
            fs::create_dir_all(nested.join("bin")).unwrap();
            fs::write(nested.join("bin").join("taplo"), b"bin").unwrap();
            builder
                .append_path_with_name(
                    nested.join("bin").join("taplo"),
                    "taplo-0.10.0/bin/taplo",
                )
                .unwrap();
            builder.into_inner().unwrap().finish().unwrap();
        }
        let out = dir.join("unpacked");
        untar_gz(&payload, &out).unwrap();
        let exe = out.join("taplo-0.10.0").join("bin").join("taplo");
        assert_eq!(fs::read(&exe).unwrap(), b"bin");
    }
}

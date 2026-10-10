//! VS Code-compatible icon-theme manifest loader (`iconDefinitions`,
//! `file`, `folder`, `folderExpanded`, `fileExtensions`, `fileNames`).
//! Icons referenced by `iconPath` become SVG entries; font-based themes
//! are honoured through `fontCharacter`/`fontColor`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::{IconKind, IconPack};

/// Load a pack from a manifest file or a directory containing one.
pub fn load_pack(path: &Path) -> Result<IconPack, String> {
    let manifest = if path.is_dir() {
        find_manifest(path)?
    } else {
        path.to_path_buf()
    };
    parse_manifest(&manifest)
}

/// A directory may hold the manifest under a conventional name; otherwise
/// the first JSON file carrying an `iconDefinitions` key wins.
fn find_manifest(dir: &Path) -> Result<PathBuf, String> {
    let conventional = dir.join("icon-theme.json");
    if conventional.is_file() {
        return Ok(conventional);
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("cannot read {}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    for file in files {
        if let Ok(bytes) = std::fs::read(&file) {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if v.get("iconDefinitions").is_some() {
                    return Ok(file);
                }
            }
        }
    }
    Err(format!("no icon-theme manifest in {}", dir.display()))
}

fn parse_manifest(manifest_path: &Path) -> Result<IconPack, String> {
    let bytes = std::fs::read(manifest_path)
        .map_err(|e| format!("cannot read {}: {e}", manifest_path.display()))?;
    let root: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("invalid JSON in {}: {e}", manifest_path.display()))?;
    let defs = root
        .get("iconDefinitions")
        .and_then(|v| v.as_object())
        .ok_or_else(|| format!("{} has no iconDefinitions", manifest_path.display()))?;
    let dir = manifest_path.parent().unwrap_or(Path::new("."));
    let name = root
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            manifest_path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "pack".to_string())
        });

    let resolve = |id: &str| -> Option<IconKind> {
        let def = defs.get(id)?;
        if let Some(p) = def.get("iconPath").and_then(|v| v.as_str()) {
            return Some(IconKind::Svg {
                path: dir.join(p.split('#').next().unwrap_or(p)),
            });
        }
        def.get("fontCharacter")
            .and_then(|v| v.as_str())
            .and_then(parse_codepoint)
            .map(|cp| {
                let color = def
                    .get("fontColor")
                    .and_then(|v| v.as_str())
                    .and_then(parse_hex_color);
                IconKind::Glyph {
                    glyph: cp,
                    color,
                }
            })
    };

    let mapping = |obj: Option<&serde_json::Map<String, serde_json::Value>>,
                       strip_dots: bool,
                       out: &mut HashMap<String, IconKind>| {
        if let Some(obj) = obj {
            for (key, val) in obj {
                let Some(id) = val.as_str() else { continue };
                // Extension keys match case-insensitively (done at lookup
                // time); exact file names must keep their case.
                let key = if strip_dots {
                    key.trim_start_matches('.').to_lowercase()
                } else {
                    key.clone()
                };
                if let Some(icon) = resolve(id) {
                    out.insert(key, icon);
                }
            }
        }
    };

    let mut pack = IconPack::empty(&name);
    mapping(
        root.get("fileNames").and_then(|v| v.as_object()),
        false,
        &mut pack.file_names,
    );
    mapping(
        root.get("fileExtensions").and_then(|v| v.as_object()),
        true,
        &mut pack.extensions,
    );
    pack.default_file = root.get("file").and_then(|v| v.as_str()).and_then(resolve);
    pack.folder = root
        .get("folder")
        .and_then(|v| v.as_str())
        .and_then(resolve);
    pack.folder_open = root
        .get("folderExpanded")
        .and_then(|v| v.as_str())
        .and_then(resolve);
    Ok(pack)
}

/// `fontCharacter` arrives as a decoded string ("\u{e001}"), a bare
/// codepoint literal ("0xE001"), or HTML ("&#xE001;").
fn parse_codepoint(s: &str) -> Option<char> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x") {
        return u32::from_str_radix(hex, 16).ok().and_then(char::from_u32);
    }
    if let Some(hex) = s
        .strip_prefix("&#x")
        .or_else(|| s.strip_prefix("&#X"))
        .map(|h| h.trim_end_matches(';'))
    {
        return u32::from_str_radix(hex, 16).ok().and_then(char::from_u32);
    }
    s.chars().next()
}

fn parse_hex_color(s: &str) -> Option<(u8, u8, u8)> {
    let hex = s.strip_prefix('#')?;
    match hex.len() {
        6 => Some((
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
        )),
        8 => Some((
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
            u8::from_str_radix(&hex[6..8], 16).ok()?,
        )),
        _ => None,
    }
}

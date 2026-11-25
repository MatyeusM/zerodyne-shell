//! Renders the wallpaper SVG with the current brand colour into a runtime
//! file. QML loads that file with QtQuick.VectorImage (true vector rendering)
//! instead of rasterising a recoloured data URL through QSvgRenderer.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

use crate::interaction::{Handler as HandlerTrait, ResultData};
use crate::runner::Ctx;

const SOURCE: &str = "assets/arch-icon.svg";
const PREFIX: &str = "arch-icon-";
const ORIGINAL_FILL: &str = "#1793d1";

/// Normalise `#rrggbb` / `rrggbb` to lowercase `#rrggbb`.
fn parse_hex(input: &str) -> Result<String> {
    let raw = input.strip_prefix('#').unwrap_or(input);
    if raw.len() != 6 || !raw.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(anyhow!(
            "wallpaper-color: expected #rrggbb, got {:?}",
            input
        ));
    }
    Ok(format!("#{}", raw.to_ascii_lowercase()))
}

/// Replace the single primary fill and lift the white highlight opacities.
/// The rest of the SVG (paths, gradients, geometry) is left byte-identical.
fn recolour(svg: &str, hex: &str) -> String {
    svg.replace(ORIGINAL_FILL, &format!("{};fill-opacity:0.8", hex))
        .replace("stop-opacity:.27450982", "stop-opacity:0.45")
        .replace("fill-opacity:.16568047", "fill-opacity:0.25")
}

/// Source lives next to the installed binary: <shell-dir>/assets/...
fn shell_dir() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let bin = exe
        .parent()
        .ok_or_else(|| anyhow!("wallpaper-color: no binary parent"))?;
    bin.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("wallpaper-color: no shell directory"))
}

/// One file per colour: a brand change produces a new URL, which makes the
/// QML VectorImage reload without any manual invalidation dance.
fn output_path(hex: &str) -> Result<PathBuf> {
    let runtime = std::env::var("XDG_RUNTIME_DIR")
        .map_err(|_| anyhow!("wallpaper-color: XDG_RUNTIME_DIR is unset"))?;
    let dir = Path::new(&runtime).join("zerodyne");
    fs::create_dir_all(&dir)?;
    Ok(dir.join(format!("{}{}.svg", PREFIX, hex.trim_start_matches('#'))))
}

/// Drop SVGs generated for previous brand colours.
fn remove_stale(dir: &Path, keep: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_generated = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with(PREFIX) && n.ends_with(".svg"));
        if is_generated && path != keep {
            let _ = fs::remove_file(path);
        }
    }
}

/// Write to a temporary file first so a reader never sees a truncated SVG.
fn write_atomic(path: &Path, data: &str) -> Result<()> {
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(&tmp, data)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

pub struct Handler;

impl Default for Handler {
    fn default() -> Self {
        Self::new()
    }
}

impl Handler {
    pub fn new() -> Self {
        Self
    }
}

impl HandlerTrait for Handler {
    fn name(&self) -> &'static str {
        "wallpaper-color"
    }

    fn usage(&self) -> &'static str {
        "wallpaper-color <#rrggbb>"
    }

    fn exec(&self, _ctx: &Ctx, args: &[String]) -> Result<ResultData> {
        if args.len() != 1 {
            return Err(anyhow!("usage: {}", self.usage()));
        }
        let hex = parse_hex(&args[0])?;
        let source = shell_dir()?.join(SOURCE);
        let svg = fs::read_to_string(&source)
            .map_err(|e| anyhow!("wallpaper-color: read {}: {}", source.display(), e))?;
        let output = output_path(&hex)?;
        write_atomic(&output, &recolour(&svg, &hex))?;
        if let Some(dir) = output.parent() {
            remove_stale(dir, &output);
        }
        Ok(ResultData::message(output.display().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_normalisation() {
        assert_eq!(parse_hex("#2BFDf4").unwrap(), "#2bfdf4");
        assert_eq!(parse_hex("2bfdf4").unwrap(), "#2bfdf4");
        assert!(parse_hex("red").is_err());
        assert!(parse_hex("#12345").is_err());
    }

    #[test]
    fn replaces_only_expected_tokens() {
        let svg = "fill:#1793d1;stop-opacity:.27450982;fill-opacity:.16568047";
        assert_eq!(
            recolour(svg, "#2bfdf4"),
            "fill:#2bfdf4;fill-opacity:0.8;stop-opacity:0.45;fill-opacity:0.25"
        );
    }
}

//! Raster `assets/logo/*.svg` into 1-bit bitmaps the TUI paints as half-blocks.
//!
//! A broken or empty SVG is skipped (cargo warning) so one bad file cannot
//! fail the crate. Add a logo by dropping `{slug}-icon.svg` or `{slug}-logo.svg`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{self, Tree};

const MAX_SIDE: f32 = 96.0;
const ALPHA_ON: u8 = 40;

fn main() {
    println!("cargo:rerun-if-changed=assets/logo");
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let logo_dir = Path::new("assets/logo");
    let mut logos: Vec<(String, u16, u16)> = Vec::new();

    let entries = match fs::read_dir(logo_dir) {
        Ok(e) => e,
        Err(err) => {
            println!("cargo:warning=assets/logo unreadable: {err}");
            write_map(&out_dir, &[]).expect("write empty logo map");
            return;
        }
    };

    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("svg"))
        .collect();
    // `-icon` after `-logo` so an icon replaces a logo for the same slug.
    files.sort_by(|a, b| {
        let rank = |p: &Path| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| if s.ends_with("-icon") { 1 } else { 0 })
                .unwrap_or(0)
        };
        rank(a).cmp(&rank(b)).then_with(|| a.cmp(b))
    });

    for path in files {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(slug) = slug_from_stem(stem) else {
            println!(
                "cargo:warning=skipping {}: filename must be {{slug}}.svg, {{slug}}-icon.svg, or {{slug}}-logo.svg with a lowercase slug",
                path.display()
            );
            continue;
        };
        match rasterize(&path) {
            Ok((w, h, bits)) => {
                if let Err(err) = fs::write(out_dir.join(format!("{slug}.bin")), &bits) {
                    println!("cargo:warning=skipping {slug}: write bitmap: {err}");
                    continue;
                }
                if let Some(pos) = logos.iter().position(|(k, _, _)| k == slug) {
                    logos[pos] = (slug.to_string(), w, h);
                } else {
                    logos.push((slug.to_string(), w, h));
                }
            }
            Err(err) => {
                println!("cargo:warning=skipping {}: {err}", path.display());
            }
        }
    }

    logos.sort_by(|a, b| a.0.cmp(&b.0));
    write_map(&out_dir, &logos).expect("write logo map");
}

fn slug_from_stem(stem: &str) -> Option<&str> {
    let slug = stem
        .strip_suffix("-icon")
        .or_else(|| stem.strip_suffix("-logo"))
        .unwrap_or(stem);
    if slug.is_empty()
        || !slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    {
        return None;
    }
    Some(slug)
}

fn rasterize(path: &Path) -> Result<(u16, u16, Vec<u8>), String> {
    let svg = fs::read(path).map_err(|e| e.to_string())?;
    let mut opt = usvg::Options {
        resources_dir: path.parent().map(Path::to_path_buf),
        ..usvg::Options::default()
    };
    opt.fontdb_mut().load_system_fonts();
    let tree = Tree::from_data(&svg, &opt).map_err(|e| e.to_string())?;
    let size = tree.size();
    let sw = size.width();
    let sh = size.height();
    if !sw.is_finite() || !sh.is_finite() || sw < 1.0 || sh < 1.0 {
        return Err("svg has no drawable size".into());
    }
    let scale = MAX_SIDE / sw.max(sh);
    let pw = (sw * scale).round().clamp(1.0, MAX_SIDE) as u32;
    let ph = (sh * scale).round().clamp(1.0, MAX_SIDE) as u32;
    let mut pixmap = Pixmap::new(pw, ph).ok_or("could not allocate pixmap")?;
    let tx = Transform::from_scale(pw as f32 / sw, ph as f32 / sh);
    resvg::render(&tree, tx, &mut pixmap.as_mut());
    pack_alpha(pixmap.data(), pw, ph)
}

fn pack_alpha(rgba: &[u8], w: u32, h: u32) -> Result<(u16, u16, Vec<u8>), String> {
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut any = false;
    for y in 0..h {
        for x in 0..w {
            if alpha_at(rgba, w, x, y) > ALPHA_ON {
                any = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    if !any {
        return Err("no ink after raster".into());
    }
    let min_x = min_x.saturating_sub(1);
    let min_y = min_y.saturating_sub(1);
    let max_x = (max_x + 1).min(w - 1);
    let max_y = (max_y + 1).min(h - 1);
    let cw = max_x - min_x + 1;
    let ch = max_y - min_y + 1;
    let stride = (cw as usize).div_ceil(8);
    let mut bits = vec![0u8; stride * ch as usize];
    for y in 0..ch {
        for x in 0..cw {
            if alpha_at(rgba, w, min_x + x, min_y + y) > ALPHA_ON {
                let i = y as usize * stride + x as usize / 8;
                bits[i] |= 1 << (7 - (x as usize % 8));
            }
        }
    }
    Ok((cw as u16, ch as u16, bits))
}

fn alpha_at(rgba: &[u8], w: u32, x: u32, y: u32) -> u8 {
    rgba[(y * w + x) as usize * 4 + 3]
}

fn write_map(out_dir: &Path, logos: &[(String, u16, u16)]) -> std::io::Result<()> {
    let mut src = String::from(
        "// @generated by build.rs from assets/logo. Do not edit.\n\
         fn bitmap(key: &str) -> Option<Bitmap> {\n\
             match key {\n",
    );
    for (slug, w, h) in logos {
        src.push_str(&format!(
            "        \"{slug}\" => Some(Bitmap {{ w: {w}, h: {h}, bits: include_bytes!(\"{slug}.bin\") }}),\n"
        ));
    }
    src.push_str("        _ => None,\n    }\n}\n");
    fs::write(out_dir.join("logos.rs"), src)
}

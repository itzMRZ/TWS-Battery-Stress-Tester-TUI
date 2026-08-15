//! Official brand marks, painted as half-block images in the TUI.
//!
//! Drop `{slug}-icon.svg` or `{slug}-logo.svg` in `assets/logo/`. `build.rs`
//! rasterizes every file it finds. A missing or broken asset is a blank slot,
//! never a panic. Recolor at render with `Brand::ink()`. Assets stay
//! `currentColor`.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

type Painted = HashMap<(String, u16, u16), Vec<String>>;

/// Terminal cell box a mark is painted into. Preserve the source aspect
/// inside the box; letterbox with spaces. Sizes match `assets/logo/README.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MarkBox {
    pub cols: u16,
    pub rows: u16,
}

impl MarkBox {
    pub const COMPACT: Self = Self { cols: 16, rows: 2 };
    pub const STANDARD: Self = Self { cols: 24, rows: 3 };
    pub const LOCKUP: Self = Self { cols: 24, rows: 4 };

    /// Largest README box that fits in `cols` × `rows` of free cells.
    /// Smaller terminals get a scaled compact mark, not a clipped one.
    pub fn fit(cols: u16, rows: u16) -> Option<Self> {
        if cols < 4 || rows < 2 {
            return None;
        }
        if rows >= Self::LOCKUP.rows && cols >= 16 {
            Some(Self {
                cols: cols.min(Self::LOCKUP.cols),
                rows: Self::LOCKUP.rows,
            })
        } else if rows >= Self::STANDARD.rows && cols >= 16 {
            Some(Self {
                cols: cols.min(Self::STANDARD.cols),
                rows: Self::STANDARD.rows,
            })
        } else {
            Some(Self {
                cols: cols.min(Self::COMPACT.cols),
                rows: Self::COMPACT.rows,
            })
        }
    }
}

struct Bitmap {
    w: u16,
    h: u16,
    bits: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/logos.rs"));

/// Half-block rows sized to `mark`. Empty if this slug has no asset.
pub fn render(slug: &str, mark: MarkBox) -> Vec<String> {
    if mark.cols == 0 || mark.rows == 0 {
        return Vec::new();
    }
    let key = (asset_key(slug).to_string(), mark.cols, mark.rows);
    static CACHE: OnceLock<Mutex<Painted>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(g) = cache.lock() {
        if let Some(rows) = g.get(&key) {
            return rows.clone();
        }
    }
    let Some(bmp) = bitmap(asset_key(slug)) else {
        if let Ok(mut g) = cache.lock() {
            g.insert(key, Vec::new());
        }
        return Vec::new();
    };
    let rows = paint(&bmp, mark);
    let rows = if has_ink(&rows) { rows } else { Vec::new() };
    if let Ok(mut g) = cache.lock() {
        g.insert(key, rows.clone());
    }
    rows
}

pub fn is_available(slug: &str) -> bool {
    bitmap(asset_key(slug)).is_some()
}

/// Slugs that reuse another brand's file. New brands drop a file named after the TUI slug instead.
fn asset_key(slug: &str) -> &str {
    match slug {
        "anker" => "soundcore",
        "redmi" | "poco" => "xiaomi",
        other => other,
    }
}

fn paint(bmp: &Bitmap, mark: MarkBox) -> Vec<String> {
    let cols = mark.cols as usize;
    let rows = mark.rows as usize;
    let dest_w = cols as f32;
    let dest_h = (rows * 2) as f32;
    let src_w = bmp.w as f32;
    let src_h = bmp.h as f32;
    let scale = (dest_w / src_w).min(dest_h / src_h);
    if !scale.is_finite() || scale <= 0.0 {
        return blank(mark);
    }
    let fit_w = src_w * scale;
    let fit_h = src_h * scale;
    let off_x = (dest_w - fit_w) / 2.0;
    let off_y = (dest_h - fit_h) / 2.0;

    let mut pixels = vec![false; cols * rows * 2];
    for dy in 0..(rows * 2) {
        for dx in 0..cols {
            let x0 = (dx as f32 - off_x) / scale;
            let x1 = ((dx + 1) as f32 - off_x) / scale;
            let y0 = (dy as f32 - off_y) / scale;
            let y1 = ((dy + 1) as f32 - off_y) / scale;
            pixels[dy * cols + dx] = coverage(bmp, x0, y0, x1, y1);
        }
    }

    let mut out = Vec::with_capacity(rows);
    for r in 0..rows {
        let mut s = String::with_capacity(cols);
        for x in 0..cols {
            let top = pixels[r * 2 * cols + x];
            let bot = pixels[(r * 2 + 1) * cols + x];
            s.push(match (top, bot) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            });
        }
        out.push(s);
    }
    out
}

fn coverage(bmp: &Bitmap, x0: f32, y0: f32, x1: f32, y1: f32) -> bool {
    let x0 = x0.max(0.0);
    let y0 = y0.max(0.0);
    let x1 = x1.min(bmp.w as f32);
    let y1 = y1.min(bmp.h as f32);
    if x1 <= x0 || y1 <= y0 {
        return false;
    }
    let ix0 = x0.floor() as u32;
    let iy0 = y0.floor() as u32;
    let ix1 = (x1.ceil() as u32).min(bmp.w as u32);
    let iy1 = (y1.ceil() as u32).min(bmp.h as u32);
    let mut on = 0u32;
    let mut n = 0u32;
    for y in iy0..iy1 {
        for x in ix0..ix1 {
            n += 1;
            if ink_at(bmp, x, y) {
                on += 1;
            }
        }
    }
    n > 0 && on * 100 >= n * 18
}

fn ink_at(bmp: &Bitmap, x: u32, y: u32) -> bool {
    if x >= bmp.w as u32 || y >= bmp.h as u32 {
        return false;
    }
    let stride = (bmp.w as usize).div_ceil(8);
    let i = y as usize * stride + x as usize / 8;
    let shift = 7 - (x as usize % 8);
    bmp.bits.get(i).is_some_and(|b| b & (1 << shift) != 0)
}

fn blank(mark: MarkBox) -> Vec<String> {
    vec![" ".repeat(mark.cols as usize); mark.rows as usize]
}

fn has_ink(rows: &[String]) -> bool {
    rows.iter()
        .any(|r| r.contains('█') || r.contains('▀') || r.contains('▄'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn logo_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/logo")
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

    #[test]
    fn missing_logo_is_empty_not_a_panic() {
        assert!(render("nope", MarkBox::COMPACT).is_empty());
        assert!(render("unknown", MarkBox::LOCKUP).is_empty());
        assert!(!is_available("nope"));
    }

    #[test]
    fn soundcore_logo_is_an_image_not_braille_cells() {
        let rows = render("soundcore", MarkBox::COMPACT);
        assert_eq!(rows.len(), MarkBox::COMPACT.rows as usize);
        assert!(rows
            .iter()
            .all(|r| r.chars().count() == MarkBox::COMPACT.cols as usize));
        assert!(has_ink(&rows));
        assert!(rows
            .iter()
            .all(|r| !r.chars().any(|c| ('\u{2800}'..='\u{28FF}').contains(&c))));
    }

    #[test]
    fn aliases_share_the_family_asset() {
        assert!(is_available("anker"));
        assert!(is_available("redmi"));
        assert!(is_available("poco"));
        assert_eq!(
            render("anker", MarkBox::COMPACT),
            render("soundcore", MarkBox::COMPACT)
        );
        assert_eq!(
            render("redmi", MarkBox::COMPACT),
            render("xiaomi", MarkBox::COMPACT)
        );
        assert_eq!(
            render("poco", MarkBox::STANDARD),
            render("xiaomi", MarkBox::STANDARD)
        );
    }

    #[test]
    fn lockup_fits_the_box() {
        let rows = render("apple", MarkBox::LOCKUP);
        assert_eq!(rows.len(), 4);
        assert!(rows.iter().all(|r| r.chars().count() == 24));
        assert!(has_ink(&rows));
    }

    #[test]
    fn fit_picks_a_box_the_area_can_hold() {
        assert_eq!(MarkBox::fit(80, 10), Some(MarkBox::LOCKUP));
        assert_eq!(MarkBox::fit(80, 3), Some(MarkBox::STANDARD));
        assert_eq!(MarkBox::fit(12, 10), Some(MarkBox { cols: 12, rows: 2 }));
        assert_eq!(MarkBox::fit(3, 10), None);
        assert_eq!(MarkBox::fit(24, 1), None);
    }

    #[test]
    fn every_svg_file_is_an_available_slug() {
        let dir = logo_dir();
        let mut found = 0;
        for ent in fs::read_dir(&dir).unwrap() {
            let path = ent.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("svg") {
                continue;
            }
            found += 1;
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap();
            let slug = slug_from_stem(stem).unwrap_or_else(|| panic!("bad logo filename {stem}"));
            assert!(
                is_available(slug),
                "{} ({slug}) was not rasterized; check cargo warnings from build.rs",
                path.display()
            );
            let rows = render(slug, MarkBox::COMPACT);
            assert!(has_ink(&rows), "{slug} rasterized to a blank compact mark");
        }
        assert!(found > 0, "assets/logo has no svg files");
    }
}

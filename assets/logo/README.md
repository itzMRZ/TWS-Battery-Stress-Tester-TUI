# Brand logos

One logo asset per brand. Drop a file here; `build.rs` discovers it on the next `cargo build`. The terminal paints it as half-blocks and recolors with `Brand::ink()`. A missing or broken SVG is a blank slot. It does not fail the build.

## Adding a logo

1. Put the SVG here as `{slug}-icon.svg` or `{slug}-logo.svg` (or `{slug}.svg`). `slug` is lowercase ASCII; `-icon` / `-logo` is stripped.
2. Monochrome, `currentColor`, a `viewBox`, no baked brand color or background. Aspect ratio is preserved inside the box below.
3. Rebuild. If the slug already aliases another brand (Anker to soundcore, Redmi/Poco to xiaomi), add one arm to `asset_key` in `src/brand/marks/mod.rs` instead of a second file.

A parse or raster error prints `cargo:warning=skipping ...` and that brand stays blank.

Some brands ship as a wordmark (their identity is the brand name in a particular type, not a separate pictorial glyph): samsung, sony, jabra, marshall, nothing, cmf, honor, bose, vivo, oppo, realme. A wordmark cannot read at a handful of half-block pixel rows; it paints as noise. `render` in `src/brand/marks/mod.rs` skips those assets and paints a plain colored initial instead (`monogram_letter`). The listed asset stays for provenance; swap it for a real pictorial glyph and drop that brand's `monogram_letter` arm whenever one turns up.

## Sizes

All SVGs use an intrinsic `viewBox` and omit fixed display dimensions:

- compact: 16 columns x 2 rows (list)
- standard: 24 columns x 3 rows
- lockup: 24 columns x 4 rows

Set the foreground at render time. `nothing-logo.svg` uses its embedded reference alpha as a recolorable mask.

Anker audio devices use `soundcore-icon.svg`. Redmi and Poco use `xiaomi-icon.svg`.

Simple Icons assets are CC0. CMF, realme, Jabra, Edifier, and Marshall logos were sourced as SVGs from Wikimedia Commons. `nothing-logo.svg` preserves the supplied Nothing Phone glyph. `soundcore-icon.svg` was sourced from SVG Repo.

| Brand / slug | Asset |
| --- | --- |
| soundcore, anker | `soundcore-icon.svg` |
| samsung | `samsung-icon.svg` |
| sony | `sony-icon.svg` |
| xiaomi, redmi, poco | `xiaomi-icon.svg` |
| realme | `realme-logo.svg` |
| oneplus | `oneplus-icon.svg` |
| oppo | `oppo-icon.svg` |
| vivo | `vivo-icon.svg` |
| nothing | `nothing-logo.svg` |
| cmf | `cmf-logo.svg` |
| google | `google-icon.svg` |
| huawei | `huawei-icon.svg` |
| honor | `honor-icon.svg` |
| jbl | `jbl-icon.svg` |
| bose | `bose-icon.svg` |
| jabra | `jabra-logo.svg` |
| sennheiser | `sennheiser-icon.svg` |
| beats | `beats-icon.svg` |
| apple | `apple-icon.svg` |
| edifier | `edifier-logo.svg` |
| marshall | `marshall-logo.svg` |

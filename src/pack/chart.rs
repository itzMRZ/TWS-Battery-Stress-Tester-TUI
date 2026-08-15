//! SVG time-vs-percent charts for the support pack.

use crate::cells;
use crate::death::EventKind;
use crate::device::Sample;

pub const LEFT: &str = "#89b4fa";
pub const RIGHT: &str = "#a6e3a1";
pub const PAIR: &str = "#cdd6f4";
pub const CASE: &str = "#f9e2af";
pub const EVENT: &str = "#f38ba8";
pub const GRID: &str = "#313244";
pub const AXIS: &str = "#6c7086";

const W: f64 = 760.0;
const H: f64 = 280.0;
const PAD_L: f64 = 48.0;
const PAD_R: f64 = 16.0;
const PAD_T: f64 = 36.0;
const PAD_B: f64 = 32.0;

#[derive(Clone, Debug)]
pub struct Series {
    pub name: String,
    pub color: &'static str,
    pub pts: Vec<(f64, f64)>,
}

impl Series {
    pub fn named(name: &str, color: &'static str, pts: Vec<(f64, f64)>) -> Self {
        Self {
            name: name.to_string(),
            color,
            pts,
        }
    }
}

pub fn series_for(samples: &[Sample], name: &'static str, color: &'static str) -> Option<Series> {
    let pts: Vec<(f64, f64)> = samples
        .iter()
        .filter_map(|s| {
            let p = cells::named_percent(&s.cells, name)?;
            Some((s.elapsed_ms as f64 / 1000.0, p as f64))
        })
        .collect();
    if pts.is_empty() {
        None
    } else {
        Some(Series::named(name, color, pts))
    }
}

pub fn overlay_series(samples: &[Sample]) -> Vec<Series> {
    let mut out = Vec::new();
    for (name, color) in [
        ("left", LEFT),
        ("right", RIGHT),
        ("pair", PAIR),
        ("case", CASE),
    ] {
        if let Some(s) = series_for(samples, name, color) {
            out.push(s);
        }
    }
    if out.is_empty() {
        let pts: Vec<(f64, f64)> = samples
            .iter()
            .filter_map(|s| {
                let p = cells::headline(&s.cells)?;
                Some((s.elapsed_ms as f64 / 1000.0, p as f64))
            })
            .collect();
        if !pts.is_empty() {
            out.push(Series::named("cells", PAIR, pts));
        }
    }
    out
}

/// Overlay plus a left-only and a right-only chart when those cells exist.
pub fn report_charts(samples: &[Sample], events: &[(u64, EventKind)]) -> String {
    let overlay = overlay_series(samples);
    let mut out = String::new();
    out.push_str(&figure("time vs percent", &overlay, events, true));
    if let Some(left) = series_for(samples, "left", LEFT) {
        out.push_str(&figure("left", &[left], &[], false));
    }
    if let Some(right) = series_for(samples, "right", RIGHT) {
        out.push_str(&figure("right", &[right], &[], false));
    }
    out
}

pub fn figure(
    caption: &str,
    series: &[Series],
    events: &[(u64, EventKind)],
    mark_events: bool,
) -> String {
    format!(
        "<figure><figcaption>{}</figcaption>{}</figure>\n",
        esc(caption),
        svg(caption, series, events, mark_events)
    )
}

pub fn svg(
    title: &str,
    series: &[Series],
    events: &[(u64, EventKind)],
    mark_events: bool,
) -> String {
    let plot_w = W - PAD_L - PAD_R;
    let plot_h = H - PAD_T - PAD_B;
    let max_t = series
        .iter()
        .flat_map(|s| s.pts.iter().map(|p| p.0))
        .chain(events.iter().map(|(ms, _)| *ms as f64 / 1000.0))
        .fold(1.0_f64, f64::max)
        .max(1.0);

    let x = |t: f64| PAD_L + (t / max_t) * plot_w;
    let y = |p: f64| PAD_T + (1.0 - (p / 100.0).clamp(0.0, 1.0)) * plot_h;

    let mut body = String::new();

    for pct in [0.0, 25.0, 50.0, 75.0, 100.0] {
        let yy = y(pct);
        body.push_str(&format!(
            r#"<line x1="{:.1}" y1="{yy:.1}" x2="{:.1}" y2="{yy:.1}" stroke="{GRID}" stroke-width="1"/>"#,
            PAD_L,
            PAD_L + plot_w,
        ));
        body.push_str(&format!(
            r#"<text x="{:.1}" y="{:.1}" fill="{AXIS}" font-size="11" text-anchor="end">{:.0}</text>"#,
            PAD_L - 8.0,
            yy + 4.0,
            pct,
        ));
    }
    for i in 0..=4 {
        let t = max_t * f64::from(i) / 4.0;
        let xx = x(t);
        body.push_str(&format!(
            r#"<line x1="{xx:.1}" y1="{:.1}" x2="{xx:.1}" y2="{:.1}" stroke="{GRID}" stroke-width="1"/>"#,
            PAD_T,
            PAD_T + plot_h,
        ));
        body.push_str(&format!(
            r#"<text x="{xx:.1}" y="{:.1}" fill="{AXIS}" font-size="11" text-anchor="middle">{}</text>"#,
            PAD_T + plot_h + 16.0,
            fmt_tick(t, max_t),
        ));
    }

    body.push_str(&format!(
        r#"<text x="12" y="{:.1}" fill="{AXIS}" font-size="11" transform="rotate(-90 12 {:.1})" text-anchor="middle">percent</text>"#,
        PAD_T + plot_h / 2.0,
        PAD_T + plot_h / 2.0,
    ));
    body.push_str(&format!(
        r#"<text x="{:.1}" y="{:.1}" fill="{AXIS}" font-size="11" text-anchor="middle">time</text>"#,
        PAD_L + plot_w / 2.0,
        H - 4.0,
    ));

    if mark_events {
        let mut last_x = f64::NEG_INFINITY;
        let mut stagger = 0i32;
        for (ms, kind) in events {
            let xx = x(*ms as f64 / 1000.0);
            body.push_str(&format!(
                r#"<line class="event" data-tag="{tag}" x1="{xx:.1}" y1="{:.1}" x2="{xx:.1}" y2="{:.1}" stroke="{EVENT}" stroke-width="1" stroke-dasharray="3 3"/>"#,
                PAD_T,
                PAD_T + plot_h,
                tag = esc(kind.tag()),
            ));
            if xx - last_x < 42.0 {
                stagger += 1;
            } else {
                stagger = 0;
            }
            last_x = xx;
            let ty = PAD_T - 6.0 - f64::from(stagger) * 11.0;
            body.push_str(&format!(
                r#"<text x="{xx:.1}" y="{ty:.1}" fill="{EVENT}" font-size="10" text-anchor="middle">{}</text>"#,
                esc(kind.tag()),
            ));
        }
    }

    for s in series {
        if s.pts.is_empty() {
            continue;
        }
        let mut d = String::new();
        for (i, (t, p)) in s.pts.iter().enumerate() {
            if i == 0 {
                d.push_str(&format!("M {:.1} {:.1}", x(*t), y(*p)));
            } else {
                d.push_str(&format!(" L {:.1} {:.1}", x(*t), y(*p)));
            }
        }
        body.push_str(&format!(
            r#"<path class="series" data-series="{}" d="{d}" fill="none" stroke="{}" stroke-width="2"/>"#,
            esc(&s.name),
            s.color,
        ));
        for (t, p) in &s.pts {
            body.push_str(&format!(
                r#"<circle cx="{:.1}" cy="{:.1}" r="2.2" fill="{}"/>"#,
                x(*t),
                y(*p),
                s.color,
            ));
        }
    }

    let mut lx = PAD_L;
    for s in series {
        if s.pts.is_empty() {
            continue;
        }
        body.push_str(&format!(
            r#"<rect x="{lx:.1}" y="8" width="14" height="3" fill="{}"/>"#,
            s.color,
        ));
        body.push_str(&format!(
            r#"<text class="legend" data-series="{}" x="{:.1}" y="12" fill="{}" font-size="11">{}</text>"#,
            esc(&s.name),
            lx + 18.0,
            s.color,
            esc(&s.name),
        ));
        lx += 18.0 + (s.name.len() as f64) * 6.6 + 18.0;
    }
    if mark_events && !events.is_empty() {
        body.push_str(&format!(
            r#"<line x1="{lx:.1}" y1="9.5" x2="{:.1}" y2="9.5" stroke="{EVENT}" stroke-width="1" stroke-dasharray="3 3"/>"#,
            lx + 14.0,
        ));
        body.push_str(&format!(
            r#"<text class="legend" data-series="events" x="{:.1}" y="12" fill="{EVENT}" font-size="11">events</text>"#,
            lx + 18.0,
        ));
    }

    if series.iter().all(|s| s.pts.is_empty()) {
        body.push_str(&format!(
            r#"<text x="{:.1}" y="{:.1}" fill="{AXIS}" font-size="13" text-anchor="middle">no samples yet</text>"#,
            PAD_L + plot_w / 2.0,
            PAD_T + plot_h / 2.0,
        ));
    }

    format!(
        r##"<svg viewBox="0 0 {W} {H}" role="img" aria-label="{title}"><rect width="{W}" height="{H}" fill="#161a20" rx="8"/>{body}</svg>"##,
        title = esc(title),
        body = body,
    )
}

fn fmt_tick(sec: f64, max_t: f64) -> String {
    let s = sec.round().max(0.0) as u64;
    if max_t >= 3600.0 {
        format!("{}:{:02}", s / 3600, (s % 3600) / 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cells;
    use crate::device::Sample;
    use chrono::Local;

    fn sample(elapsed_ms: u64, left: Option<u8>, right: Option<u8>) -> Sample {
        let mut cells_v = Vec::new();
        if let Some(p) = left {
            cells_v.push(cells::cell("left", Some(p), "test"));
        }
        if let Some(p) = right {
            cells_v.push(cells::cell("right", Some(p), "test"));
        }
        Sample {
            t: Local::now(),
            elapsed_ms,
            cells: cells_v,
            codec: None,
            host_volume: None,
            present: true,
        }
    }

    #[test]
    fn overlay_includes_left_and_right_and_omits_empty_pair() {
        let samples = [
            sample(0, Some(90), Some(88)),
            sample(3_600_000, Some(40), Some(42)),
        ];
        let series = overlay_series(&samples);
        let names: Vec<&str> = series.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["left", "right"]);
    }

    #[test]
    fn report_has_three_charts_legend_and_event_tags() {
        let samples = [
            sample(0, Some(90), Some(88)),
            sample(1_800_000, Some(65), Some(70)),
            sample(3_600_000, Some(40), Some(42)),
        ];
        let html = report_charts(&samples, &[(1_800_000, EventKind::Blip)]);
        assert!(html.contains("time vs percent"), "{html}");
        assert!(html.contains("<figcaption>left</figcaption>"), "{html}");
        assert!(html.contains("<figcaption>right</figcaption>"), "{html}");
        assert!(html.contains("data-series=\"left\""), "{html}");
        assert!(html.contains("data-series=\"right\""), "{html}");
        assert!(html.contains("data-tag=\"blip\""), "{html}");
        assert!(html.contains(">events<"), "{html}");
        assert!(!html.contains("data-series=\"pair\""), "{html}");
    }

    #[test]
    fn pair_only_device_gets_one_chart() {
        let s = Sample {
            t: Local::now(),
            elapsed_ms: 0,
            cells: vec![cells::cell("pair", Some(80), "os")],
            codec: None,
            host_volume: None,
            present: true,
        };
        let html = report_charts(&[s], &[]);
        assert!(html.contains("time vs percent"));
        assert!(!html.contains("<figcaption>left</figcaption>"));
        assert!(!html.contains("<figcaption>right</figcaption>"));
        assert!(html.contains("data-series=\"pair\""));
    }
}

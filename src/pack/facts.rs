//! Soak findings: drain, L/R imbalance, and event tags that are not just noise.

use crate::cells;
use crate::death::EventKind;
use crate::device::Sample;

const CELL_ORDER: &[&str] = &["left", "right", "pair", "case", "pack"];
/// Gap large enough to notice on a TWS pair; smaller drift stays in the cells table.
const IMBALANCE_PCT: u8 = 15;

#[derive(Clone, Debug, PartialEq)]
pub struct CellCurve {
    pub name: &'static str,
    pub start: Option<u8>,
    pub end: Option<u8>,
    pub min: Option<u8>,
    pub drain_per_hour: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Facts {
    pub elapsed_ms: u64,
    pub cells: Vec<CellCurve>,
    pub lr_gap: Option<u8>,
    pub findings: Vec<String>,
}

pub fn collect(samples: &[Sample], events: &[(u64, EventKind)]) -> Facts {
    let elapsed_ms = samples.last().map(|s| s.elapsed_ms).unwrap_or(0);
    let cells: Vec<CellCurve> = CELL_ORDER
        .iter()
        .filter_map(|name| curve(samples, name, elapsed_ms))
        .collect();
    let lr_gap = max_lr_gap(samples);
    let findings = findings(&cells, lr_gap, events);
    Facts {
        elapsed_ms,
        cells,
        lr_gap,
        findings,
    }
}

fn curve(samples: &[Sample], name: &'static str, elapsed_ms: u64) -> Option<CellCurve> {
    let vals: Vec<u8> = samples
        .iter()
        .filter_map(|s| cells::named_percent(&s.cells, name))
        .collect();
    if vals.is_empty() {
        return None;
    }
    let start = vals.first().copied();
    let end = vals.last().copied();
    let min = vals.iter().copied().min();
    let drain_per_hour = match (start, end) {
        (Some(a), Some(b)) if b <= a => drain(a, b, elapsed_ms),
        _ => None,
    };
    Some(CellCurve {
        name,
        start,
        end,
        min,
        drain_per_hour,
    })
}

fn drain(start: u8, end: u8, elapsed_ms: u64) -> Option<f64> {
    if elapsed_ms < 3 * 60 * 1000 {
        return None;
    }
    let hours = elapsed_ms as f64 / 3_600_000.0;
    Some((f64::from(start) - f64::from(end)) / hours)
}

fn max_lr_gap(samples: &[Sample]) -> Option<u8> {
    samples
        .iter()
        .filter_map(|s| {
            let l = cells::named_percent(&s.cells, "left")?;
            let r = cells::named_percent(&s.cells, "right")?;
            Some(l.abs_diff(r))
        })
        .max()
}

fn findings(cells: &[CellCurve], lr_gap: Option<u8>, events: &[(u64, EventKind)]) -> Vec<String> {
    let mut out = Vec::new();
    let count = |kind: EventKind| events.iter().filter(|(_, e)| *e == kind).count();

    if count(EventKind::ReportedEmptyStillPlaying) > 0 {
        out.push("firmware said empty while still playing".into());
    }
    if count(EventKind::FalseDeath) > 0 {
        out.push("came back after a false death".into());
    }
    if count(EventKind::PercentStuck) > 0 {
        out.push("percent stuck while still playing".into());
    }
    match count(EventKind::Blip) {
        0 => {}
        1 => out.push("one brief disconnect".into()),
        n => out.push(format!("{n} brief disconnects")),
    }
    if let Some(gap) = lr_gap {
        if gap >= IMBALANCE_PCT {
            out.push(format!("left and right differed by up to {gap}%"));
        }
    }
    for c in cells {
        if let (Some(a), Some(b)) = (c.start, c.end) {
            if b > a.saturating_add(2) {
                out.push(format!("{} rose {a}% → {b}% during the soak", c.name));
            }
        }
    }
    out
}

pub fn html_block(facts: &Facts) -> String {
    let mut s = String::new();
    if !facts.cells.is_empty() {
        s.push_str("<h2>cells</h2>\n<table class=\"cells\"><thead><tr>");
        s.push_str(
            "<th></th><th>start</th><th>last</th><th>min</th><th>drain</th></tr></thead><tbody>",
        );
        for c in &facts.cells {
            s.push_str(&format!(
                "<tr><th>{}</th><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                c.name,
                pct(c.start),
                pct(c.end),
                pct(c.min),
                c.drain_per_hour
                    .map(|d| format!("{d:.0}%/h"))
                    .unwrap_or_else(|| "—".into()),
            ));
        }
        s.push_str("</tbody></table>\n");
    }
    if !facts.findings.is_empty() {
        s.push_str("<aside class=\"findings\"><h2>findings</h2><ul>");
        for f in &facts.findings {
            s.push_str(&format!("<li>{}</li>", esc(f)));
        }
        s.push_str("</ul></aside>\n");
    }
    s
}

pub fn text_block(facts: &Facts) -> String {
    let mut s = String::new();
    if !facts.cells.is_empty() {
        s.push_str("cells:\n");
        for c in &facts.cells {
            s.push_str(&format!(
                "  {:<6} start {}  last {}  min {}  {}\n",
                c.name,
                pct(c.start),
                pct(c.end),
                pct(c.min),
                c.drain_per_hour
                    .map(|d| format!("{d:.0}%/h"))
                    .unwrap_or_else(|| "—".into()),
            ));
        }
    }
    if facts.findings.is_empty() {
        s.push_str("findings:  none\n");
    } else {
        s.push_str("findings:\n");
        for f in &facts.findings {
            s.push_str(&format!("  - {f}\n"));
        }
    }
    s
}

fn pct(p: Option<u8>) -> String {
    p.map(|n| format!("{n}%")).unwrap_or_else(|| "—".into())
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
    fn drain_is_fifty_percent_over_two_hours() {
        let samples = [
            sample(0, Some(90), Some(90)),
            sample(7_200_000, Some(40), Some(40)),
        ];
        let f = collect(&samples, &[]);
        let left = f.cells.iter().find(|c| c.name == "left").unwrap();
        assert_eq!(left.start, Some(90));
        assert_eq!(left.end, Some(40));
        let drain = left.drain_per_hour.expect("drain");
        assert!((drain - 25.0).abs() < 0.01, "{drain}");
        assert!(
            text_block(&f).contains("left") && text_block(&f).contains("25%/h"),
            "{}",
            text_block(&f)
        );
    }

    #[test]
    fn imbalance_and_empty_playing_are_findings() {
        let samples = [
            sample(0, Some(90), Some(70)),
            sample(600_000, Some(80), Some(65)),
        ];
        let f = collect(&samples, &[(0, EventKind::ReportedEmptyStillPlaying)]);
        assert_eq!(f.lr_gap, Some(20));
        assert!(
            f.findings
                .iter()
                .any(|s| s.contains("differed by up to 20%")),
            "{:?}",
            f.findings
        );
        assert!(
            f.findings
                .iter()
                .any(|s| s.contains("empty while still playing")),
            "{:?}",
            f.findings
        );
    }

    #[test]
    fn no_findings_when_curve_is_quiet() {
        let samples = [
            sample(0, Some(50), Some(50)),
            sample(120_000, Some(49), Some(49)),
        ];
        let f = collect(&samples, &[]);
        assert!(f.findings.is_empty(), "{:?}", f.findings);
        assert!(text_block(&f).contains("findings:  none"));
        assert!(!html_block(&f).contains("findings"));
    }
}

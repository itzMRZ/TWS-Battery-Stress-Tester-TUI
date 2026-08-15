//! Support pack: HTML, CSV, JSONL, and TXT written as the soak runs.

mod chart;
mod facts;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};

use crate::alias::sanitize;
use crate::cells;
use crate::death::EventKind;
use crate::device::{FoundDevice, Sample, SoakKind, Stimulus};

pub fn library_root() -> PathBuf {
    dirs::document_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("tws-tester")
}

pub fn runs_root() -> PathBuf {
    library_root().join("runs")
}

pub fn device_dir(alias: &str) -> PathBuf {
    runs_root().join(sanitize(alias))
}

pub fn soak_dir(alias: &str, started: DateTime<Local>, kind: SoakKind) -> PathBuf {
    let stamp = started.format("%Y-%m-%dT%H%M");
    device_dir(alias).join(format!("{stamp}-{}", kind.slug()))
}

pub struct PackWriter {
    dir: PathBuf,
    jsonl: PathBuf,
}

impl PackWriter {
    pub fn create(
        alias: &str,
        started: DateTime<Local>,
        kind: SoakKind,
        device: &FoundDevice,
        stimulus: Stimulus,
    ) -> std::io::Result<Self> {
        let dir = soak_dir(alias, started, kind);
        fs::create_dir_all(&dir)?;
        let jsonl = dir.join("session.jsonl");
        let mut w = OpenOptions::new().create(true).append(true).open(&jsonl)?;
        writeln!(
            w,
            "{}",
            serde_json::json!({
                "kind": "start",
                "t": started.to_rfc3339(),
                "alias": alias,
                "name": device.name,
                "address": device.address,
                "brand": device.brand.slug,
                "brand_family": device.brand.family,
                "chip": device.chip,
                "services": device.services(),
                "soak": kind.slug(),
                "stimulus": stimulus.label(),
                "codec": device.codec,
                "host_volume": device.host_volume,
                "headset_volume": device.headset_volume,
                "anc": match &device.anc {
                    crate::device::AncKnowledge::Unknown => "unknown".to_string(),
                    crate::device::AncKnowledge::Known { mode, .. } => mode.label().to_string(),
                }
            })
        )?;
        let s = Self { dir, jsonl };
        s.rewrite_human(
            alias,
            started,
            kind,
            device,
            stimulus,
            &[],
            &[],
            None,
            "in progress",
        )?;
        Ok(s)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn append_sample(&self, sample: &Sample) -> std::io::Result<()> {
        let mut w = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.jsonl)?;
        writeln!(
            w,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "kind": "sample",
                "t": sample.t.to_rfc3339(),
                "elapsed_ms": sample.elapsed_ms,
                "present": sample.present,
                "codec": sample.codec,
                "host_volume": sample.host_volume,
                "cells": sample.cells,
            }))?
        )
    }

    pub fn append_event(
        &self,
        t: DateTime<Local>,
        elapsed_ms: u64,
        event: EventKind,
    ) -> std::io::Result<()> {
        let mut w = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.jsonl)?;
        writeln!(
            w,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "kind": "event",
                "t": t.to_rfc3339(),
                "elapsed_ms": elapsed_ms,
                "tag": event.tag(),
                "event": event.to_string(),
            }))?
        )
    }

    pub fn rewrite_human(
        &self,
        alias: &str,
        started: DateTime<Local>,
        kind: SoakKind,
        device: &FoundDevice,
        stimulus: Stimulus,
        samples: &[Sample],
        events: &[(u64, EventKind)],
        stop: Option<&str>,
        status: &str,
    ) -> std::io::Result<()> {
        fs::write(self.dir.join("samples.csv"), csv(samples))?;
        fs::write(self.dir.join("events.csv"), events_csv(events))?;
        fs::write(
            self.dir.join("summary.txt"),
            summary(
                alias, started, kind, device, stimulus, samples, events, stop, status,
            ),
        )?;
        fs::write(
            self.dir.join("report.html"),
            html(
                alias, started, kind, device, stimulus, samples, events, stop, status,
            ),
        )?;
        rewrite_index(alias)?;
        Ok(())
    }
}

fn csv(samples: &[Sample]) -> String {
    let mut out = String::from("elapsed_s,headline,pair,left,right,case,present,codec\n");
    for s in samples {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            s.elapsed_ms / 1000,
            csv_pct(cells::headline(&s.cells)),
            csv_pct(cells::named_percent(&s.cells, "pair")),
            csv_pct(cells::named_percent(&s.cells, "left")),
            csv_pct(cells::named_percent(&s.cells, "right")),
            csv_pct(cells::named_percent(&s.cells, "case")),
            s.present,
            s.codec.clone().unwrap_or_default()
        ));
    }
    out
}

fn events_csv(events: &[(u64, EventKind)]) -> String {
    let mut out = String::from("elapsed_s,tag,label\n");
    for (ms, e) in events {
        out.push_str(&format!(
            "{},{},{}\n",
            ms / 1000,
            e.tag(),
            csv_escape(&e.to_string())
        ));
    }
    out
}

fn csv_pct(p: Option<u8>) -> String {
    p.map(|n| n.to_string()).unwrap_or_default()
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn summary(
    alias: &str,
    started: DateTime<Local>,
    kind: SoakKind,
    device: &FoundDevice,
    stimulus: Stimulus,
    samples: &[Sample],
    events: &[(u64, EventKind)],
    stop: Option<&str>,
    status: &str,
) -> String {
    let facts = facts::collect(samples, events);
    let elapsed = facts.elapsed_ms;
    let mut s = String::new();
    s.push_str("tws-tester support pack\n");
    s.push_str("=======================\n\n");
    s.push_str(&format!("alias:     {alias}\n"));
    s.push_str(&format!("name:      {}\n", device.name));
    s.push_str(&format!("address:   {}\n", device.address));
    s.push_str(&format!(
        "brand:     {}\n",
        if device.brand.slug == device.brand.family.label() {
            device.brand.slug.to_string()
        } else {
            format!(
                "{} (family {})",
                device.brand.slug,
                device.brand.family.label()
            )
        }
    ));
    s.push_str(&format!("class:     {}\n", device.class.label()));
    if let Some(chip) = &device.chip {
        s.push_str(&format!("chip:      {chip}\n"));
    }
    let services = device.services();
    if !services.is_empty() {
        s.push_str(&format!("services:  {}\n", services.join(" ")));
    }
    s.push_str(&format!("soak:      {}\n", kind.label()));
    s.push_str(&format!(
        "started:   {}\n",
        started.format("%Y-%m-%d %H:%M:%S")
    ));
    s.push_str(&format!("elapsed:   {}\n", fmt_elapsed(elapsed)));
    s.push_str(&format!("status:    {status}\n"));
    if let Some(stop) = stop {
        s.push_str(&format!("stop:      {stop}\n"));
    }
    s.push_str(&format!("stimulus:  {}\n", stimulus.label()));
    s.push_str(&format!("codec:     {}\n", device.pretty_codec()));
    s.push_str(&format!(
        "volume:    host {}  headset {}\n",
        device
            .host_volume
            .map(|v| format!("{v}%"))
            .unwrap_or_else(|| "unknown".into()),
        device
            .headset_volume
            .map(|v| format!("{v}%"))
            .unwrap_or_else(|| "unknown".into()),
    ));
    s.push_str(&format!(
        "anc:       {}\n",
        match device.anc {
            crate::device::AncKnowledge::Unknown => "unknown (cannot switch on this stack)",
            crate::device::AncKnowledge::Known { mode, .. } => mode.label(),
        }
    ));
    s.push('\n');
    s.push_str(&facts::text_block(&facts));
    s.push('\n');
    if events.is_empty() {
        s.push_str("events:    none\n");
    } else {
        s.push_str("events:\n");
        for (ms, e) in events {
            s.push_str(&format!("  - {}  [{}]  {e}\n", fmt_elapsed(*ms), e.tag()));
        }
    }
    s
}

fn html(
    alias: &str,
    started: DateTime<Local>,
    kind: SoakKind,
    device: &FoundDevice,
    stimulus: Stimulus,
    samples: &[Sample],
    events: &[(u64, EventKind)],
    stop: Option<&str>,
    status: &str,
) -> String {
    let facts = facts::collect(samples, events);
    let charts = chart::report_charts(samples, events);
    let event_lis: String = events
        .iter()
        .map(|(ms, e)| {
            format!(
                "<li><code>{}</code> <code class=\"tag\">{}</code> {}</li>",
                fmt_elapsed(*ms),
                esc(e.tag()),
                esc(&e.to_string())
            )
        })
        .collect();
    let stop_l = stop.unwrap_or("—");
    let (br, bgc, bb) = device.brand.rgb();
    let ink = if device.brand.ink_light() {
        "#e6e1d6"
    } else {
        "#0d0f12"
    };
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>{alias}</title>
<style>
  :root {{ color-scheme: dark; }}
  body {{ margin: 0; font: 15px/1.45 ui-sans-serif, system-ui, sans-serif; background: #0d0f12; color: #e6e1d6; }}
  header {{ padding: 28px 32px 8px; }}
  h1 {{ font-weight: 600; font-size: 22px; margin: 0; letter-spacing: -0.02em; }}
  h2 {{ font-size: 14px; font-weight: 600; letter-spacing: 0.04em; text-transform: uppercase; color: #8b909a; margin: 28px 0 10px; }}
  .sub {{ color: #8b909a; margin-top: 4px; }}
  .brand {{ display: inline-block; font: 12px/1.2 ui-monospace, monospace; padding: 4px 8px; border-radius: 4px; margin: 8px 0 0; letter-spacing: 0.06em; }}
  main {{ padding: 8px 32px 48px; max-width: 880px; }}
  figure {{ margin: 0 0 20px; }}
  figcaption {{ color: #8b909a; font-size: 13px; margin: 0 0 8px; }}
  svg {{ width: 100%; height: auto; display: block; }}
  dl {{ display: grid; grid-template-columns: 140px 1fr; gap: 6px 16px; }}
  dt {{ color: #8b909a; }}
  ul {{ padding-left: 18px; }}
  a {{ color: #e8b86d; }}
  code.tag {{ color: #f38ba8; }}
  table.cells {{ border-collapse: collapse; width: 100%; font-variant-numeric: tabular-nums; }}
  table.cells th, table.cells td {{ text-align: left; padding: 6px 10px 6px 0; border-bottom: 1px solid #313244; }}
  table.cells th {{ color: #8b909a; font-weight: 500; }}
  .findings {{ border-left: 3px solid #f38ba8; padding: 4px 0 4px 14px; margin: 16px 0 8px; }}
  .findings h2 {{ margin-top: 0; color: #f38ba8; }}
</style>
</head>
<body>
<header>
  <div class="sub">tws-tester</div>
  <div class="brand" style="background: rgb({br},{bgc},{bb}); color: {ink}">{display}</div>
  <h1>{alias}</h1>
  <div class="sub">{name} · {address}</div>
</header>
<main>
  {charts}
  {facts}
  <dl>
    <dt>soak</dt><dd>{kind} · {status}</dd>
    <dt>started</dt><dd>{started}</dd>
    <dt>stimulus</dt><dd>{stimulus}</dd>
    <dt>codec</dt><dd>{codec}</dd>
    <dt>stop</dt><dd>{stop}</dd>
  </dl>
  <h2>events</h2>
  <ul>{events}</ul>
  <p class="sub"><code>summary.txt</code> is the text dump. <code>samples.csv</code> is the curve. <code>events.csv</code> is the tagged timeline. This page rewrites while a soak is live.</p>
</main>
</body>
</html>
"##,
        alias = esc(alias),
        name = esc(&device.name),
        address = esc(&device.address),
        kind = esc(kind.label()),
        status = esc(status),
        started = esc(&started.format("%Y-%m-%d %H:%M:%S").to_string()),
        stimulus = esc(stimulus.label()),
        codec = esc(&device.pretty_codec()),
        stop = esc(stop_l),
        charts = charts,
        facts = facts::html_block(&facts),
        br = br,
        bgc = bgc,
        bb = bb,
        ink = ink,
        display = esc(device.brand.display_name()),
        events = if event_lis.is_empty() {
            "<li>none</li>".into()
        } else {
            event_lis
        },
    )
}

fn rewrite_index(alias: &str) -> std::io::Result<()> {
    let dir = device_dir(alias);
    let mut soaks = Vec::new();
    if dir.exists() {
        for e in fs::read_dir(&dir)? {
            let e = e?;
            if e.file_type()?.is_dir() {
                let name = e.file_name().to_string_lossy().into_owned();
                soaks.push(name);
            }
        }
    }
    soaks.sort();
    soaks.reverse();
    let links: String = soaks
        .iter()
        .map(|s| format!("<li><a href=\"{s}/report.html\">{s}</a></li>"))
        .collect();
    let body = format!(
        r##"<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8"/><title>{alias}</title>
<style>
body {{ margin: 32px; font: 15px/1.45 ui-sans-serif, system-ui; background: #0d0f12; color: #e6e1d6; }}
a {{ color: #e8b86d; }}
.sub {{ color: #8b909a; }}
</style></head>
<body>
<div class="sub">tws-tester</div>
<h1>{alias}</h1>
<p class="sub">pick two reports to compare by opening them. overlay from the TUI writes overlay.html here.</p>
<ul>{links}</ul>
</body></html>
"##,
        alias = esc(alias),
        links = if links.is_empty() {
            "<li>no soaks yet</li>".into()
        } else {
            links
        },
    );
    fs::write(dir.join("index.html"), body)
}

pub fn write_overlay(alias: &str, a: &str, b: &str) -> std::io::Result<PathBuf> {
    let dir = device_dir(alias);
    let pa = load_percents(&dir.join(a).join("samples.csv"));
    let pb = load_percents(&dir.join(b).join("samples.csv"));
    let series = [
        chart::Series::named(a, chart::LEFT, pa),
        chart::Series::named(b, chart::RIGHT, pb),
    ];
    let plot = chart::figure("overlay", &series, &[], false);
    let html = format!(
        r##"<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8"/><title>{alias} overlay</title>
<style>
body {{ margin: 32px; font: 15px/1.45 ui-sans-serif, system-ui; background: #0d0f12; color: #e6e1d6; }}
figure {{ margin: 0; }}
figcaption {{ color: #8b909a; font-size: 13px; margin: 0 0 8px; }}
svg {{ width: 100%; height: auto; display: block; }}
.a {{ color: {left}; }} .b {{ color: {right}; }}
</style></head>
<body>
<h1>{alias}</h1>
<p><span class="a">● {a}</span> &nbsp; <span class="b">● {b}</span></p>
{plot}
</body></html>
"##,
        left = chart::LEFT,
        right = chart::RIGHT,
    );
    let dest = dir.join("overlay.html");
    fs::write(&dest, html)?;
    Ok(dest)
}

fn load_percents(csv_path: &Path) -> Vec<(f64, f64)> {
    let Ok(text) = fs::read_to_string(csv_path) else {
        return Vec::new();
    };
    text.lines()
        .skip(1)
        .filter_map(|l| {
            let mut p = l.split(',');
            let t: f64 = p.next()?.parse().ok()?;
            let pct: f64 = p.next()?.parse().ok()?;
            Some((t, pct))
        })
        .collect()
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn fmt_elapsed(ms: u64) -> String {
    let s = ms / 1000;
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    if h > 0 {
        format!("{h}h {m:02}m {sec:02}s")
    } else {
        format!("{m:02}:{sec:02}")
    }
}

pub fn list_soaks(alias: &str) -> Vec<String> {
    let dir = device_dir(alias);
    let mut soaks = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                soaks.push(e.file_name().to_string_lossy().into_owned());
            }
        }
    }
    soaks.sort();
    soaks.reverse();
    soaks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::SoakKind;
    use chrono::TimeZone;

    #[test]
    fn soak_folder_nests_under_alias() {
        let t = chrono::Local
            .with_ymd_and_hms(2026, 8, 15, 1, 46, 0)
            .single()
            .expect("valid local time");
        let p = soak_dir("Soundcore P30i", t, SoakKind::Remaining);
        let s = p.to_string_lossy().replace('\\', "/");
        assert!(s.contains("runs/Soundcore P30i/"));
        assert!(s.ends_with("2026-08-15T0146-remaining"));
    }

    #[test]
    fn events_csv_uses_stable_tags() {
        let csv = events_csv(&[
            (0, EventKind::Blip),
            (12_000, EventKind::ReportedEmptyStillPlaying),
        ]);
        assert!(csv.starts_with("elapsed_s,tag,label\n"));
        assert!(csv.contains("0,blip,brief disconnect"));
        assert!(csv.contains("12,empty_playing,"));
    }
}

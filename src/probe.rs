//! Capture host + Device facts so a new Device or a new OS can be supported.

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use chrono::Local;
use serde::Serialize;

use crate::brand::Family;
use crate::device::{AncKnowledge, DeviceClass, FoundDevice};
use crate::host::Host;
use crate::pack;

#[derive(Serialize)]
pub struct Snapshot {
    pub captured: String,
    pub os: String,
    pub arch: String,
    pub family: String,
    pub host_error: Option<String>,
    pub devices: Vec<DeviceFact>,
    pub logs: Vec<NamedLog>,
}

#[derive(Serialize, Clone)]
pub struct DeviceFact {
    pub address: String,
    pub name: String,
    pub connected: bool,
    pub paired: bool,
    pub bonded: bool,
    pub class: String,
    pub brand: String,
    pub brand_family: String,
    pub cells: Vec<CellFact>,
    pub codec: Option<String>,
    pub pretty_codec: String,
    pub profiles: Vec<String>,
    pub host_volume: Option<u8>,
    pub headset_volume: Option<u8>,
    pub anc: String,
    pub sink: Option<String>,
    pub card: Option<String>,
    pub uuids: Vec<String>,
    pub services: Vec<String>,
    pub address_type: Option<String>,
    pub rssi: Option<i16>,
    pub modalias: Option<String>,
    pub chip: Option<String>,
    pub gaps: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct CellFact {
    pub name: String,
    pub percent: Option<u8>,
    pub source: String,
}

#[derive(Serialize, Clone)]
pub struct NamedLog {
    pub name: String,
    pub body: String,
}

pub async fn run() -> Result<PathBuf> {
    let snap = collect().await;
    let dir = write(&snap)?;
    println!("{}", dir.display());
    println!("{}", render(&snap));
    Ok(dir)
}

pub async fn collect() -> Snapshot {
    let host = Host::connect().await;
    let mut devices = host.scan().await.unwrap_or_default();
    if !devices.is_empty() {
        tokio::time::sleep(std::time::Duration::from_millis(1800)).await;
        if let Ok(again) = host.scan().await {
            devices = again;
        }
    }
    let addrs: Vec<String> = devices.iter().map(|d| d.address.clone()).collect();
    let logs = host
        .probe_logs(&addrs)
        .into_iter()
        .map(|(name, body)| NamedLog { name, body })
        .collect();
    Snapshot {
        captured: Local::now().to_rfc3339(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        family: std::env::consts::FAMILY.to_string(),
        host_error: host.error().map(str::to_string),
        devices: devices.iter().map(DeviceFact::from).collect(),
        logs,
    }
}

pub fn write(snap: &Snapshot) -> Result<PathBuf> {
    let stamp = Local::now().format("%Y-%m-%dT%H%M");
    let dir = pack::library_root()
        .join("probes")
        .join(format!("{stamp}-{}", snap.os));
    fs::create_dir_all(&dir)?;
    fs::write(dir.join("probe.json"), serde_json::to_string_pretty(snap)?)?;
    fs::write(dir.join("probe.md"), render(snap))?;
    for log in &snap.logs {
        let name = sanitize_log_name(&log.name);
        fs::write(dir.join(format!("{name}.txt")), &log.body)?;
    }
    Ok(dir)
}

pub fn render(snap: &Snapshot) -> String {
    let mut s = String::new();
    s.push_str("# tws-tester probe\n\n");
    s.push_str(&format!("captured:  {}\n", snap.captured));
    s.push_str(&format!("os:        {} {}\n", snap.os, snap.arch));
    s.push_str(&format!("family:    {}\n", snap.family));
    match &snap.host_error {
        Some(e) => s.push_str(&format!("host:      {e}\n")),
        None => s.push_str("host:      ok\n"),
    }
    s.push('\n');

    let host_gaps = host_gaps(snap);
    let device_gaps: Vec<&str> = snap
        .devices
        .iter()
        .flat_map(|d| d.gaps.iter().map(String::as_str))
        .collect();
    s.push_str("## gaps\n\n");
    if host_gaps.is_empty() && device_gaps.is_empty() {
        s.push_str("none; this look is enough to soak on this host.\n\n");
    } else {
        for g in &host_gaps {
            s.push_str(&format!("- {g}\n"));
        }
        for d in &snap.devices {
            for g in &d.gaps {
                s.push_str(&format!("- {}: {g}\n", d.name));
            }
        }
        s.push('\n');
    }

    if snap.devices.is_empty() {
        s.push_str("## devices\n\nnone seen. Pair and connect the Device, then probe again.\n\n");
    } else {
        s.push_str("## devices\n\n");
        for d in &snap.devices {
            s.push_str(&format!("### {}  `{}`\n\n", d.name, d.address));
            s.push_str(&format!(
                "- class {} · brand {} · family {}\n",
                d.class, d.brand, d.brand_family
            ));
            s.push_str(&format!(
                "- connected {} · paired {} · bonded {}\n",
                d.connected, d.paired, d.bonded
            ));
            if let Some(t) = &d.address_type {
                s.push_str(&format!("- address type {t}\n"));
            }
            if let Some(r) = d.rssi {
                s.push_str(&format!("- rssi {r}\n"));
            }
            if let Some(m) = &d.modalias {
                s.push_str(&format!("- modalias `{m}`\n"));
            }
            if let Some(c) = &d.chip {
                s.push_str(&format!("- chip {c}\n"));
            }
            s.push_str(&format!("- codec {}\n", d.pretty_codec));
            if !d.profiles.is_empty() {
                s.push_str(&format!("- profiles {}\n", d.profiles.join(", ")));
            }
            s.push_str(&format!(
                "- volume host {}  headset {}\n",
                opt_u8(d.host_volume),
                opt_u8(d.headset_volume)
            ));
            s.push_str(&format!("- anc {}\n", d.anc));
            s.push_str(&format!("- sink {}\n", d.sink.as_deref().unwrap_or("—")));
            s.push_str(&format!("- card {}\n", d.card.as_deref().unwrap_or("—")));
            s.push_str(&format!("- services {}\n", d.services.join(" ")));
            if !d.uuids.is_empty() {
                s.push_str("- uuids\n");
                for u in &d.uuids {
                    s.push_str(&format!("  - `{u}`\n"));
                }
            }
            if d.cells.is_empty() {
                s.push_str("- cells none\n");
            } else {
                s.push_str("- cells\n");
                for c in &d.cells {
                    s.push_str(&format!(
                        "  - {} {} ({})\n",
                        c.name,
                        opt_u8(c.percent),
                        c.source
                    ));
                }
            }
            s.push('\n');
        }
    }

    if !snap.logs.is_empty() {
        s.push_str("## host logs\n\n");
        s.push_str("Raw command output is also in `*.txt` beside this file.\n\n");
        for log in &snap.logs {
            s.push_str(&format!(
                "### {}\n\n```\n{}\n```\n\n",
                log.name,
                clip(&log.body, 8000)
            ));
        }
    }
    s
}

pub fn device_gaps(d: &FoundDevice) -> Vec<String> {
    let mut g = Vec::new();
    if d.brand.family == Family::Unknown {
        g.push("brand unknown; advertised name and UUIDs did not match a Family".into());
    } else if !d.brand.family.cell_probe() {
        g.push(format!(
            "family {} has no cell probe yet",
            d.brand.family.label()
        ));
    }
    if d.cells.is_empty() {
        g.push("no cells".into());
    } else if d.class == DeviceClass::Tws {
        let left = d
            .cells
            .iter()
            .any(|c| c.name == "left" && c.percent.is_some());
        let right = d
            .cells
            .iter()
            .any(|c| c.name == "right" && c.percent.is_some());
        if !left || !right {
            g.push("TWS without split left/right percent".into());
        }
    }
    if d.connected && d.sink.is_none() {
        g.push("connected but no audio sink".into());
    }
    if d.connected && d.codec.is_none() {
        g.push("connected but codec unknown".into());
    }
    if d.uuids
        .iter()
        .any(|u| u.to_ascii_lowercase().contains("00001101"))
        && !d.brand.family.cell_probe()
    {
        g.push("SPP present; a family adapter can talk RFCOMM".into());
    }
    g
}

fn host_gaps(snap: &Snapshot) -> Vec<String> {
    let mut g = Vec::new();
    if let Some(e) = &snap.host_error {
        g.push(format!("no Host adapter ({e})"));
    }
    if snap.os == "windows" {
        g.push("Windows Host is experimental; this folder is the capture for extending it".into());
    }
    if snap.devices.is_empty() && snap.host_error.is_none() {
        g.push("scan returned no audio Device".into());
    }
    g
}

impl From<&FoundDevice> for DeviceFact {
    fn from(d: &FoundDevice) -> Self {
        Self {
            address: d.address.clone(),
            name: d.name.clone(),
            connected: d.connected,
            paired: d.paired,
            bonded: d.bonded,
            class: d.class.label().into(),
            brand: d.brand.slug.into(),
            brand_family: d.brand.family.label().into(),
            cells: d
                .cells
                .iter()
                .map(|c| CellFact {
                    name: c.name.clone(),
                    percent: c.percent,
                    source: c.source.clone(),
                })
                .collect(),
            codec: d.codec.clone(),
            pretty_codec: d.pretty_codec(),
            profiles: d.codec_choices(),
            host_volume: d.host_volume,
            headset_volume: d.headset_volume,
            anc: match d.anc {
                AncKnowledge::Unknown => "unknown".into(),
                AncKnowledge::Known { mode, can_set } => {
                    format!("{} (can_set {can_set})", mode.label())
                }
            },
            sink: d.sink.clone(),
            card: d.card.clone(),
            uuids: d.uuids.clone(),
            services: d.services().into_iter().map(str::to_string).collect(),
            address_type: d.address_type.clone(),
            rssi: d.rssi,
            modalias: d.modalias.clone(),
            chip: d.chip.clone(),
            gaps: device_gaps(d),
        }
    }
}

fn opt_u8(v: Option<u8>) -> String {
    v.map(|n| format!("{n}%")).unwrap_or_else(|| "—".into())
}

fn clip(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…\n", &s[..max])
    }
}

fn sanitize_log_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brand::Brand;
    use crate::cells;
    use crate::device::{AncKnowledge, CellReading, DeviceClass, FoundDevice};
    use std::collections::HashMap;

    fn device(
        name: &str,
        class: DeviceClass,
        brand: Brand,
        cells: Vec<CellReading>,
        uuids: Vec<String>,
        connected: bool,
        sink: Option<&str>,
        codec: Option<&str>,
    ) -> FoundDevice {
        FoundDevice {
            address: "AA:BB:CC:DD:EE:FF".into(),
            name: name.into(),
            connected,
            paired: true,
            class,
            cells,
            codec: codec.map(str::to_string),
            profiles: Vec::new(),
            profile_labels: HashMap::new(),
            host_volume: None,
            headset_volume: None,
            anc: AncKnowledge::Unknown,
            sink: sink.map(str::to_string),
            card: None,
            uuids,
            brand,
            bonded: true,
            address_type: Some("public".into()),
            rssi: Some(-42),
            modalias: Some("bluetooth:v05D6p000Ad0240".into()),
            chip: Some("JieLi 05D6:000A".into()),
        }
    }

    #[test]
    fn unknown_tws_lists_the_gaps_a_family_adapter_needs() {
        let d = device(
            "Mystery Buds",
            DeviceClass::Tws,
            Brand::UNKNOWN,
            vec![],
            vec!["00001101-0000-1000-8000-00805f9b34fb".into()],
            true,
            None,
            None,
        );
        let g = device_gaps(&d);
        assert!(g.iter().any(|s| s.contains("brand unknown")), "{g:?}");
        assert!(g.iter().any(|s| s.contains("no cells")), "{g:?}");
        assert!(g.iter().any(|s| s.contains("SPP")), "{g:?}");
        assert!(g.iter().any(|s| s.contains("no audio sink")), "{g:?}");
    }

    #[test]
    fn soundcore_with_split_cells_is_quiet() {
        let d = device(
            "soundcore P30i",
            DeviceClass::Tws,
            Brand::detect("soundcore P30i", &[]),
            vec![
                cells::cell("left", Some(80), "rfcomm"),
                cells::cell("right", Some(78), "rfcomm"),
            ],
            vec![],
            true,
            Some("bluez_sink.18_9C"),
            Some("a2dp_sink_aac"),
        );
        let g = device_gaps(&d);
        assert!(g.is_empty(), "{g:?}");
    }

    #[test]
    fn render_includes_windows_host_gap_and_device_facts() {
        let d = device(
            "Galaxy Buds2 Pro",
            DeviceClass::Tws,
            Brand::detect("Galaxy Buds2 Pro", &[]),
            vec![cells::cell("pair", Some(50), "os")],
            vec!["0000110b-0000-1000-8000-00805f9b34fb".into()],
            true,
            Some("sink"),
            Some("a2dp"),
        );
        let snap = Snapshot {
            captured: "2026-08-15T04:42:00+06:00".into(),
            os: "windows".into(),
            arch: "x86_64".into(),
            family: "windows".into(),
            host_error: Some("this OS is experimental; Linux is the tested path".into()),
            devices: vec![DeviceFact::from(&d)],
            logs: vec![NamedLog {
                name: "windows-pnp-bluetooth".into(),
                body: "Status  FriendlyName\nOK      Galaxy Buds2 Pro".into(),
            }],
        };
        let md = render(&snap);
        assert!(md.contains("Windows Host is experimental"), "{md}");
        assert!(md.contains("Galaxy Buds2 Pro"), "{md}");
        assert!(md.contains("AA:BB:CC:DD:EE:FF"), "{md}");
        assert!(md.contains("windows-pnp-bluetooth"), "{md}");
        assert!(md.contains("TWS without split left/right"), "{md}");
        assert!(md.contains("modalias"), "{md}");
    }

    #[test]
    fn log_names_are_safe_filenames() {
        assert_eq!(sanitize_log_name("windows pnp"), "windows-pnp");
        assert_eq!(sanitize_log_name("pactl/cards"), "pactl-cards");
    }
}

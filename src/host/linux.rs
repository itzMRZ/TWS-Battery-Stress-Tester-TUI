//! BlueZ D-Bus, PipeWire playback, and logind sleep inhibit.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zbus::Connection;

use super::PlayHandle;
use crate::brand::{Brand, CellTransport, Family};
use crate::cells;
use crate::device::{parse_modalias, AncKnowledge, CellReading, DeviceClass, FoundDevice};

const AUDIO_SINK: &str = "0000110b-0000-1000-8000-00805f9b34fb";
const HEADSET: &str = "00001108-0000-1000-8000-00805f9b34fb";
const HANDSFREE: &str = "0000111e-0000-1000-8000-00805f9b34fb";
const SPEAKER: &str = "0000110a-0000-1000-8000-00805f9b34fb";

type Ifaces = HashMap<String, HashMap<String, OwnedValue>>;
type Managed = HashMap<OwnedObjectPath, Ifaces>;

struct ExtraCache {
    cells: Vec<CellReading>,
    channel: Option<u8>,
    until: Instant,
}

pub struct LinuxHost {
    conn: Connection,
    extras: Arc<Mutex<HashMap<String, ExtraCache>>>,
    inflight: Arc<Mutex<HashSet<String>>>,
}

impl LinuxHost {
    pub async fn connect() -> Result<Self> {
        let conn = Connection::system().await.context("system bus")?;
        let dbus = zbus::fdo::DBusProxy::new(&conn)
            .await
            .context("system bus")?;
        let running = dbus
            .name_has_owner("org.bluez".try_into().context("bus name")?)
            .await
            .context("system bus")?;
        if !running {
            anyhow::bail!("bluetoothd is not running");
        }
        Ok(Self {
            conn,
            extras: Arc::new(Mutex::new(HashMap::new())),
            inflight: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    pub async fn scan(&self) -> Result<Vec<FoundDevice>> {
        let proxy = zbus::Proxy::new(
            &self.conn,
            "org.bluez",
            "/",
            "org.freedesktop.DBus.ObjectManager",
        )
        .await?;
        let objects: Managed =
            tokio::time::timeout(Duration::from_secs(3), proxy.call("GetManagedObjects", &()))
                .await
                .context("bluetoothd did not answer")?
                .context("GetManagedObjects")?;
        let pulse = pulse_snapshot().await;

        let mut out = Vec::new();
        for (path, ifaces) in &objects {
            let Some(dev) = ifaces.get("org.bluez.Device1") else {
                continue;
            };
            let uuids = str_array(dev.get("UUIDs"));
            let icon = str_prop(dev, "Icon");
            let has_battery = ifaces.contains_key("org.bluez.Battery1");
            if !is_audio(&uuids) && !icon_is_audio(icon.as_deref()) && !has_battery {
                continue;
            }
            let address = str_prop(dev, "Address").unwrap_or_default();
            if address.is_empty() {
                continue;
            }
            let name = str_prop(dev, "Name")
                .or_else(|| str_prop(dev, "Alias"))
                .unwrap_or_else(|| address.clone());
            let connected = bool_prop(dev, "Connected").unwrap_or(false);
            let paired = bool_prop(dev, "Paired").unwrap_or(false);
            let bonded = bool_prop(dev, "Bonded").unwrap_or(paired);
            let address_type = str_prop(dev, "AddressType");
            let rssi = i16_prop(dev, "RSSI");
            let modalias = str_prop(dev, "Modalias");
            let chip = modalias.as_deref().and_then(chip_label);
            let os_cells = bluez_cells(&objects, path.as_str());
            let extra = if connected {
                self.cached_extra(&address)
            } else {
                self.drop_extra(&address);
                Vec::new()
            };
            let cells = cells::merge(os_cells, extra);
            let key = mac_underscores(&address);
            let sink = pulse.sinks.iter().find(|s| s.contains(&key)).cloned();
            let card = pulse.cards.iter().find(|c| c.name.contains(&key)).cloned();
            let (codec, profiles, labels, host_volume) = if let Some(c) = &card {
                (
                    c.active.clone(),
                    c.profiles.clone(),
                    c.labels.clone(),
                    sink.as_deref().and_then(|s| pulse.volumes.get(s).copied()),
                )
            } else {
                (None, Vec::new(), HashMap::new(), None)
            };
            let class = DeviceClass::guess(&name, icon.as_deref());
            let brand = Brand::detect(&name, &uuids);
            if connected {
                self.kick_family_probe(&address, brand.family);
            }
            out.push(FoundDevice {
                class,
                address,
                name,
                connected,
                paired,
                cells,
                codec,
                profiles,
                profile_labels: labels,
                host_volume,
                headset_volume: None,
                anc: AncKnowledge::Unknown,
                sink,
                card: card.map(|c| c.name),
                brand,
                uuids,
                bonded,
                address_type,
                rssi,
                modalias,
                chip,
            });
        }
        out.sort_by(|a, b| b.connected.cmp(&a.connected).then(a.name.cmp(&b.name)));
        Ok(out)
    }

    fn cached_extra(&self, address: &str) -> Vec<CellReading> {
        self.extras
            .lock()
            .ok()
            .and_then(|g| g.get(address).map(|e| e.cells.clone()))
            .unwrap_or_default()
    }

    fn drop_extra(&self, address: &str) {
        if let Ok(mut g) = self.extras.lock() {
            g.remove(address);
        }
    }

    fn kick_family_probe(&self, address: &str, family: Family) {
        if !family.cell_probe() {
            return;
        }
        let now = Instant::now();
        let hint = {
            let Ok(g) = self.extras.lock() else {
                return;
            };
            match g.get(address) {
                Some(e) if e.until > now => return,
                Some(e) => e.channel,
                None => None,
            }
        };
        {
            let Ok(mut inf) = self.inflight.lock() else {
                return;
            };
            if !inf.insert(address.to_string()) {
                return;
            }
        }
        let extras = Arc::clone(&self.extras);
        let inflight = Arc::clone(&self.inflight);
        let address = address.to_string();
        tokio::task::spawn_blocking(move || {
            let (cells, channel) = match family.cell_transport() {
                CellTransport::Aap => (super::aap::probe(&address), None),
                CellTransport::Rfcomm => super::rfcomm::probe(&address, family, hint),
                CellTransport::None => (Vec::new(), None),
            };
            if let Ok(mut g) = extras.lock() {
                let empty = cells.is_empty();
                if empty {
                    if let Some(old) = g.get_mut(&address) {
                        if !old.cells.is_empty() {
                            old.until = Instant::now() + Duration::from_secs(8);
                            let _ = inflight.lock().map(|mut i| i.remove(&address));
                            return;
                        }
                    }
                }
                let ttl = if empty {
                    Duration::from_secs(5)
                } else {
                    Duration::from_secs(28)
                };
                g.insert(
                    address.clone(),
                    ExtraCache {
                        cells,
                        channel,
                        until: Instant::now() + ttl,
                    },
                );
            }
            let _ = inflight.lock().map(|mut i| i.remove(&address));
        });
    }
}

fn bluez_cells(objects: &Managed, dev_path: &str) -> Vec<CellReading> {
    let prefix = format!("{dev_path}/");
    let mut cells = Vec::new();
    let mut battery_n = 0usize;
    for (path, ifaces) in objects {
        let p = path.as_str();
        if p != dev_path && !p.starts_with(&prefix) {
            continue;
        }
        if let Some(bat) = ifaces.get("org.bluez.Battery1") {
            if let Some(pct) = u8_prop(bat, "Percentage") {
                battery_n += 1;
                let source = str_prop(bat, "Source").unwrap_or_default();
                let source = if source.is_empty() {
                    "bluez.Battery1".into()
                } else {
                    format!("bluez.{source}")
                };
                let name = battery_cell_name(&source, battery_n);
                cells.push(CellReading {
                    name,
                    percent: Some(pct),
                    source,
                });
            }
        }
        if let Some(ch) = ifaces.get("org.bluez.GattCharacteristic1") {
            let uuid = str_prop(ch, "UUID")
                .unwrap_or_default()
                .to_ascii_lowercase();
            if uuid.contains("00002a19") {
                if let Some(pct) = gatt_percent(ch) {
                    let name = gatt_cell_name(p);
                    cells.push(CellReading {
                        name,
                        percent: Some(pct),
                        source: "bluez.gatt".into(),
                    });
                }
            }
        }
    }
    cells
}

fn battery_cell_name(source: &str, n: usize) -> String {
    let s = source.to_ascii_lowercase();
    if s.contains("left") {
        "left".into()
    } else if s.contains("right") {
        "right".into()
    } else if s.contains("case") {
        "case".into()
    } else if n == 1 {
        "pair".into()
    } else {
        format!("pair{n}")
    }
}

fn gatt_cell_name(path: &str) -> String {
    let p = path.to_ascii_lowercase();
    if p.contains("left") {
        "left".into()
    } else if p.contains("right") {
        "right".into()
    } else if p.contains("case") {
        "case".into()
    } else {
        "gatt".into()
    }
}

fn gatt_percent(ch: &HashMap<String, OwnedValue>) -> Option<u8> {
    let v = ch.get("Value")?;
    let bytes = <Vec<u8>>::try_from(v.clone()).ok()?;
    let n = *bytes.first()?;
    (n <= 100).then_some(n)
}

pub fn set_host_volume(sink: &str, percent: u8) -> Result<()> {
    let status = Command::new("pactl")
        .args(["set-sink-volume", sink, &format!("{percent}%")])
        .status()?;
    if !status.success() {
        anyhow::bail!("pactl set-sink-volume failed");
    }
    Ok(())
}

pub fn set_profile(card: &str, profile: &str) -> Result<()> {
    let status = Command::new("pactl")
        .args(["set-card-profile", card, profile])
        .status()?;
    if !status.success() {
        anyhow::bail!("pactl set-card-profile failed");
    }
    Ok(())
}

pub fn play_loop(sink: Option<&str>, path: &Path) -> Result<PlayHandle> {
    if let Ok(h) = play_wav_stream(sink, path) {
        return Ok(h);
    }
    if let Ok(h) = play_ffmpeg_stream(sink, path) {
        return Ok(h);
    }
    play_restart_loop(sink, path)
}

/// One PipeWire stream; the 2 s pattern is written forever into the same sink.
fn play_wav_stream(sink: Option<&str>, path: &Path) -> Result<PlayHandle> {
    let (rate, channels, pcm) = wav_pcm(path)?;
    if pcm.is_empty() {
        anyhow::bail!("empty wav");
    }
    let mut cmd = Command::new("pw-play");
    cmd.args([
        "--raw",
        "--rate",
        &rate.to_string(),
        "--channels",
        &channels.to_string(),
        "--format",
        "s16",
        "--latency",
        "200ms",
    ]);
    if let Some(sink) = sink {
        cmd.args(["--target", sink]);
    }
    cmd.arg("-");
    let (mut handle, mut stdin) = PlayHandle::spawn_piped(cmd)?;
    let pump = std::thread::spawn(move || loop {
        if stdin.write_all(&pcm).is_err() {
            break;
        }
    });
    std::thread::sleep(Duration::from_millis(80));
    if !handle.alive() {
        anyhow::bail!("pw-play raw exited");
    }
    Ok(handle.with_pump(pump))
}

fn play_ffmpeg_stream(sink: Option<&str>, path: &Path) -> Result<PlayHandle> {
    if Command::new("ffmpeg")
        .args(["-version"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()
        .filter(|s| s.success())
        .is_none()
    {
        anyhow::bail!("no ffmpeg");
    }
    let file = sh_quote(&path.display().to_string());
    let target = sink
        .map(|s| format!("--target {} ", sh_quote(s)))
        .unwrap_or_default();
    let script = format!(
        "trap 'kill 0' EXIT; ffmpeg -nostdin -hide_banner -loglevel error -stream_loop -1 -i {file} -f s16le -ac 2 -ar 48000 - | pw-play --raw --rate 48000 --channels 2 --format s16 --latency 200ms {target}-"
    );
    let mut cmd = Command::new("bash");
    cmd.args(["-c", &script]);
    PlayHandle::spawn(cmd)
}

fn play_restart_loop(sink: Option<&str>, wav: &Path) -> Result<PlayHandle> {
    let wav = sh_quote(&wav.display().to_string());
    let script = if let Some(sink) = sink {
        let sink = sh_quote(sink);
        format!(
            "trap 'kill 0' EXIT; while true; do pw-play --target {sink} {wav} || paplay --device={sink} {wav} || paplay {wav}; done"
        )
    } else {
        format!("trap 'kill 0' EXIT; while true; do pw-play {wav} || paplay {wav}; done")
    };
    let mut cmd = Command::new("bash");
    cmd.args(["-c", &script]);
    PlayHandle::spawn(cmd)
}

fn wav_pcm(path: &Path) -> Result<(u32, u16, Vec<u8>)> {
    let mut f = std::fs::File::open(path)?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    parse_wav_pcm(&buf).ok_or_else(|| anyhow::anyhow!("not a PCM wav"))
}

fn parse_wav_pcm(buf: &[u8]) -> Option<(u32, u16, Vec<u8>)> {
    if buf.len() < 44 || &buf[0..4] != b"RIFF" || &buf[8..12] != b"WAVE" {
        return None;
    }
    let mut i = 12usize;
    let mut rate = 48_000u32;
    let mut channels = 2u16;
    while i + 8 <= buf.len() {
        let id = &buf[i..i + 4];
        let n = u32::from_le_bytes(buf[i + 4..i + 8].try_into().ok()?) as usize;
        let start = i + 8;
        let end = start.checked_add(n)?;
        if end > buf.len() {
            return None;
        }
        if id == b"fmt " && n >= 16 {
            let fmt = u16::from_le_bytes(buf[start..start + 2].try_into().ok()?);
            if fmt != 1 {
                return None;
            }
            channels = u16::from_le_bytes(buf[start + 2..start + 4].try_into().ok()?);
            rate = u32::from_le_bytes(buf[start + 4..start + 8].try_into().ok()?);
            let bits = u16::from_le_bytes(buf[start + 14..start + 16].try_into().ok()?);
            if bits != 16 {
                return None;
            }
        }
        if id == b"data" {
            return Some((rate, channels, buf[start..end].to_vec()));
        }
        i = end + (n % 2); // word align
    }
    None
}

pub fn inhibit_sleep() -> Result<PlayHandle> {
    let mut cmd = Command::new("systemd-inhibit");
    cmd.args([
        "--what=sleep:idle:handle-lid-switch",
        "--who=tws-tester",
        "--why=Bluetooth soak",
        "--mode=block",
        "sleep",
        "infinity",
    ]);
    PlayHandle::spawn(cmd)
}

fn icon_is_audio(icon: Option<&str>) -> bool {
    let Some(i) = icon else { return false };
    let i = i.to_ascii_lowercase();
    i.contains("audio") || i.contains("headset") || i.contains("headphone") || i.contains("speaker")
}

fn is_audio(uuids: &[String]) -> bool {
    uuids.iter().any(|u| {
        let u = u.to_ascii_lowercase();
        u.contains(AUDIO_SINK)
            || u.contains(HEADSET)
            || u.contains(HANDSFREE)
            || u.contains(SPEAKER)
    })
}

fn mac_underscores(addr: &str) -> String {
    addr.replace(':', "_")
}

fn str_prop(map: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    let v = map.get(key)?;
    if let Ok(s) = <&str>::try_from(v) {
        return Some(s.to_string());
    }
    String::try_from(v.clone()).ok()
}

fn bool_prop(map: &HashMap<String, OwnedValue>, key: &str) -> Option<bool> {
    let v = map.get(key)?;
    bool::try_from(v)
        .ok()
        .or_else(|| bool::try_from(v.clone()).ok())
}

fn u8_prop(map: &HashMap<String, OwnedValue>, key: &str) -> Option<u8> {
    let v = map.get(key)?;
    u8::try_from(v)
        .ok()
        .or_else(|| u8::try_from(v.clone()).ok())
}

fn i16_prop(map: &HashMap<String, OwnedValue>, key: &str) -> Option<i16> {
    let v = map.get(key)?;
    i16::try_from(v)
        .ok()
        .or_else(|| i16::try_from(v.clone()).ok())
        .or_else(|| {
            i32::try_from(v.clone())
                .ok()
                .and_then(|n| i16::try_from(n).ok())
        })
}

fn chip_label(modalias: &str) -> Option<String> {
    let ids = parse_modalias(modalias).map(|(v, p)| format!("{v:04X}:{p:04X}"));
    let vendor = hwdb_vendor(modalias);
    match (vendor, ids) {
        (Some(name), Some(ids)) => Some(format!("{name}  {ids}")),
        (Some(name), None) => Some(name),
        (None, Some(ids)) => Some(ids),
        (None, None) => None,
    }
}

fn hwdb_vendor(modalias: &str) -> Option<String> {
    let query = format!("{modalias}*");
    let out = Command::new("systemd-hwdb")
        .args(["query", &query])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let raw = text.lines().find_map(|l| {
        l.strip_prefix("ID_VENDOR_FROM_DATABASE=")
            .map(str::trim)
            .filter(|s| !s.is_empty())
    })?;
    Some(short_company(raw))
}

fn short_company(s: &str) -> String {
    for sep in [
        " technology",
        " Technology",
        " Co.",
        " co.",
        " Inc",
        " Limited",
        ",",
    ] {
        if let Some(i) = s.find(sep) {
            if i >= 4 {
                return s[..i].trim().to_string();
            }
        }
    }
    if s.len() > 32 {
        format!("{}…", s.chars().take(30).collect::<String>())
    } else {
        s.to_string()
    }
}

fn str_array(v: Option<&OwnedValue>) -> Vec<String> {
    let Some(v) = v else {
        return Vec::new();
    };
    <Vec<String>>::try_from(v.clone()).unwrap_or_default()
}

#[derive(Clone, Default)]
struct CardInfo {
    name: String,
    active: Option<String>,
    profiles: Vec<String>,
    labels: HashMap<String, String>,
}

#[derive(Default)]
struct PulseSnap {
    sinks: Vec<String>,
    volumes: HashMap<String, u8>,
    cards: Vec<CardInfo>,
}

async fn pulse_snapshot() -> PulseSnap {
    let mut snap = PulseSnap::default();
    if let Some(text) = pactl_text(&["list", "sinks"]).await {
        parse_sinks(&text, &mut snap);
    }
    if let Some(text) = pactl_text(&["list", "cards"]).await {
        snap.cards = parse_cards(&text);
    }
    snap
}

async fn pactl_text(args: &[&str]) -> Option<String> {
    let run = tokio::process::Command::new("pactl").args(args).output();
    match tokio::time::timeout(Duration::from_secs(2), run).await {
        Ok(Ok(out)) if !out.stdout.is_empty() => {
            Some(String::from_utf8_lossy(&out.stdout).into_owned())
        }
        _ => None,
    }
}

fn parse_sinks(text: &str, snap: &mut PulseSnap) {
    let mut name: Option<String> = None;
    for line in text.lines() {
        if line.starts_with("Sink #") {
            name = None;
            continue;
        }
        if let Some(rest) = line.trim().strip_prefix("Name:") {
            let n = rest.trim().to_string();
            if n.is_empty() {
                continue;
            }
            snap.sinks.push(n.clone());
            name = Some(n);
            continue;
        }
        if let Some(rest) = line.trim().strip_prefix("Volume:") {
            if let (Some(n), Some(p)) = (name.as_ref(), parse_volume_percent(rest)) {
                snap.volumes.insert(n.clone(), p);
            }
        }
    }
}

fn sh_quote(s: &str) -> String {
    let mut out = String::from("'");
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

fn parse_volume_percent(text: &str) -> Option<u8> {
    let mut best: Option<u8> = None;
    for part in text.split('/') {
        if let Some(p) = part.trim().strip_suffix('%') {
            if let Ok(n) = p.trim().parse::<u16>() {
                best = Some(n.min(150) as u8);
            }
        }
    }
    best
}

fn parse_cards(text: &str) -> Vec<CardInfo> {
    let mut cards = Vec::new();
    let mut cur: Option<CardInfo> = None;
    let mut in_profiles = false;
    for line in text.lines() {
        if line.starts_with("Card #") {
            if let Some(c) = cur.take() {
                cards.push(c);
            }
            cur = Some(CardInfo::default());
            in_profiles = false;
        }
        let Some(c) = cur.as_mut() else { continue };
        if let Some(rest) = line.trim().strip_prefix("Name:") {
            c.name = rest.trim().to_string();
        }
        if line.trim() == "Profiles:" {
            in_profiles = true;
            continue;
        }
        if line.trim().starts_with("Active Profile:") {
            in_profiles = false;
            c.active = line
                .split_once(':')
                .map(|(_, v)| v.trim().to_string())
                .filter(|s| !s.is_empty());
            continue;
        }
        if in_profiles {
            if let Some((name, rest)) = line.trim().split_once(':') {
                let name = name.trim();
                if name.starts_with("a2dp")
                    || name.starts_with("headset")
                    || name.starts_with("off")
                {
                    c.labels.insert(name.to_string(), codec_label(name, rest));
                    c.profiles.push(name.to_string());
                }
            }
        }
    }
    if let Some(c) = cur {
        cards.push(c);
    }
    cards
}

fn codec_label(profile: &str, rest: &str) -> String {
    if let Some(idx) = rest.find("codec ") {
        let tail = &rest[idx + 6..];
        let codec = tail.split([')', ',', '(']).next().unwrap_or("").trim();
        if !codec.is_empty() {
            return codec.to_string();
        }
    }
    profile.to_string()
}

pub fn probe_logs(addresses: &[String]) -> Vec<(String, String)> {
    use super::cmd_output;
    let mut logs = vec![
        ("uname".into(), cmd_output("uname", &["-a"])),
        (
            "os-release".into(),
            std::fs::read_to_string("/etc/os-release").unwrap_or_else(|e| format!("not read: {e}")),
        ),
        (
            "bluetoothctl-version".into(),
            cmd_output("bluetoothctl", &["--version"]),
        ),
        (
            "bluetoothctl-show".into(),
            cmd_output("bluetoothctl", &["--timeout", "5", "show"]),
        ),
        (
            "bluetoothctl-devices".into(),
            cmd_output("bluetoothctl", &["--timeout", "5", "devices"]),
        ),
        ("pactl-info".into(), cmd_output("pactl", &["info"])),
        (
            "pactl-sinks".into(),
            cmd_output("pactl", &["list", "short", "sinks"]),
        ),
        (
            "pactl-cards".into(),
            cmd_output("pactl", &["list", "short", "cards"]),
        ),
    ];
    for addr in addresses {
        logs.push((
            format!("bluetoothctl-info-{addr}"),
            cmd_output("bluetoothctl", &["--timeout", "8", "info", addr]),
        ));
    }
    logs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulse_volume_line_from_p30i() {
        let text = "Volume: front-left: 40766 /  62% / -12.37 dB,   front-right: 40766 /  62% / -12.37 dB\n        balance 0.00\n";
        assert_eq!(parse_volume_percent(text), Some(62));
    }

    #[test]
    fn list_sinks_names_and_volumes() {
        let text = "\
Sink #0
	State: SUSPENDED
	Name: alsa_output.pci-0000_00_1f.3.analog-stereo
	Mute: no
	Volume: front-left: 30000 /  46% / -20.25 dB,   front-right: 30000 /  46% / -20.25 dB
Sink #12
	State: RUNNING
	Name: bluez_output.18_9C_2C_34_0B_D4.1
	Mute: no
	Volume: front-left: 40766 /  62% / -12.37 dB,   front-right: 40766 /  62% / -12.37 dB
	Base Volume: 65536 / 100% / 0.00 dB
";
        let mut snap = PulseSnap::default();
        parse_sinks(text, &mut snap);
        assert_eq!(
            snap.sinks,
            [
                "alsa_output.pci-0000_00_1f.3.analog-stereo",
                "bluez_output.18_9C_2C_34_0B_D4.1"
            ]
        );
        assert_eq!(
            snap.volumes
                .get("bluez_output.18_9C_2C_34_0B_D4.1")
                .copied(),
            Some(62)
        );
        assert_eq!(
            snap.volumes
                .get("alsa_output.pci-0000_00_1f.3.analog-stereo")
                .copied(),
            Some(46)
        );
    }

    #[test]
    fn sh_quote_wraps_metacharacters() {
        assert_eq!(sh_quote("bluez_output.ok"), "'bluez_output.ok'");
        assert_eq!(sh_quote("a'b"), "'a'\\''b'");
        assert_eq!(sh_quote("x; rm -rf /"), "'x; rm -rf /'");
    }

    #[test]
    fn aac_profile_label() {
        let rest = " High Fidelity Playback (A2DP Sink, codec AAC) (sinks: 1, sources: 0, priority: 133, available: yes)";
        assert_eq!(codec_label("a2dp-sink", rest), "AAC");
    }

    #[test]
    fn reference_wav_pcm_roundtrip() {
        let dir = std::env::temp_dir().join("tws-tester-wav-pcm");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("r.wav");
        crate::reference::write_wav(&p).unwrap();
        let (rate, ch, pcm) = parse_wav_pcm(&std::fs::read(&p).unwrap()).expect("pcm");
        assert_eq!(rate, 48_000);
        assert_eq!(ch, 2);
        assert_eq!(pcm.len(), 48_000 * 2 * 2 * 2); // 2s stereo i16
    }
}

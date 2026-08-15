//! Device, named cells on it, soak kind, stimulus, and a Sample.

use std::collections::HashMap;

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceClass {
    Tws,
    Headphone,
    Speaker,
    Unknown,
}

impl DeviceClass {
    pub fn guess(name: &str, icon: Option<&str>) -> Self {
        let n = name.to_ascii_lowercase();
        let i = icon.unwrap_or("").to_ascii_lowercase();
        if n.contains("speaker") || i.contains("speaker") {
            Self::Speaker
        } else if n.contains("airpods max") {
            Self::Headphone
        } else if n.contains("nothing ear")
            || n.contains("cmf bud")
            || n.contains("enco")
            || n.contains("bud")
            || n.contains("airpod")
            || n.contains("powerbeats")
            || n.contains("beats fit")
            || n.contains("tws")
            || n.contains("eartip")
            || n.contains("in-ear")
            || n.contains("earbud")
            || i == "audio-headset"
        {
            Self::Tws
        } else if n.contains("headphone") || i.contains("headphone") || i.contains("audio") {
            Self::Headphone
        } else {
            Self::Unknown
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Tws => "tws",
            Self::Headphone => "headphones",
            Self::Speaker => "speaker",
            Self::Unknown => "audio",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p30i_is_tws() {
        assert_eq!(
            DeviceClass::guess("soundcore P30i", Some("audio-headset")),
            DeviceClass::Tws
        );
        assert_eq!(
            DeviceClass::guess("Galaxy Buds2 Pro", Some("audio-headset")),
            DeviceClass::Tws
        );
        assert_eq!(
            DeviceClass::guess("AirPods Pro - Find My", Some("audio-headphones")),
            DeviceClass::Tws
        );
        assert_eq!(
            DeviceClass::guess("AirPods Max", Some("audio-headphones")),
            DeviceClass::Headphone
        );
        assert_eq!(
            DeviceClass::guess("Beats Fit Pro", Some("audio-headphones")),
            DeviceClass::Tws
        );
        assert_eq!(
            DeviceClass::guess("Nothing Ear (2)", Some("audio-headset")),
            DeviceClass::Tws
        );
        assert_eq!(
            DeviceClass::guess("Powerbeats Pro", Some("audio-headphones")),
            DeviceClass::Tws
        );
    }

    #[test]
    fn modalias_p30i() {
        assert_eq!(
            parse_modalias("bluetooth:v05D6p000Ad0240"),
            Some((0x05D6, 0x000A))
        );
    }

    #[test]
    fn services_p30i_shape() {
        let u = [
            "0000110b-0000-1000-8000-00805f9b34fb",
            "0000110d-0000-1000-8000-00805f9b34fb",
            "0000110e-0000-1000-8000-00805f9b34fb",
            "0000111e-0000-1000-8000-00805f9b34fb",
            "00001101-0000-1000-8000-00805f9b34fb",
            "0cf12d31-fac3-4553-bd80-d6832e7b3959",
        ]
        .map(String::from);
        let s = service_labels(&u);
        assert!(s.contains(&"a2dp"));
        assert!(s.contains(&"hfp"));
        assert!(s.contains(&"spp"));
        assert!(s.contains(&"vendor"));
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellReading {
    pub name: String,
    pub percent: Option<u8>,
    pub source: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AncMode {
    Off,
    Anc,
    Transparency,
}

impl AncMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "normal",
            Self::Anc => "anc",
            Self::Transparency => "transparency",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AncKnowledge {
    Unknown,
    Known { mode: AncMode, can_set: bool },
}

#[derive(Clone, Debug)]
pub struct FoundDevice {
    pub address: String,
    pub name: String,
    pub connected: bool,
    pub paired: bool,
    pub class: DeviceClass,
    pub cells: Vec<CellReading>,
    pub codec: Option<String>,
    pub profiles: Vec<String>,
    pub profile_labels: HashMap<String, String>,
    pub host_volume: Option<u8>,
    pub headset_volume: Option<u8>,
    pub anc: AncKnowledge,
    pub sink: Option<String>,
    pub card: Option<String>,
    pub uuids: Vec<String>,
    pub brand: crate::brand::Brand,
    pub bonded: bool,
    pub address_type: Option<String>,
    pub rssi: Option<i16>,
    pub modalias: Option<String>,
    pub chip: Option<String>,
}

impl FoundDevice {
    pub fn headline_percent(&self) -> Option<u8> {
        self.cells.iter().filter_map(|c| c.percent).min()
    }

    pub fn pretty_codec(&self) -> String {
        match &self.codec {
            Some(k) => self.pretty_profile(k),
            None => "—".into(),
        }
    }

    pub fn pretty_profile(&self, key: &str) -> String {
        self.profile_labels
            .get(key)
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }

    pub fn services(&self) -> Vec<&'static str> {
        service_labels(&self.uuids)
    }

    pub fn codec_choices(&self) -> Vec<String> {
        self.profiles
            .iter()
            .map(|p| self.pretty_profile(p))
            .collect()
    }
}

pub fn parse_modalias(s: &str) -> Option<(u16, u16)> {
    let rest = s.strip_prefix("bluetooth:")?;
    let rest = rest.strip_prefix('v').or_else(|| rest.strip_prefix('V'))?;
    if rest.len() < 9 {
        return None;
    }
    let vid = u16::from_str_radix(&rest[..4], 16).ok()?;
    let rest = rest[4..]
        .strip_prefix('p')
        .or_else(|| rest[4..].strip_prefix('P'))?;
    if rest.len() < 4 {
        return None;
    }
    let pid = u16::from_str_radix(&rest[..4], 16).ok()?;
    Some((vid, pid))
}

pub fn service_labels(uuids: &[String]) -> Vec<&'static str> {
    let mut out = Vec::new();
    let mut vendor = false;
    for u in uuids {
        let u = u.to_ascii_lowercase();
        let tag = if u.contains("0000110b") || u.contains("0000110d") {
            Some("a2dp")
        } else if u.contains("0000110e") || u.contains("0000110c") {
            Some("avrcp")
        } else if u.contains("0000111e") || u.contains("00001108") {
            Some("hfp")
        } else if u.contains("00001101") {
            Some("spp")
        } else if u.contains("00001200") {
            Some("pnp")
        } else if u.contains("0000180f") {
            Some("gatt-battery")
        } else if u.contains("00001844") || u.contains("0000184e") || u.contains("00001850") {
            Some("le-audio")
        } else if u.contains("0000110a") {
            Some("a2dp-src")
        } else if !u.starts_with("0000") {
            vendor = true;
            None
        } else {
            None
        };
        if let Some(t) = tag {
            if !out.contains(&t) {
                out.push(t);
            }
        }
    }
    if vendor {
        out.push("vendor");
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SoakKind {
    FullLife,
    Remaining,
}

impl SoakKind {
    pub fn from_start_percent(p: Option<u8>) -> Self {
        match p {
            Some(n) if n >= 95 => Self::FullLife,
            _ => Self::Remaining,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::FullLife => "full_life",
            Self::Remaining => "remaining",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::FullLife => "full life",
            Self::Remaining => "remaining",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stimulus {
    Reference,
    Playlist,
}

impl Stimulus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::Playlist => "playlist",
        }
    }

    pub fn default_for(class: DeviceClass) -> Self {
        match class {
            DeviceClass::Speaker => Self::Playlist,
            _ => Self::Reference,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sample {
    pub t: DateTime<Local>,
    pub elapsed_ms: u64,
    pub cells: Vec<CellReading>,
    pub codec: Option<String>,
    pub host_volume: Option<u8>,
    pub present: bool,
}

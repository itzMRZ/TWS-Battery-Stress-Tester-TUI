//! One-cell glyphs for class, cells, link, and battery.

use crate::device::DeviceClass;

pub fn class(c: DeviceClass) -> &'static str {
    match c {
        DeviceClass::Tws => "◎",
        DeviceClass::Headphone => "⊂",
        DeviceClass::Speaker => "▢",
        DeviceClass::Unknown => "·",
    }
}

pub fn cell(name: &str) -> &'static str {
    match name {
        "left" => "◐",
        "right" => "◑",
        "case" => "▢",
        "pair" | "pack" => "●",
        _ => "▮",
    }
}

pub fn link(connected: bool) -> &'static str {
    if connected {
        "●"
    } else {
        "○"
    }
}

pub fn battery(percent: Option<u8>) -> &'static str {
    match percent {
        None => "·",
        Some(0) => "!",
        Some(n) if n < 15 => "▁",
        Some(n) if n < 30 => "▂",
        Some(n) if n < 45 => "▃",
        Some(n) if n < 60 => "▄",
        Some(n) if n < 75 => "▅",
        Some(n) if n < 90 => "▆",
        _ => "▇",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battery_empty_is_bang() {
        assert_eq!(battery(Some(0)), "!");
        assert_eq!(battery(Some(100)), "▇");
        assert_eq!(battery(None), "·");
    }
}

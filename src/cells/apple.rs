//! Apple accessory protocol battery frames on L2CAP. AirPods and Beats.
//! Disconnected components stay unknown.

use crate::device::CellReading;

use super::cell;

const SRC: &str = "apple.aap";
const BATTERY: [u8; 6] = [0x04, 0x00, 0x04, 0x00, 0x04, 0x00];
const LEFT: u8 = 0x04;
const RIGHT: u8 = 0x02;
const CASE: u8 = 0x08;
const DISCONNECTED: u8 = 0x04;

/// Session open. Without this the buds ignore later frames.
pub fn handshake() -> &'static [u8] {
    &[
        0x00, 0x00, 0x04, 0x00, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ]
}

/// Subscribe to battery (and other) pushes. Send both; some firmware answers only one.
pub fn request_notifications() -> &'static [u8] {
    &[0x04, 0x00, 0x04, 0x00, 0x0f, 0x00, 0xff, 0xff, 0xfe, 0xff]
}

pub fn request_notifications_all() -> &'static [u8] {
    &[0x04, 0x00, 0x04, 0x00, 0x0f, 0x00, 0xff, 0xff, 0xff, 0xff]
}

pub fn cells_from_stream(bytes: &[u8]) -> Vec<CellReading> {
    let Some((_, left, right, case)) = take_battery(bytes) else {
        return Vec::new();
    };
    let mut cells = Vec::new();
    if left.is_some() {
        cells.push(cell("left", left, SRC));
    }
    if right.is_some() {
        cells.push(cell("right", right, SRC));
    }
    if case.is_some() {
        cells.push(cell("case", case, SRC));
    }
    cells
}

fn take_battery(input: &[u8]) -> Option<(usize, Option<u8>, Option<u8>, Option<u8>)> {
    if input.len() < 7 || !input.starts_with(&BATTERY) {
        return None;
    }
    let n = input[6] as usize;
    if n == 0 || n > 8 {
        return None;
    }
    let total = 7 + n * 5;
    if input.len() < total {
        return None;
    }
    let mut left = None;
    let mut right = None;
    let mut case = None;
    for i in 0..n {
        let rec = &input[7 + i * 5..7 + (i + 1) * 5];
        let (kind, spacer, level, status, end) = (rec[0], rec[1], rec[2], rec[3], rec[4]);
        if spacer != 0x01 || end != 0x01 || level > 100 || status == DISCONNECTED {
            continue;
        }
        match kind {
            LEFT if left.is_none() => left = Some(level),
            RIGHT if right.is_none() => right = Some(level),
            CASE if case.is_none() => case = Some(level),
            _ => {}
        }
    }
    Some((total, left, right, case))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documented_aap_battery_frame() {
        let frame = [
            0x04, 0x00, 0x04, 0x00, 0x04, 0x00, 0x03, 0x02, 0x01, 0x64, 0x02, 0x01, 0x04, 0x01,
            0x63, 0x01, 0x01, 0x08, 0x01, 0x11, 0x02, 0x01,
        ];
        let cells = cells_from_stream(&frame);
        assert_eq!(named(&cells, "right"), Some(100));
        assert_eq!(named(&cells, "left"), Some(99));
        assert_eq!(named(&cells, "case"), Some(17));
    }

    #[test]
    fn disconnected_case_is_omitted() {
        let frame = [
            0x04, 0x00, 0x04, 0x00, 0x04, 0x00, 0x03, 0x02, 0x01, 0x5d, 0x02, 0x01, 0x04, 0x01,
            0x5e, 0x02, 0x01, 0x08, 0x01, 0x00, 0x04, 0x01,
        ];
        let cells = cells_from_stream(&frame);
        assert_eq!(named(&cells, "right"), Some(93));
        assert_eq!(named(&cells, "left"), Some(94));
        assert!(cells.iter().all(|c| c.name != "case"));
    }

    #[test]
    fn handshake_and_notify_are_fixed() {
        assert_eq!(handshake()[0], 0x00);
        assert_eq!(handshake().len(), 16);
        assert_eq!(request_notifications()[4], 0x0f);
    }

    fn named(cells: &[CellReading], name: &str) -> Option<u8> {
        cells
            .iter()
            .find(|c| c.name == name)
            .and_then(|c| c.percent)
    }
}

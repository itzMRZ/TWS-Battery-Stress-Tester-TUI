//! Nothing / CMF family RFCOMM. One 0x55 framing across Ear and CMF Buds.
//!
//! GET battery is 0xC007. Payload is a count then type/value pairs
//! (2=left, 3=right, 4=case). Value bit 7 is charging; bits 6:0 are percent.

use crate::device::CellReading;

use super::cell;

const SRC: &str = "nothing.rfcomm";
const SOF: u8 = 0x55;
const CTRL_HOST: u16 = 0x0160;
const CMD_PROTO: u16 = 0xc001;
const CMD_ACTIVATED: u16 = 0xf001;
const CMD_BATTERY: u16 = 0xc007;
const EVT_BATTERY: u16 = 0xe001;
const BAT_LEFT: u8 = 2;
const BAT_RIGHT: u8 = 3;
const BAT_CASE: u8 = 4;

pub fn handshake() -> Vec<u8> {
    encode(CMD_PROTO, 1, &[])
}

pub fn activate() -> Vec<u8> {
    encode(CMD_ACTIVATED, 1, &[])
}

pub fn request_battery() -> Vec<u8> {
    encode(CMD_BATTERY, 1, &[])
}

pub fn cells_from_stream(bytes: &[u8]) -> Vec<CellReading> {
    let mut left = None;
    let mut right = None;
    let mut case = None;
    for frame in take_frames(bytes) {
        let cmd = frame.cmd | 0x8000;
        if cmd != CMD_BATTERY && frame.cmd != EVT_BATTERY && cmd != EVT_BATTERY {
            continue;
        }
        apply_battery(&frame.payload, &mut left, &mut right, &mut case);
    }
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

struct Frame {
    cmd: u16,
    payload: Vec<u8>,
}

fn apply_battery(
    payload: &[u8],
    left: &mut Option<u8>,
    right: &mut Option<u8>,
    case: &mut Option<u8>,
) {
    if payload.len() < 3 {
        return;
    }
    let count = payload[0] as usize;
    if count == 0 || count > 8 || payload.len() < 1 + count * 2 {
        return;
    }
    for i in 0..count {
        let t = payload[1 + i * 2];
        let v = payload[2 + i * 2] & 0x7f;
        let Some(p) = gage(v) else {
            continue;
        };
        match t {
            BAT_LEFT if left.is_none() => *left = Some(p),
            BAT_RIGHT if right.is_none() => *right = Some(p),
            BAT_CASE if case.is_none() => *case = Some(p),
            _ => {}
        }
    }
}

fn gage(n: u8) -> Option<u8> {
    if n > 100 {
        None
    } else {
        Some(n)
    }
}

fn encode(cmd: u16, fsn: u8, payload: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(10 + payload.len());
    b.push(SOF);
    b.extend_from_slice(&CTRL_HOST.to_le_bytes());
    b.extend_from_slice(&cmd.to_le_bytes());
    b.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    b.push(fsn);
    b.extend_from_slice(payload);
    let crc = crc16_arc(&b);
    b.extend_from_slice(&crc.to_le_bytes());
    b
}

fn crc16_arc(data: &[u8]) -> u16 {
    let mut crc = 0xffffu16;
    for &b in data {
        crc ^= u16::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xa001
            } else {
                crc >> 1
            };
        }
    }
    crc
}

fn take_frames(mut input: &[u8]) -> Vec<Frame> {
    let mut out = Vec::new();
    while !input.is_empty() {
        match take_one(input) {
            Some((rest, frame)) => {
                out.push(frame);
                input = rest;
            }
            None => {
                input = &input[1..];
            }
        }
    }
    out
}

fn take_one(input: &[u8]) -> Option<(&[u8], Frame)> {
    if input.first() != Some(&SOF) || input.len() < 8 {
        return None;
    }
    let ctrl = u16::from_le_bytes([input[1], input[2]]);
    let cmd = u16::from_le_bytes([input[3], input[4]]);
    let len = u16::from_le_bytes([input[5], input[6]]) as usize;
    let crc_len = if ctrl & 0x20 != 0 { 2 } else { 0 };
    let total = 8 + len + crc_len;
    if input.len() < total {
        return None;
    }
    if crc_len == 2 {
        let got = u16::from_le_bytes([input[8 + len], input[9 + len]]);
        if crc16_arc(&input[..8 + len]) != got {
            return None;
        }
    }
    Some((
        &input[total..],
        Frame {
            cmd,
            payload: input[8..8 + len].to_vec(),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_anc_frame_crc() {
        // Ear (2) transparency SET captured on RFCOMM 15.
        let frame = [
            0x55, 0x60, 0x01, 0x0f, 0xf0, 0x03, 0x00, 0xcb, 0x01, 0x07, 0x00, 0xc5, 0xaf,
        ];
        assert_eq!(crc16_arc(&frame[..11]), 0xafc5);
        let parsed = take_frames(&frame);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].cmd, 0xf00f);
        assert_eq!(parsed[0].payload, [0x01, 0x07, 0x00]);
    }

    #[test]
    fn battery_get_is_c007() {
        let f = take_frames(&request_battery());
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].cmd, CMD_BATTERY);
        assert!(f[0].payload.is_empty());
    }

    #[test]
    fn counted_pairs_are_left_right_case() {
        // count=3, left 80, right 75, case 30 (not charging).
        let payload = [3, BAT_LEFT, 80, BAT_RIGHT, 75, BAT_CASE, 30];
        let frame = encode(CMD_BATTERY & 0x7fff, 1, &payload);
        let cells = cells_from_stream(&frame);
        assert_eq!(named(&cells, "left"), Some(80));
        assert_eq!(named(&cells, "right"), Some(75));
        assert_eq!(named(&cells, "case"), Some(30));
    }

    #[test]
    fn charging_bit_is_stripped() {
        let payload = [1, BAT_LEFT, 80 | 0x80];
        let frame = encode(EVT_BATTERY, 2, &payload);
        assert_eq!(named(&cells_from_stream(&frame), "left"), Some(80));
    }

    #[test]
    fn unknown_percent_is_omitted() {
        let payload = [1, BAT_LEFT, 0x7f];
        let frame = encode(CMD_BATTERY, 1, &payload);
        assert!(cells_from_stream(&frame).is_empty());
    }

    #[test]
    fn garbage_is_not_cells() {
        assert!(cells_from_stream(&[1, 2, 3, 4, 5, 6, 7, 8]).is_empty());
    }

    fn named(cells: &[CellReading], name: &str) -> Option<u8> {
        cells
            .iter()
            .find(|c| c.name == name)
            .and_then(|c| c.percent)
    }
}

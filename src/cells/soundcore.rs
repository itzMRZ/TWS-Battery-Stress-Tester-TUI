//! Soundcore / Anker family RFCOMM. Same framing across many models.

use crate::device::CellReading;

use super::{cell, scale_levels};

const OUT: [u8; 5] = [0x08, 0xee, 0x00, 0x00, 0x00];
const IN: [u8; 5] = [0x09, 0xff, 0x00, 0x00, 0x01];
const SRC: &str = "soundcore.rfcomm";

pub fn request_battery_level() -> Vec<u8> {
    encode([0x01, 0x03], &[])
}

pub fn request_state() -> Vec<u8> {
    encode([0x01, 0x01], &[])
}

fn encode(cmd: [u8; 2], body: &[u8]) -> Vec<u8> {
    let len = (OUT.len() + cmd.len() + 2 + body.len() + 1) as u16;
    let mut b = Vec::with_capacity(len as usize);
    b.extend_from_slice(&OUT);
    b.extend_from_slice(&cmd);
    b.extend_from_slice(&len.to_le_bytes());
    b.extend_from_slice(body);
    let cs = checksum(&b);
    b.push(cs);
    b
}

fn checksum(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |a, b| a.wrapping_add(*b))
}

#[derive(Debug, Clone)]
struct Packet {
    cmd: [u8; 2],
    body: Vec<u8>,
}

fn take_packets(mut input: &[u8]) -> Vec<Packet> {
    let mut out = Vec::new();
    while !input.is_empty() {
        match take_one(input) {
            Some((rest, pkt)) => {
                out.push(pkt);
                input = rest;
            }
            None => {
                input = &input[1..];
            }
        }
    }
    out
}

fn take_one(input: &[u8]) -> Option<(&[u8], Packet)> {
    if input.len() < 10 {
        return None;
    }
    if !(input.starts_with(&IN) || input.starts_with(&OUT)) {
        return None;
    }
    let len = u16::from_le_bytes([input[7], input[8]]) as usize;
    if len < 10 || input.len() < len {
        return None;
    }
    let pkt = &input[..len];
    if checksum(&pkt[..len - 1]) != pkt[len - 1] {
        return None;
    }
    Some((
        &input[len..],
        Packet {
            cmd: [pkt[5], pkt[6]],
            body: pkt[9..len - 1].to_vec(),
        },
    ))
}

pub fn cells_from_stream(bytes: &[u8]) -> Vec<CellReading> {
    let mut left = None;
    let mut right = None;
    let mut case = None;
    let mut pack = None;
    for pkt in take_packets(bytes) {
        match pkt.cmd {
            [0x01, 0x03] => apply_battery_level(&pkt.body, &mut left, &mut right, &mut pack),
            [0x01, 0x01] => apply_state(&pkt.body, &mut left, &mut right, &mut case, &mut pack),
            _ => {}
        }
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
    if pack.is_some() && left.is_none() && right.is_none() {
        cells.push(cell("pack", pack, SRC));
    }
    cells
}

fn apply_battery_level(
    body: &[u8],
    left: &mut Option<u8>,
    right: &mut Option<u8>,
    pack: &mut Option<u8>,
) {
    if body.is_empty() {
        return;
    }
    let scaled = scale_levels(body);
    if scaled.len() == 1 {
        if pack.is_none() {
            *pack = scaled[0];
        }
        return;
    }
    if let Some(p) = scaled.first().copied().flatten() {
        *left = Some(p);
    }
    if let Some(p) = scaled.get(1).copied().flatten() {
        *right = Some(p);
    }
}

fn apply_state(
    body: &[u8],
    left: &mut Option<u8>,
    right: &mut Option<u8>,
    case: &mut Option<u8>,
    pack: &mut Option<u8>,
) {
    if let Some(found) = tlv_batteries(body) {
        if left.is_none() {
            *left = found.0;
        }
        if right.is_none() {
            *right = found.1;
        }
        if case.is_none() {
            *case = found.2;
        }
        return;
    }
    // Common TWS prefix: tws (2) + DualBattery (left, right, chg, chg).
    if body.len() >= 6 {
        let scaled = scale_levels(&body[2..4]);
        if left.is_none() {
            *left = scaled.first().copied().flatten();
        }
        if right.is_none() {
            *right = scaled.get(1).copied().flatten();
        }
        return;
    }
    if body.len() >= 2 && left.is_none() && right.is_none() {
        let scaled = scale_levels(&body[..1]);
        if pack.is_none() {
            *pack = scaled.first().copied().flatten();
        }
    }
}

/// Newer Soundcore state bodies: tag, length, value. Tags 3/4/8 are L/R/case.
fn tlv_batteries(body: &[u8]) -> Option<(Option<u8>, Option<u8>, Option<u8>)> {
    let mut i = 0;
    let mut left = None;
    let mut right = None;
    let mut case = None;
    let mut tags = 0u8;
    while i + 2 <= body.len() {
        let tag = body[i];
        let len = body[i + 1] as usize;
        if !(1..=32).contains(&tag) || len == 0 || len > 32 {
            return None;
        }
        i += 2;
        if i + len > body.len() {
            return None;
        }
        let v = &body[i..i + len];
        i += len;
        tags += 1;
        match (tag, len) {
            (3, 2) => left = tlv_level(v),
            (4, 2) => right = tlv_level(v),
            (8, 2) => case = tlv_level(v),
            _ => {}
        }
    }
    if tags < 3 || (left.is_none() && right.is_none()) {
        return None;
    }
    Some((left, right, case))
}

fn tlv_level(v: &[u8]) -> Option<u8> {
    // Documented as [charging_flag, level%].
    let n = v[1];
    if n == 255 || n > 100 {
        None
    } else {
        Some(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_state_matches_published_fixture() {
        assert_eq!(
            request_state(),
            vec![0x08, 0xee, 0x00, 0x00, 0x00, 0x01, 0x01, 0x0a, 0x00, 0x02]
        );
    }

    #[test]
    fn request_battery_matches_published_fixture() {
        assert_eq!(
            request_battery_level(),
            vec![0x08, 0xee, 0x00, 0x00, 0x00, 0x01, 0x03, 0x0a, 0x00, 0x04]
        );
    }

    #[test]
    fn dual_level_fixture_is_tenths() {
        // OpenSCQ30 DualBatteryLevel fixture: left 3, right 4 (of 10).
        let input = [
            0x09, 0xff, 0x00, 0x00, 0x01, 0x01, 0x03, 0x0c, 0x00, 0x03, 0x04, 0x20,
        ];
        let cells = cells_from_stream(&input);
        assert_eq!(named(&cells, "left"), Some(30));
        assert_eq!(named(&cells, "right"), Some(40));
    }

    #[test]
    fn live_p30i_battery_command() {
        let input = [
            0x09, 0xff, 0x00, 0x00, 0x01, 0x01, 0x03, 0x0d, 0x00, 0x05, 0x04, 0xff, 0x22,
        ];
        let cells = cells_from_stream(&input);
        assert_eq!(named(&cells, "left"), Some(50));
        assert_eq!(named(&cells, "right"), Some(40));
        assert!(cells.iter().all(|c| c.name != "case"));
    }

    #[test]
    fn live_p30i_state_has_same_split() {
        let hex = "09ff0000010101650001010504ffff30312e363530312e363533393539443430423334324339433138fefe8c9e8b6c63738a7b7878000000000000000000000affff6366ffff4444330055000001ff003101010001020100000000000000000000000000f0";
        let bytes = hex_bytes(hex);
        let cells = cells_from_stream(&bytes);
        assert_eq!(named(&cells, "left"), Some(50));
        assert_eq!(named(&cells, "right"), Some(40));
    }

    #[test]
    fn q30_single_is_pack_not_left() {
        let input = [
            0x09, 0xff, 0x00, 0x00, 0x01, 0x01, 0x03, 0x0b, 0x00, 0x02, 0x1a,
        ];
        let cells = cells_from_stream(&input);
        assert_eq!(named(&cells, "pack"), Some(20));
        assert!(cells.iter().all(|c| c.name != "left"));
    }

    #[test]
    fn garbage_is_not_cells() {
        assert!(cells_from_stream(&[1, 2, 3, 4, 5, 6, 7, 8]).is_empty());
    }

    #[test]
    fn tlv_left_right_case() {
        // Minimal inbound wrapper around a TLV body with tags 1,3,4,8.
        let body = [
            1, 1, 0, // host
            3, 2, 0, 80, // left 80%
            4, 2, 0, 70, // right 70%
            8, 2, 0, 40, // case 40%
        ];
        let pkt = encode_in([0x01, 0x01], &body);
        let cells = cells_from_stream(&pkt);
        assert_eq!(named(&cells, "left"), Some(80));
        assert_eq!(named(&cells, "right"), Some(70));
        assert_eq!(named(&cells, "case"), Some(40));
    }

    fn encode_in(cmd: [u8; 2], body: &[u8]) -> Vec<u8> {
        let len = (IN.len() + cmd.len() + 2 + body.len() + 1) as u16;
        let mut b = Vec::new();
        b.extend_from_slice(&IN);
        b.extend_from_slice(&cmd);
        b.extend_from_slice(&len.to_le_bytes());
        b.extend_from_slice(body);
        let cs = checksum(&b);
        b.push(cs);
        b
    }

    fn named(cells: &[CellReading], name: &str) -> Option<u8> {
        cells
            .iter()
            .find(|c| c.name == name)
            .and_then(|c| c.percent)
    }

    fn hex_bytes(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
}

//! Sony Headphones Connect family (MDR over Tandem RFCOMM).
//!
//! Same framing across WH/WF/LinkBuds generations. v1 and v2 share the
//! Tandem wrapper; battery opcodes differ, so 0x22 is only sent after a v2
//! handshake (on v1 that byte is power-off).

use crate::device::CellReading;

use super::cell;

const SRC: &str = "sony.rfcomm";
const START: u8 = 0x3e;
const END: u8 = 0x3c;
const ESC: u8 = 0x3d;
const DATA_MDR: u8 = 0x0c;
const ACK: u8 = 0x01;

const GET_PROTOCOL: u8 = 0x00;
const RET_PROTOCOL: u8 = 0x01;
const GET_BATTERY: u8 = 0x10;
const RET_BATTERY: u8 = 0x11;
const NTFY_BATTERY: u8 = 0x13;
const V2_GET_BATTERY: u8 = 0x22;
const V2_RET_BATTERY: u8 = 0x23;

const TYPE_PACK: u8 = 0x00;
const TYPE_LEFT_RIGHT: u8 = 0x02;
const TYPE_CASE: u8 = 0x03;

struct Frame {
    data_type: u8,
    seq: u8,
    payload: Vec<u8>,
}

pub fn handshake() -> Vec<u8> {
    encode(DATA_MDR, 0, &[GET_PROTOCOL, 0x00])
}

/// Battery GETs after the handshake reply. v1 never sees 0x22.
pub fn battery_requests(seen: &[u8]) -> Vec<Vec<u8>> {
    if is_v2(seen) {
        vec![encode(DATA_MDR, 0, &[V2_GET_BATTERY, TYPE_PACK])]
    } else {
        [TYPE_PACK, TYPE_LEFT_RIGHT, TYPE_CASE]
            .into_iter()
            .map(|t| encode(DATA_MDR, 0, &[GET_BATTERY, t]))
            .collect()
    }
}

pub fn acks(bytes: &[u8]) -> Vec<Vec<u8>> {
    take_frames(bytes)
        .into_iter()
        .filter(|f| f.data_type == DATA_MDR)
        .map(|f| encode(ACK, 1 - (f.seq & 1), &[]))
        .collect()
}

pub fn cells_from_stream(bytes: &[u8]) -> Vec<CellReading> {
    let mut left = None;
    let mut right = None;
    let mut case = None;
    let mut pack = None;
    for frame in take_frames(bytes) {
        if frame.data_type != DATA_MDR || frame.payload.is_empty() {
            continue;
        }
        apply_payload(&frame.payload, &mut left, &mut right, &mut case, &mut pack);
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

fn is_v2(bytes: &[u8]) -> bool {
    take_frames(bytes).into_iter().any(|f| {
        f.data_type == DATA_MDR
            && f.payload.len() >= 3
            && f.payload[0] == RET_PROTOCOL
            && f.payload[2] == 0x03
    })
}

fn apply_payload(
    payload: &[u8],
    left: &mut Option<u8>,
    right: &mut Option<u8>,
    case: &mut Option<u8>,
    pack: &mut Option<u8>,
) {
    match payload[0] {
        RET_BATTERY | NTFY_BATTERY => apply_v1(&payload[1..], left, right, case, pack),
        V2_RET_BATTERY => {
            if payload.len() >= 3 {
                if let Some(p) = gage(payload[2]) {
                    if pack.is_none() {
                        *pack = Some(p);
                    }
                }
            }
        }
        _ => {}
    }
}

fn apply_v1(
    rest: &[u8],
    left: &mut Option<u8>,
    right: &mut Option<u8>,
    case: &mut Option<u8>,
    pack: &mut Option<u8>,
) {
    let Some(&kind) = rest.first() else {
        return;
    };
    match kind {
        TYPE_PACK => {
            if let Some(p) = rest.get(1).copied().and_then(gage) {
                if pack.is_none() {
                    *pack = Some(p);
                }
            }
        }
        TYPE_LEFT_RIGHT => {
            if let Some(p) = rest.get(1).copied().and_then(gage) {
                if left.is_none() {
                    *left = Some(p);
                }
            }
            if let Some(p) = rest.get(3).copied().and_then(gage) {
                if right.is_none() {
                    *right = Some(p);
                }
            }
        }
        TYPE_CASE => {
            if let Some(p) = rest.get(1).copied().and_then(gage) {
                if case.is_none() {
                    *case = Some(p);
                }
            }
        }
        _ => {}
    }
}

fn gage(n: u8) -> Option<u8> {
    if n > 100 {
        None
    } else {
        Some(n)
    }
}

fn encode(data_type: u8, seq: u8, payload: &[u8]) -> Vec<u8> {
    let n = payload.len() as u32;
    let mut inner = Vec::with_capacity(7 + payload.len());
    inner.push(data_type);
    inner.push(seq);
    inner.extend_from_slice(&n.to_be_bytes());
    inner.extend_from_slice(payload);
    let sum = checksum(&inner);
    inner.push(sum);
    let mut out = vec![START];
    out.extend(escape(&inner));
    out.push(END);
    out
}

fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |a, b| a.wrapping_add(*b))
}

fn escape(src: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(src.len());
    for &b in src {
        if matches!(b, START | END | ESC) {
            out.push(ESC);
            out.push(b & 0xef);
        } else {
            out.push(b);
        }
    }
    out
}

fn unescape(src: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(src.len());
    let mut i = 0;
    while i < src.len() {
        if src[i] == ESC {
            i += 1;
            let b = *src.get(i)?;
            out.push(b | 0x10);
        } else {
            out.push(src[i]);
        }
        i += 1;
    }
    Some(out)
}

fn take_frames(mut input: &[u8]) -> Vec<Frame> {
    let mut out = Vec::new();
    while let Some(rest) = skip_to_start(input) {
        input = rest;
        match take_one(input) {
            Some((next, frame)) => {
                out.push(frame);
                input = next;
            }
            None => break,
        }
    }
    out
}

fn skip_to_start(input: &[u8]) -> Option<&[u8]> {
    let i = input.iter().position(|&b| b == START)?;
    Some(&input[i..])
}

fn take_one(input: &[u8]) -> Option<(&[u8], Frame)> {
    if input.first() != Some(&START) {
        return None;
    }
    let end = input.iter().skip(1).position(|&b| b == END)? + 1;
    let escaped = &input[1..end];
    let rest = &input[end + 1..];
    let inner = unescape(escaped)?;
    if inner.len() < 7 {
        return Some((
            rest,
            Frame {
                data_type: 0xff,
                seq: 0,
                payload: Vec::new(),
            },
        ));
    }
    let data_type = inner[0];
    let seq = inner[1];
    let n = u32::from_be_bytes(inner[2..6].try_into().ok()?) as usize;
    if inner.len() != 6 + n + 1 {
        return Some((
            rest,
            Frame {
                data_type: 0xff,
                seq,
                payload: Vec::new(),
            },
        ));
    }
    if checksum(&inner[..inner.len() - 1]) != inner[inner.len() - 1] {
        return Some((
            rest,
            Frame {
                data_type: 0xff,
                seq,
                payload: Vec::new(),
            },
        ));
    }
    Some((
        rest,
        Frame {
            data_type,
            seq,
            payload: inner[6..6 + n].to_vec(),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tandem_escape_table() {
        // Documented unescaped → escaped pairs.
        assert_eq!(
            escape(&[0x3c, 0x3d, 0x3e]),
            [0x3d, 0x2c, 0x3d, 0x2d, 0x3d, 0x2e]
        );
        assert_eq!(
            unescape(&escape(&[0x10, 0x3e, 0x20])).unwrap(),
            [0x10, 0x3e, 0x20]
        );
    }

    #[test]
    fn handshake_is_protocol_info_get() {
        let f = take_frames(&handshake());
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].data_type, DATA_MDR);
        assert_eq!(f[0].payload, [GET_PROTOCOL, 0x00]);
    }

    #[test]
    fn pack_battery_is_pack_not_left() {
        // RET 0x11, type 0x00, 80%, not charging. mdr-protocol BATTERY payload.
        let frame = encode(DATA_MDR, 0, &[RET_BATTERY, TYPE_PACK, 80, 0]);
        let cells = cells_from_stream(&frame);
        assert_eq!(named(&cells, "pack"), Some(80));
        assert!(cells.iter().all(|c| c.name != "left"));
    }

    #[test]
    fn left_right_and_case() {
        let lr = encode(DATA_MDR, 0, &[RET_BATTERY, TYPE_LEFT_RIGHT, 67, 0, 76, 0]);
        let cr = encode(DATA_MDR, 0, &[NTFY_BATTERY, TYPE_CASE, 40, 0]);
        let mut bytes = lr;
        bytes.extend(cr);
        let cells = cells_from_stream(&bytes);
        assert_eq!(named(&cells, "left"), Some(67));
        assert_eq!(named(&cells, "right"), Some(76));
        assert_eq!(named(&cells, "case"), Some(40));
    }

    #[test]
    fn v2_ret_is_pack() {
        let frame = encode(DATA_MDR, 0, &[V2_RET_BATTERY, TYPE_PACK, 60, 0]);
        assert_eq!(named(&cells_from_stream(&frame), "pack"), Some(60));
    }

    #[test]
    fn v2_handshake_does_not_send_v1_power_off_opcode() {
        let hello = encode(DATA_MDR, 1, &[RET_PROTOCOL, 0x00, 0x03, 0x00]);
        let reqs = battery_requests(&hello);
        assert_eq!(reqs.len(), 1);
        let inner = take_frames(&reqs[0]);
        assert_eq!(inner[0].payload, [V2_GET_BATTERY, TYPE_PACK]);
    }

    #[test]
    fn v1_battery_gets_are_inquired_types() {
        let reqs = battery_requests(&[]);
        let kinds: Vec<u8> = reqs
            .iter()
            .flat_map(|r| take_frames(r))
            .map(|f| f.payload[1])
            .collect();
        assert_eq!(kinds, [TYPE_PACK, TYPE_LEFT_RIGHT, TYPE_CASE]);
        assert!(reqs
            .iter()
            .all(|r| take_frames(r)[0].payload[0] == GET_BATTERY));
    }

    #[test]
    fn ack_flips_seq() {
        let data = encode(DATA_MDR, 0, &[RET_BATTERY, TYPE_PACK, 10, 0]);
        let ack = acks(&data);
        assert_eq!(ack.len(), 1);
        let f = &take_frames(&ack[0])[0];
        assert_eq!(f.data_type, ACK);
        assert_eq!(f.seq, 1);
        assert!(f.payload.is_empty());
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

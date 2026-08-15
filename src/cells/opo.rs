//! Oppo / OnePlus / Realme (OPO / HeyMelody) RFCOMM.
//!
//! One framing across the three Families. Battery query is 0x0105 or 0x0106;
//! replies are packed `[id, val]` tuples or ASCII CSV. Id 1/2/3 = left/right/case.

use crate::device::CellReading;

use super::cell;

const SRC: &str = "opo.rfcomm";
const SOF: u8 = 0xaa;
const GET_BATTERY: &[u16] = &[0x0105, 0x0106];
const RET_BATTERY: &[u16] = &[0x8105, 0x8106, 0x0204, 0x0505];

pub fn battery_requests() -> Vec<Vec<u8>> {
    GET_BATTERY
        .iter()
        .copied()
        .map(|cmd| encode(cmd, 1, &[]))
        .collect()
}

pub fn cells_from_stream(bytes: &[u8]) -> Vec<CellReading> {
    let mut left = None;
    let mut right = None;
    let mut case = None;
    for frame in take_frames(bytes) {
        if !RET_BATTERY.contains(&frame.cmd) {
            continue;
        }
        apply(&frame.payload, &mut left, &mut right, &mut case);
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

fn apply(payload: &[u8], left: &mut Option<u8>, right: &mut Option<u8>, case: &mut Option<u8>) {
    if parse_csv(payload, left, right, case) {
        return;
    }
    parse_packed(payload, left, right, case);
}

fn parse_csv(
    payload: &[u8],
    left: &mut Option<u8>,
    right: &mut Option<u8>,
    case: &mut Option<u8>,
) -> bool {
    let body = if payload.len() > 4 && payload[3] == 3 && payload[4..].contains(&b',') {
        &payload[4..]
    } else {
        payload
    };
    let Ok(s) = std::str::from_utf8(body) else {
        return false;
    };
    if !s.bytes().any(|b| b == b',') {
        return false;
    }
    let nums: Vec<u8> = s
        .split(',')
        .filter_map(|p| p.trim().parse::<u8>().ok())
        .collect();
    if nums.len() < 3 {
        return false;
    }
    let mut i = 0;
    let mut hit = false;
    while i + 2 < nums.len() {
        let id = nums[i];
        let charging = nums[i + 1];
        let raw = nums[i + 2];
        let pct = if charging == 2 && raw >= 100 {
            raw - 100
        } else {
            raw
        };
        if put(id, pct, left, right, case) {
            hit = true;
        }
        i += 3;
    }
    hit
}

fn parse_packed(
    payload: &[u8],
    left: &mut Option<u8>,
    right: &mut Option<u8>,
    case: &mut Option<u8>,
) {
    let pairs = if payload.len() >= 4 && (1..=8).contains(&payload[1]) {
        &payload[2..]
    } else {
        payload
    };
    let mut i = 0;
    while i + 1 < pairs.len() {
        let id = pairs[i];
        let pct = pairs[i + 1] & 0x7f;
        put(id, pct, left, right, case);
        i += 2;
    }
}

fn put(
    id: u8,
    pct: u8,
    left: &mut Option<u8>,
    right: &mut Option<u8>,
    case: &mut Option<u8>,
) -> bool {
    let Some(p) = gage(pct) else {
        return false;
    };
    match id {
        1 if left.is_none() => {
            *left = Some(p);
            true
        }
        2 if right.is_none() => {
            *right = Some(p);
            true
        }
        3 if case.is_none() => {
            *case = Some(p);
            true
        }
        _ => false,
    }
}

fn gage(n: u8) -> Option<u8> {
    if n > 100 {
        None
    } else {
        Some(n)
    }
}

fn encode(cmd: u16, seq: u8, payload: &[u8]) -> Vec<u8> {
    let mut inner = Vec::with_capacity(5 + payload.len());
    inner.extend_from_slice(&cmd.to_le_bytes());
    inner.push(seq);
    inner.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    inner.extend_from_slice(payload);
    let mut body = vec![0, 0];
    body.extend(inner);
    let mut out = vec![SOF];
    out.extend(varint(body.len()));
    out.extend(body);
    out
}

fn varint(n: usize) -> Vec<u8> {
    let mut n = n as u32;
    let mut out = Vec::new();
    loop {
        let mut b = (n & 0x7f) as u8;
        n >>= 7;
        if n != 0 {
            b |= 0x80;
            out.push(b);
        } else {
            out.push(b);
            break;
        }
    }
    out
}

fn take_frames(mut input: &[u8]) -> Vec<Frame> {
    let mut out = Vec::new();
    while !input.is_empty() {
        match take_one(input) {
            Some((rest, frame)) => {
                out.push(frame);
                input = rest;
            }
            None => input = &input[1..],
        }
    }
    out
}

fn take_one(input: &[u8]) -> Option<(&[u8], Frame)> {
    if input.first() != Some(&SOF) || input.len() < 4 {
        return None;
    }
    let (n, vlen) = read_varint(&input[1..])?;
    let start = 1 + vlen;
    if input.len() < start + n || n < 7 {
        return None;
    }
    let body = &input[start..start + n];
    let rest = &input[start + n..];
    if body[0] != 0 || body[1] != 0 {
        return None;
    }
    let cmd = u16::from_le_bytes([body[2], body[3]]);
    let pay_len = u16::from_le_bytes([body[5], body[6]]) as usize;
    if body.len() != 7 + pay_len {
        return None;
    }
    Some((
        rest,
        Frame {
            cmd,
            payload: body[7..].to_vec(),
        },
    ))
}

fn read_varint(input: &[u8]) -> Option<(usize, usize)> {
    let mut n = 0u32;
    let mut shift = 0;
    for (i, &b) in input.iter().enumerate() {
        n |= u32::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Some((n as usize, i + 1));
        }
        shift += 7;
        if shift > 21 {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_anc_frame_is_not_cells() {
        // Nord Buds ANC-on capture: CAT 0x04, not battery.
        let frame = [
            0xaa, 0x0a, 0x00, 0x00, 0x04, 0x04, 0x40, 0x03, 0x00, 0x01, 0x01, 0x04,
        ];
        assert!(cells_from_stream(&frame).is_empty());
        let parsed = take_frames(&frame);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].cmd, 0x0404);
    }

    #[test]
    fn battery_gets_are_0105_and_0106() {
        let reqs = battery_requests();
        let cmds: Vec<u16> = reqs
            .iter()
            .flat_map(|r| take_frames(r))
            .map(|f| f.cmd)
            .collect();
        assert_eq!(cmds, [0x0105, 0x0106]);
    }

    #[test]
    fn packed_pairs_are_left_right_case() {
        let payload = [0, 3, 1, 80, 2, 75, 3, 30];
        let frame = encode(0x8106, 1, &payload);
        let cells = cells_from_stream(&frame);
        assert_eq!(named(&cells, "left"), Some(80));
        assert_eq!(named(&cells, "right"), Some(75));
        assert_eq!(named(&cells, "case"), Some(30));
    }

    #[test]
    fn csv_charging_is_offset_by_100() {
        let csv = b"1,2,118,2,2,118,3,2,102";
        let mut payload = vec![0x19, 0x00, 0x00, 0x03];
        payload.extend_from_slice(csv);
        let frame = encode(0x8105, 1, &payload);
        let cells = cells_from_stream(&frame);
        assert_eq!(named(&cells, "left"), Some(18));
        assert_eq!(named(&cells, "right"), Some(18));
        assert_eq!(named(&cells, "case"), Some(2));
    }

    #[test]
    fn charging_bit_stripped_in_packed() {
        let frame = encode(0x8106, 1, &[0, 1, 1, 80 | 0x80]);
        assert_eq!(named(&cells_from_stream(&frame), "left"), Some(80));
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

//! Galaxy Buds family SPP frames. Battery offsets are shared across generations.
//!
//! Early generations use `FE`/`EE` frames. Later ones push the same 0x60/0x61
//! status IDs inside `FD`/`DD` frames (11-bit size in the two-byte header).

use crate::device::CellReading;

use super::cell;

const SRC: &str = "samsung.rfcomm";
const PREAMBLE: u8 = 0xfe;
const POSTAMBLE: u8 = 0xee;
const PREAMBLE_V2: u8 = 0xfd;
const POSTAMBLE_V2: u8 = 0xdd;
const STATUS_IDS: &[u8] = &[0x60, 0x61];
const GET_STATUS: u8 = 0x21;
const GET_EXTENDED_STATUS: u8 = 0x22;

pub fn cells_from_stream(bytes: &[u8]) -> Vec<CellReading> {
    let mut left = None;
    let mut right = None;
    let mut case = None;
    for frame in take_frames(bytes) {
        if !STATUS_IDS.contains(&frame.msg_id) {
            continue;
        }
        if let Some((l, r, c)) = status_cells(&frame.payload) {
            if left.is_none() {
                left = l;
            }
            if right.is_none() {
                right = r;
            }
            if case.is_none() {
                case = c;
            }
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
    cells
}

/// Empty GET_STATUS / GET_EXTENDED_STATUS in both SPP framings.
pub fn status_requests() -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for id in [GET_EXTENDED_STATUS, GET_STATUS] {
        out.push(encode_fe(id, &[]));
        out.push(encode_fd(id, &[]));
    }
    out
}

fn encode_fe(msg_id: u8, payload: &[u8]) -> Vec<u8> {
    let size = 1 + payload.len() + 2;
    let mut body = vec![msg_id];
    body.extend_from_slice(payload);
    let crc = crc16_kermit(&body);
    let mut b = vec![PREAMBLE, 0, size as u8, msg_id];
    b.extend_from_slice(payload);
    b.push(crc as u8);
    b.push((crc >> 8) as u8);
    b.push(POSTAMBLE);
    b
}

fn encode_fd(msg_id: u8, payload: &[u8]) -> Vec<u8> {
    let size = (1 + payload.len() + 2) as u16;
    let mut body = vec![msg_id];
    body.extend_from_slice(payload);
    let crc = crc16_kermit(&body);
    let mut b = vec![PREAMBLE_V2];
    b.extend_from_slice(&(size & 0x07ff).to_le_bytes());
    b.push(msg_id);
    b.extend_from_slice(payload);
    b.push(crc as u8);
    b.push((crc >> 8) as u8);
    b.push(POSTAMBLE_V2);
    b
}

fn crc16_kermit(data: &[u8]) -> u16 {
    let mut crc = 0u16;
    for &b in data {
        crc ^= u16::from(b);
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0x8408;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

struct Frame {
    msg_id: u8,
    payload: Vec<u8>,
}

fn take_frames(mut input: &[u8]) -> Vec<Frame> {
    let mut out = Vec::new();
    while let Some(i) = input
        .iter()
        .position(|&b| b == PREAMBLE || b == PREAMBLE_V2)
    {
        input = &input[i..];
        match take_one(input) {
            Some((rest, f)) => {
                out.push(f);
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
    match input.first().copied() {
        Some(PREAMBLE) => take_sized(input, input.get(2).copied()? as usize, POSTAMBLE),
        Some(PREAMBLE_V2) => {
            if input.len() < 3 {
                return None;
            }
            let size = u16::from_le_bytes([input[1], input[2]]) as usize & 0x07ff;
            take_sized(input, size, POSTAMBLE_V2)
        }
        _ => None,
    }
}

fn take_sized(input: &[u8], size: usize, postamble: u8) -> Option<(&[u8], Frame)> {
    // size = msgid + payload + crc16
    if input.len() < 7 || size < 3 {
        return None;
    }
    let total = 3 + size + 1;
    if input.len() < total {
        return None;
    }
    if input[total - 1] != postamble {
        return None;
    }
    let msg_id = input[3];
    let payload_len = size - 3;
    let payload = input[4..4 + payload_len].to_vec();
    Some((&input[total..], Frame { msg_id, payload }))
}

fn status_cells(payload: &[u8]) -> Option<(Option<u8>, Option<u8>, Option<u8>)> {
    // EXTENDED_STATUS: [ver, ear, L, R, tws, ...] and Buds+ puts case at [7].
    // STATUS_UPDATED: [ear, L, R, tws, ...]
    if payload.len() >= 5 && payload[4] <= 1 && looks_pct(payload[2]) && looks_pct(payload[3]) {
        let case = if payload.len() > 7 {
            gage(payload[7])
        } else {
            None
        };
        return Some((gage(payload[2]), gage(payload[3]), case));
    }
    if payload.len() >= 4 && payload[3] <= 1 && looks_pct(payload[1]) && looks_pct(payload[2]) {
        return Some((gage(payload[1]), gage(payload[2]), None));
    }
    None
}

fn looks_pct(n: u8) -> bool {
    n <= 100
}

fn gage(n: u8) -> Option<u8> {
    // GalaxyBudsClient uses 101 as "no case reading".
    if n > 100 {
        None
    } else {
        Some(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extended_status_buds_plus_layout() {
        // FE, type, size, msgid 0x61, payload..., crc placeholder, EE
        let payload = [1u8, 0, 67, 76, 1, 1, 17, 40, 0, 0, 0, 0];
        let frame = wrap(0x61, &payload);
        let cells = cells_from_stream(&frame);
        assert_eq!(named(&cells, "left"), Some(67));
        assert_eq!(named(&cells, "right"), Some(76));
        assert_eq!(named(&cells, "case"), Some(40));
    }

    #[test]
    fn case_101_is_unknown() {
        let payload = [1u8, 0, 50, 50, 1, 0, 0, 101];
        let frame = wrap(0x61, &payload);
        let cells = cells_from_stream(&frame);
        assert_eq!(named(&cells, "left"), Some(50));
        assert!(cells.iter().all(|c| c.name != "case"));
    }

    #[test]
    fn other_message_is_ignored() {
        let payload = [67u8, 76, 1, 1];
        let frame = wrap(0x2a, &payload);
        assert!(cells_from_stream(&frame).is_empty());
    }

    #[test]
    fn fd_extended_status_later_generation() {
        // Later-generation FD/DD status (live capture). Same L/R offsets as FE/EE.
        let frame = [
            0xfd, 0x33, 0x00, 0x61, 0x0e, 0x04, 0x29, 0x12, 0x01, 0x01, 0x22, 0x00, 0x00, 0x00,
            0xbf, 0x22, 0x00, 0x00, 0x47, 0x01, 0x47, 0x01, 0x02, 0x00, 0x03, 0x66, 0x00, 0x01,
            0x00, 0x10, 0x00, 0x01, 0x00, 0x00, 0x11, 0x02, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xe3, 0xac, 0xdd,
        ];
        let cells = cells_from_stream(&frame);
        assert_eq!(named(&cells, "left"), Some(41));
        assert_eq!(named(&cells, "right"), Some(18));
        assert_eq!(named(&cells, "case"), Some(0));
    }

    #[test]
    fn status_requests_are_both_spp_framings() {
        let reqs = status_requests();
        assert!(reqs.iter().any(|f| f.first() == Some(&PREAMBLE)));
        assert!(reqs.iter().any(|f| f.first() == Some(&PREAMBLE_V2)));
        assert!(reqs.iter().any(|f| f.get(3) == Some(&GET_EXTENDED_STATUS)));
        assert!(reqs
            .iter()
            .all(|f| { matches!(f.last(), Some(&POSTAMBLE) | Some(&POSTAMBLE_V2)) }));
    }

    fn wrap(msg_id: u8, payload: &[u8]) -> Vec<u8> {
        let size = 1 + payload.len() + 2;
        let mut b = vec![PREAMBLE, 0, size as u8, msg_id];
        b.extend_from_slice(payload);
        b.extend_from_slice(&[0, 0, POSTAMBLE]);
        b
    }

    fn named(cells: &[CellReading], name: &str) -> Option<u8> {
        cells
            .iter()
            .find(|c| c.name == name)
            .and_then(|c| c.percent)
    }
}

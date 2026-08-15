//! Bose family BMAP over RFCOMM. One GET for the pack cell.
//!
//! Block 2 function 2 is BatteryLevel. Headphones report a single percent;
//! missing split cells stay unknown.

use crate::device::CellReading;

use super::cell;

const SRC: &str = "bose.rfcomm";
const BLOCK_STATUS: u8 = 2;
const FUNC_BATTERY: u8 = 2;
const OP_GET: u8 = 0x01;

pub fn request_battery() -> Vec<u8> {
    vec![BLOCK_STATUS, FUNC_BATTERY, OP_GET, 0x00]
}

pub fn cells_from_stream(bytes: &[u8]) -> Vec<CellReading> {
    let mut i = 0;
    while i + 5 <= bytes.len() {
        if bytes[i] == BLOCK_STATUS && bytes[i + 1] == FUNC_BATTERY {
            if let Some(p) = gage(bytes[i + 4]) {
                return vec![cell("pack", Some(p), SRC)];
            }
        }
        i += 1;
    }
    Vec::new()
}

fn gage(n: u8) -> Option<u8> {
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
    fn get_matches_documented_bmap() {
        assert_eq!(request_battery(), [2, 2, 0x01, 0x00]);
    }

    #[test]
    fn status_reply_byte_four_is_percent() {
        // Documented: resp[4] is the percent (0x50 = 80).
        let reply = [2, 2, 0x01, 0x01, 0x50, 0xff, 0xff, 0x00];
        let cells = cells_from_stream(&reply);
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].name, "pack");
        assert_eq!(cells[0].percent, Some(80));
    }

    #[test]
    fn unknown_ff_is_not_a_cell() {
        assert!(cells_from_stream(&[2, 2, 0x01, 0x01, 0xff]).is_empty());
    }

    #[test]
    fn garbage_is_not_cells() {
        assert!(cells_from_stream(&[1, 2, 3, 4, 5, 6, 7, 8]).is_empty());
    }
}

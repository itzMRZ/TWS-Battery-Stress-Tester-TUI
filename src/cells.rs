//! Named cells from the OS and from Family probes. Missing stays unknown.

use crate::device::CellReading;

mod apple;
mod bose;
mod nothing;
mod opo;
mod samsung;
mod sony;
mod soundcore;

pub use apple::{
    cells_from_stream as apple_cells, handshake as aap_handshake,
    request_notifications as aap_request_notifications,
    request_notifications_all as aap_request_notifications_all,
};
pub use bose::{cells_from_stream as bose_cells, request_battery as bose_request_battery};
pub use nothing::{
    activate as nothing_activate, cells_from_stream as nothing_cells,
    handshake as nothing_handshake, request_battery as nothing_request_battery,
};
pub use opo::{battery_requests as opo_battery_requests, cells_from_stream as opo_cells};
pub use samsung::{cells_from_stream as samsung_cells, status_requests as samsung_status_requests};
pub use sony::{
    acks as sony_acks, battery_requests as sony_battery_requests, cells_from_stream as sony_cells,
    handshake as sony_handshake,
};
pub use soundcore::{cells_from_stream as soundcore_cells, request_battery_level, request_state};

const ORDER: &[&str] = &["left", "right", "case", "pack", "pair"];

pub fn merge(os: Vec<CellReading>, extra: Vec<CellReading>) -> Vec<CellReading> {
    let mut out = extra;
    for c in os {
        if !out.iter().any(|e| e.name == c.name) {
            out.push(c);
        }
    }
    out.sort_by_key(|c| {
        ORDER
            .iter()
            .position(|n| *n == c.name)
            .unwrap_or(ORDER.len())
    });
    out
}

pub fn named_percent(cells: &[CellReading], name: &str) -> Option<u8> {
    cells
        .iter()
        .find(|c| c.name == name)
        .and_then(|c| c.percent)
}

pub fn headline(cells: &[CellReading]) -> Option<u8> {
    cells.iter().filter_map(|c| c.percent).min()
}

pub(crate) fn cell(name: &str, percent: Option<u8>, source: &str) -> CellReading {
    CellReading {
        name: name.into(),
        percent,
        source: source.into(),
    }
}

/// Soundcore and some TWS report 0-10 steps. 255 means the bud is gone.
pub(crate) fn scale_levels(raw: &[u8]) -> Vec<Option<u8>> {
    let valid: Vec<u8> = raw
        .iter()
        .copied()
        .filter(|&n| n != 255 && n <= 100)
        .collect();
    let tenths = !valid.is_empty() && valid.iter().all(|&n| n <= 10);
    raw.iter()
        .copied()
        .map(|n| {
            if n == 255 || n > 100 {
                None
            } else if tenths {
                Some(n.saturating_mul(10))
            } else {
                Some(n)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_keeps_os_pair_beside_split() {
        let os = vec![cell("pair", Some(70), "bluez.Battery1")];
        let extra = vec![
            cell("left", Some(50), "soundcore.rfcomm"),
            cell("right", Some(40), "soundcore.rfcomm"),
        ];
        let m = merge(os, extra);
        assert_eq!(
            m.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            ["left", "right", "pair"]
        );
        assert_eq!(headline(&m), Some(40));
    }

    #[test]
    fn two_five_five_is_unknown() {
        assert_eq!(scale_levels(&[5, 255]), [Some(50), None]);
    }
}

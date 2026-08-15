//! Apple accessory protocol over L2CAP PSM 0x1001 (AirPods and Beats).

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{FromRawFd, RawFd};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::cells;
use crate::device::CellReading;

const AF_BLUETOOTH: i32 = 31;
const BTPROTO_L2CAP: i32 = 0;
const PSM: u16 = 0x1001;
const BDADDR_BREDR: u8 = 0;

static PROBE_GATE: Mutex<()> = Mutex::new(());

#[repr(C)]
struct SockaddrL2 {
    l2_family: libc::sa_family_t,
    l2_psm: u16,
    l2_bdaddr: [u8; 6],
    l2_cid: u16,
    l2_bdaddr_type: u8,
}

pub fn probe(address: &str) -> Vec<CellReading> {
    let _gate = PROBE_GATE.lock().unwrap_or_else(|e| e.into_inner());
    try_aap(address).unwrap_or_default()
}

fn try_aap(address: &str) -> io::Result<Vec<CellReading>> {
    let mut sock = connect(address)?;
    sock.write_all(cells::aap_handshake())?;
    let mut found = collect(&mut sock, Duration::from_millis(2000));
    sock.write_all(cells::aap_request_notifications())?;
    sock.write_all(cells::aap_request_notifications_all())?;
    merge(&mut found, collect(&mut sock, Duration::from_millis(2000)));
    if named_missing(&found) {
        merge(&mut found, collect(&mut sock, Duration::from_millis(1500)));
    }
    Ok(found)
}

fn collect(sock: &mut File, budget: Duration) -> Vec<CellReading> {
    let deadline = Instant::now() + budget;
    let mut found = Vec::new();
    let mut chunk = [0u8; 2048];
    while Instant::now() < deadline && named_missing(&found) {
        match sock.read(&mut chunk) {
            Ok(0) => std::thread::sleep(Duration::from_millis(40)),
            Ok(n) => merge(&mut found, cells::apple_cells(&chunk[..n])),
            Err(e)
                if e.kind() == io::ErrorKind::WouldBlock
                    || e.kind() == io::ErrorKind::TimedOut
                    || e.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    found
}

fn merge(into: &mut Vec<CellReading>, extra: Vec<CellReading>) {
    for c in extra {
        if !into.iter().any(|e| e.name == c.name) {
            into.push(c);
        }
    }
}

fn named_missing(cells: &[CellReading]) -> bool {
    let left = cells
        .iter()
        .any(|c| c.name == "left" && c.percent.is_some());
    let right = cells
        .iter()
        .any(|c| c.name == "right" && c.percent.is_some());
    !(left && right)
}

fn connect(address: &str) -> io::Result<File> {
    let bdaddr = parse_bdaddr(address)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "bad bluetooth address"))?;
    let fd = unsafe { libc::socket(AF_BLUETOOTH, libc::SOCK_SEQPACKET, BTPROTO_L2CAP) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let tv = libc::timeval {
        tv_sec: 0,
        tv_usec: 600_000,
    };
    unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            &tv as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::timeval>() as u32,
        );
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_SNDTIMEO,
            &tv as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::timeval>() as u32,
        );
        let flags = libc::fcntl(fd, libc::F_GETFD);
        if flags >= 0 {
            libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC);
        }
    }
    let addr = SockaddrL2 {
        l2_family: AF_BLUETOOTH as libc::sa_family_t,
        l2_psm: PSM.to_le(),
        l2_bdaddr: bdaddr,
        l2_cid: 0,
        l2_bdaddr_type: BDADDR_BREDR,
    };
    let rc = unsafe {
        libc::connect(
            fd,
            &addr as *const _ as *const libc::sockaddr,
            std::mem::size_of::<SockaddrL2>() as u32,
        )
    };
    if rc != 0 {
        let err = io::Error::last_os_error();
        unsafe {
            libc::close(fd);
        }
        return Err(err);
    }
    Ok(unsafe { File::from_raw_fd(fd as RawFd) })
}

fn parse_bdaddr(address: &str) -> Option<[u8; 6]> {
    let mut parts = [0u8; 6];
    let mut i = 0;
    for p in address.split(':') {
        if i >= 6 {
            return None;
        }
        parts[i] = u8::from_str_radix(p, 16).ok()?;
        i += 1;
    }
    if i != 6 {
        return None;
    }
    parts.reverse();
    Some(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sockaddr_l2_is_packed() {
        assert!(std::mem::size_of::<SockaddrL2>() >= 13);
        assert_eq!(std::mem::offset_of!(SockaddrL2, l2_psm), 2);
        assert_eq!(std::mem::offset_of!(SockaddrL2, l2_bdaddr), 4);
    }
}

//! BR/EDR RFCOMM for Family cell probes. Channel is discovered, then cached.

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{FromRawFd, RawFd};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::brand::Family;
use crate::cells;
use crate::device::CellReading;

const AF_BLUETOOTH: i32 = 31;
const BTPROTO_RFCOMM: i32 = 3;

const SOUNDCORE_CHANNELS: &[u8] = &[10, 12, 8, 9, 11, 5, 7, 4, 6, 3, 2, 15, 1];
/// Classic SPP (1-2) then later Gear Manager channels. Hint from a prior probe wins.
const SAMSUNG_CHANNELS: &[u8] = &[
    1, 2, 20, 21, 27, 3, 4, 5, 6, 19, 22, 23, 24, 25, 26, 28, 29, 30, 7, 8, 9, 10,
];
/// Tandem/MDR channel varies by generation (9, 15, 18 seen in the wild).
const SONY_CHANNELS: &[u8] = &[9, 15, 18, 1, 8, 13, 16, 12, 10, 7];
/// Ear / CMF: classic 15, then nearby SPP numbers.
const NOTHING_CHANNELS: &[u8] = &[15, 17, 16, 18, 14, 13, 1, 12, 11, 10];
/// BMAP control is 2 on later QC, 8 on QC35.
const BOSE_CHANNELS: &[u8] = &[2, 8, 1, 14];
/// HeyMelody / OPO: RFCOMM 15 (DLCI 30).
const OPO_CHANNELS: &[u8] = &[15, 1, 14, 13, 16, 12];

static PROBE_GATE: Mutex<()> = Mutex::new(());

#[repr(C)]
struct SockaddrRc {
    rc_family: libc::sa_family_t,
    rc_bdaddr: [u8; 6],
    rc_channel: u8,
}

pub fn probe(address: &str, family: Family, hint: Option<u8>) -> (Vec<CellReading>, Option<u8>) {
    let _gate = PROBE_GATE.lock().unwrap_or_else(|e| e.into_inner());
    let mut channels: Vec<u8> = match family {
        Family::Soundcore => SOUNDCORE_CHANNELS.to_vec(),
        Family::Samsung => SAMSUNG_CHANNELS.to_vec(),
        Family::Sony => SONY_CHANNELS.to_vec(),
        Family::Nothing => NOTHING_CHANNELS.to_vec(),
        Family::Bose => BOSE_CHANNELS.to_vec(),
        Family::Oppo | Family::OnePlus | Family::Realme => OPO_CHANNELS.to_vec(),
        _ => return (Vec::new(), None),
    };
    if let Some(h) = hint {
        channels.retain(|c| *c != h);
        channels.insert(0, h);
    }
    for ch in channels {
        match try_channel(address, family, ch) {
            Ok(cells) if !cells.is_empty() => return (cells, Some(ch)),
            Ok(_) => {}
            Err(e) if e.raw_os_error() == Some(libc::EBUSY) => {
                std::thread::sleep(Duration::from_millis(180));
                if let Ok(cells) = try_channel(address, family, ch) {
                    if !cells.is_empty() {
                        return (cells, Some(ch));
                    }
                }
            }
            Err(_) => {}
        }
    }
    (Vec::new(), None)
}

fn try_channel(address: &str, family: Family, channel: u8) -> io::Result<Vec<CellReading>> {
    let mut sock = connect(address, channel)?;
    match family {
        Family::Soundcore => {
            sock.write_all(&cells::request_battery_level())?;
            let mut buf = read_for(&mut sock, Duration::from_millis(500));
            let mut found = cells::soundcore_cells(&buf);
            if found.len() < 2 {
                sock.write_all(&cells::request_state())?;
                buf.extend(read_for(&mut sock, Duration::from_millis(600)));
                found = cells::soundcore_cells(&buf);
            }
            Ok(found)
        }
        Family::Samsung => {
            let mut buf = read_for(&mut sock, Duration::from_millis(800));
            let mut found = cells::samsung_cells(&buf);
            if found.is_empty() {
                for frame in cells::samsung_status_requests() {
                    sock.write_all(&frame)?;
                    buf.extend(read_for(&mut sock, Duration::from_millis(400)));
                    found = cells::samsung_cells(&buf);
                    if !found.is_empty() {
                        break;
                    }
                }
            }
            Ok(found)
        }
        Family::Sony => sony_session(&mut sock),
        Family::Nothing => nothing_session(&mut sock),
        Family::Bose => {
            sock.write_all(&cells::bose_request_battery())?;
            let buf = read_for(&mut sock, Duration::from_millis(500));
            Ok(cells::bose_cells(&buf))
        }
        Family::Oppo | Family::OnePlus | Family::Realme => opo_session(&mut sock),
        _ => Ok(Vec::new()),
    }
}

fn sony_session(sock: &mut File) -> io::Result<Vec<CellReading>> {
    sock.write_all(&cells::sony_handshake())?;
    let mut buf = read_for(sock, Duration::from_millis(600));
    write_all_frames(sock, &cells::sony_acks(&buf))?;
    for req in cells::sony_battery_requests(&buf) {
        sock.write_all(&req)?;
        let chunk = read_for(sock, Duration::from_millis(400));
        write_all_frames(sock, &cells::sony_acks(&chunk))?;
        buf.extend(chunk);
    }
    Ok(cells::sony_cells(&buf))
}

fn nothing_session(sock: &mut File) -> io::Result<Vec<CellReading>> {
    sock.write_all(&cells::nothing_handshake())?;
    let mut buf = read_for(sock, Duration::from_millis(500));
    sock.write_all(&cells::nothing_activate())?;
    buf.extend(read_for(sock, Duration::from_millis(300)));
    sock.write_all(&cells::nothing_request_battery())?;
    buf.extend(read_for(sock, Duration::from_millis(500)));
    Ok(cells::nothing_cells(&buf))
}

fn opo_session(sock: &mut File) -> io::Result<Vec<CellReading>> {
    let mut buf = Vec::new();
    for req in cells::opo_battery_requests() {
        sock.write_all(&req)?;
        buf.extend(read_for(sock, Duration::from_millis(400)));
        if !cells::opo_cells(&buf).is_empty() {
            break;
        }
    }
    Ok(cells::opo_cells(&buf))
}

fn write_all_frames(sock: &mut File, frames: &[Vec<u8>]) -> io::Result<()> {
    for f in frames {
        sock.write_all(f)?;
    }
    Ok(())
}

fn connect(address: &str, channel: u8) -> io::Result<File> {
    let bdaddr = parse_bdaddr(address)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "bad bluetooth address"))?;
    let fd = unsafe { libc::socket(AF_BLUETOOTH, libc::SOCK_STREAM, BTPROTO_RFCOMM) };
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
    let addr = SockaddrRc {
        rc_family: AF_BLUETOOTH as libc::sa_family_t,
        rc_bdaddr: bdaddr,
        rc_channel: channel,
    };
    let rc = unsafe {
        libc::connect(
            fd,
            &addr as *const _ as *const libc::sockaddr,
            std::mem::size_of::<SockaddrRc>() as u32,
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

fn read_for(sock: &mut File, budget: Duration) -> Vec<u8> {
    let deadline = Instant::now() + budget;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 512];
    while Instant::now() < deadline {
        match sock.read(&mut chunk) {
            Ok(0) => {
                if !buf.is_empty() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(40));
            }
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e)
                if e.kind() == io::ErrorKind::WouldBlock
                    || e.kind() == io::ErrorKind::TimedOut
                    || e.kind() == io::ErrorKind::Interrupted =>
            {
                if !buf.is_empty() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    buf
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
    fn mac_is_reversed_for_rfcomm() {
        assert_eq!(
            parse_bdaddr("18:9C:2C:34:0B:D4"),
            Some([0xD4, 0x0B, 0x34, 0x2C, 0x9C, 0x18])
        );
    }

    #[test]
    fn sockaddr_matches_glibc() {
        assert_eq!(std::mem::size_of::<SockaddrRc>(), 10);
        assert_eq!(std::mem::offset_of!(SockaddrRc, rc_channel), 8);
    }
}

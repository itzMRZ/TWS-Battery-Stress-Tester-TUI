//! Host adapters: BlueZ, PipeWire, sleep inhibit. Linux is the tested path.

use std::path::Path;
use std::process::{ChildStdin, Stdio};
use std::thread::JoinHandle;

use anyhow::Result;

use crate::device::{AncMode, FoundDevice};

pub struct PlayHandle {
    child: std::process::Child,
    _pump: Option<JoinHandle<()>>,
}

impl PlayHandle {
    pub fn spawn(mut cmd: std::process::Command) -> Result<Self> {
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        Self::spawn_inner(cmd, None)
    }

    pub fn spawn_piped(mut cmd: std::process::Command) -> Result<(Self, ChildStdin)> {
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        let mut child = cmd.spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("player stdin"))?;
        Ok((Self { child, _pump: None }, stdin))
    }

    fn spawn_inner(mut cmd: std::process::Command, pump: Option<JoinHandle<()>>) -> Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        Ok(Self {
            child: cmd.spawn()?,
            _pump: pump,
        })
    }

    pub fn with_pump(mut self, pump: JoinHandle<()>) -> Self {
        self._pump = Some(pump);
        self
    }

    pub fn alive(&mut self) -> bool {
        !matches!(self.child.try_wait(), Ok(Some(_)) | Err(_))
    }
}

impl Drop for PlayHandle {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::killpg(self.child.id() as i32, libc::SIGTERM);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        // Pump thread exits on EPIPE; do not join (quit must not block).
    }
}

pub enum Host {
    #[cfg(target_os = "linux")]
    Linux(linux::LinuxHost),
    Unsupported(String),
}

impl Host {
    pub async fn connect() -> Self {
        #[cfg(target_os = "linux")]
        match linux::LinuxHost::connect().await {
            Ok(h) => Self::Linux(h),
            Err(e) => Self::Unsupported(format!("bluetooth: {e}")),
        }
        #[cfg(not(target_os = "linux"))]
        Self::Unsupported("this OS is experimental; Linux is the tested path".into())
    }

    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Unsupported(s) => Some(s.as_str()),
            #[cfg(target_os = "linux")]
            Self::Linux(_) => None,
        }
    }

    /// bluetoothd may start after the TUI. Retry the system bus on Linux
    /// when the first connect failed.
    pub async fn reconnect_if_needed(&mut self) {
        #[cfg(target_os = "linux")]
        if matches!(self, Self::Unsupported(_)) {
            *self = Self::connect().await;
        }
    }

    pub async fn scan(&self) -> Result<Vec<FoundDevice>> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Linux(h) => h.scan().await,
            Self::Unsupported(_) => Ok(Vec::new()),
        }
    }

    pub async fn set_host_volume(&self, sink: &str, percent: u8) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Linux(_) => linux::set_host_volume(sink, percent),
            Self::Unsupported(_) => Ok(()),
        }
    }

    pub async fn set_profile(&self, card: &str, profile: &str) -> Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Linux(_) => linux::set_profile(card, profile),
            Self::Unsupported(_) => Ok(()),
        }
    }

    pub async fn set_anc(&self, _address: &str, _mode: AncMode) -> Result<()> {
        anyhow::bail!("this Device cannot switch ANC from here")
    }

    pub fn play_loop(&self, sink: Option<&str>, wav: &Path) -> Result<PlayHandle> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Linux(_) => linux::play_loop(sink, wav),
            Self::Unsupported(s) => anyhow::bail!("{s}"),
        }
    }

    pub fn inhibit_sleep(&self) -> Result<PlayHandle> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Linux(_) => linux::inhibit_sleep(),
            Self::Unsupported(_) => anyhow::bail!("no sleep inhibit"),
        }
    }

    /// Named command dumps for a probe. Addresses are connected Devices from scan.
    pub fn probe_logs(&self, addresses: &[String]) -> Vec<(String, String)> {
        let mut logs = vec![(
            "os".into(),
            format!(
                "os {}\narch {}\nfamily {}\nhost {}\n",
                std::env::consts::OS,
                std::env::consts::ARCH,
                std::env::consts::FAMILY,
                self.error().unwrap_or("ok")
            ),
        )];
        #[cfg(target_os = "linux")]
        logs.extend(linux::probe_logs(addresses));
        #[cfg(windows)]
        logs.extend(windows::probe_logs());
        #[cfg(not(target_os = "linux"))]
        let _ = addresses;
        logs
    }
}

pub(crate) fn cmd_output(bin: &str, args: &[&str]) -> String {
    match std::process::Command::new(bin).args(args).output() {
        Ok(o) => {
            let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
            let err = String::from_utf8_lossy(&o.stderr);
            if !err.trim().is_empty() {
                if !s.is_empty() {
                    s.push_str("\n--- stderr ---\n");
                }
                s.push_str(&err);
            }
            if s.trim().is_empty() {
                format!("(exit {})", o.status)
            } else {
                s
            }
        }
        Err(e) => format!("not run: {e}"),
    }
}

#[cfg(target_os = "linux")]
mod aap;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod rfcomm;
#[cfg(windows)]
mod windows;

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[tokio::test]
    async fn scan_smoke() {
        let h = super::Host::connect().await;
        if let Some(e) = h.error() {
            eprintln!("host: {e}");
            return;
        }
        let list = h.scan().await.expect("scan");
        for d in &list {
            eprintln!(
                "{} {} class={} brand={} connected={} pct={:?} vol={:?} sink={:?} codec={:?} pretty={}",
                d.name,
                d.address,
                d.class.label(),
                d.brand.slug,
                d.connected,
                d.headline_percent(),
                d.host_volume,
                d.sink,
                d.codec,
                d.pretty_codec()
            );
        }
    }

    #[tokio::test]
    async fn live_p30i_volume_and_playback() {
        let h = super::Host::connect().await;
        if h.error().is_some() {
            return;
        }
        let list = h.scan().await.expect("scan");
        let d = list
            .iter()
            .find(|d| d.address.eq_ignore_ascii_case("18:9C:2C:34:0B:D4"))
            .expect("P30i paired");
        assert!(d.connected, "connect the P30i");
        assert!(d.headline_percent().is_some(), "battery % missing");
        assert_eq!(d.brand.slug, "soundcore");
        assert_eq!(d.class, crate::device::DeviceClass::Tws);
        assert_eq!(d.pretty_codec(), "AAC");
        let sink = d.sink.clone().expect("pulse sink");
        let vol = d.host_volume.expect("host volume");
        let addr = d.address.clone();
        h.set_host_volume(&sink, vol)
            .await
            .expect("set volume to current");
        let again = h.scan().await.expect("rescan");
        let d2 = again
            .iter()
            .find(|x| x.address == addr)
            .expect("still there");
        let got = d2.host_volume.expect("host volume after set");
        assert!(
            (got as i16 - vol as i16).abs() <= 2,
            "volume roundtrip {vol} -> {got}"
        );

        let dir = std::env::temp_dir().join("tws-tester-live");
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("reference.wav");
        crate::reference::write_wav(&wav).unwrap();
        let player = h.play_loop(Some(&sink), &wav).expect("play");
        tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
        drop(player);
        let inhibit = h.inhibit_sleep();
        assert!(inhibit.is_ok(), "systemd-inhibit: {:?}", inhibit.err());
    }

    #[tokio::test]
    async fn live_p30i_split_cells() {
        let h = super::Host::connect().await;
        if h.error().is_some() {
            return;
        }
        let list = h.scan().await.expect("scan");
        let Some(d) = list
            .iter()
            .find(|d| d.address.eq_ignore_ascii_case("18:9C:2C:34:0B:D4"))
        else {
            return;
        };
        if !d.connected {
            return;
        }
        for _ in 0..40 {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            let list = h.scan().await.expect("scan");
            let Some(d) = list
                .iter()
                .find(|d| d.address.eq_ignore_ascii_case("18:9C:2C:34:0B:D4"))
            else {
                continue;
            };
            let left = d
                .cells
                .iter()
                .find(|c| c.name == "left")
                .and_then(|c| c.percent);
            let right = d
                .cells
                .iter()
                .find(|c| c.name == "right")
                .and_then(|c| c.percent);
            if left.is_some() && right.is_some() {
                eprintln!("P30i split cells {cells:?}", cells = d.cells);
                return;
            }
        }
        panic!("left/right cells did not appear on the Soundcore family probe");
    }

    const BUDS2_PRO: &str = "04:29:2E:F2:93:74";

    #[tokio::test]
    async fn live_buds2_pro_volume_and_playback() {
        let h = super::Host::connect().await;
        if h.error().is_some() {
            return;
        }
        let list = h.scan().await.expect("scan");
        let Some(d) = list
            .iter()
            .find(|d| d.address.eq_ignore_ascii_case(BUDS2_PRO))
        else {
            return;
        };
        if !d.connected {
            return;
        }
        eprintln!(
            "Buds2 Pro cells={:?} pct={:?} codec={} sink={:?} vol={:?}",
            d.cells,
            d.headline_percent(),
            d.pretty_codec(),
            d.sink,
            d.host_volume
        );
        assert!(d.headline_percent().is_some(), "battery % missing");
        assert_eq!(d.brand.slug, "samsung");
        assert_eq!(d.class, crate::device::DeviceClass::Tws);
        assert_eq!(d.brand.product_label(&d.name), "Samsung Galaxy buds2 pro");
        assert_eq!(d.pretty_codec(), "AAC");
        let sink = d.sink.clone().expect("pulse sink");
        let vol = d.host_volume.expect("host volume");
        let addr = d.address.clone();
        h.set_host_volume(&sink, vol)
            .await
            .expect("set volume to current");
        let again = h.scan().await.expect("rescan");
        let d2 = again
            .iter()
            .find(|x| x.address == addr)
            .expect("still there");
        let got = d2.host_volume.expect("host volume after set");
        assert!(
            (got as i16 - vol as i16).abs() <= 2,
            "volume roundtrip {vol} -> {got}"
        );

        let dir = std::env::temp_dir().join("tws-tester-live");
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("reference.wav");
        crate::reference::write_wav(&wav).unwrap();
        let player = h.play_loop(Some(&sink), &wav).expect("play");
        tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
        drop(player);
        let inhibit = h.inhibit_sleep();
        assert!(inhibit.is_ok(), "systemd-inhibit: {:?}", inhibit.err());
    }

    #[tokio::test]
    async fn live_buds2_pro_split_cells() {
        let h = super::Host::connect().await;
        if h.error().is_some() {
            return;
        }
        let list = h.scan().await.expect("scan");
        let Some(d) = list
            .iter()
            .find(|d| d.address.eq_ignore_ascii_case(BUDS2_PRO))
        else {
            return;
        };
        if !d.connected {
            return;
        }
        for _ in 0..40 {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            let list = h.scan().await.expect("scan");
            let Some(d) = list
                .iter()
                .find(|x| x.address.eq_ignore_ascii_case(BUDS2_PRO))
            else {
                continue;
            };
            let left = d
                .cells
                .iter()
                .find(|c| c.name == "left")
                .and_then(|c| c.percent);
            let right = d
                .cells
                .iter()
                .find(|c| c.name == "right")
                .and_then(|c| c.percent);
            if left.is_some() && right.is_some() {
                eprintln!("Buds2 Pro split cells {cells:?}", cells = d.cells);
                return;
            }
        }
        panic!("left/right cells did not appear on the Samsung family probe");
    }

    const AIRPODS_PRO: &str = "AC:07:75:F0:61:D2";

    #[tokio::test]
    async fn live_airpods_pro_volume_and_playback() {
        let h = super::Host::connect().await;
        if h.error().is_some() {
            return;
        }
        let list = h.scan().await.expect("scan");
        let Some(d) = list
            .iter()
            .find(|d| d.address.eq_ignore_ascii_case(AIRPODS_PRO))
        else {
            return;
        };
        if !d.connected {
            return;
        }
        eprintln!(
            "AirPods Pro cells={:?} pct={:?} class={} codec={} sink={:?} vol={:?}",
            d.cells,
            d.headline_percent(),
            d.class.label(),
            d.pretty_codec(),
            d.sink,
            d.host_volume
        );
        assert_eq!(d.brand.slug, "apple");
        assert_eq!(d.class, crate::device::DeviceClass::Tws);
        assert_eq!(
            d.brand.product_label(&d.name),
            "Apple AirPods Pro - Find My"
        );
        assert_eq!(d.pretty_codec(), "AAC");
        let sink = d.sink.clone().expect("pulse sink");
        let vol = d.host_volume.expect("host volume");
        let addr = d.address.clone();
        h.set_host_volume(&sink, vol)
            .await
            .expect("set volume to current");
        let again = h.scan().await.expect("rescan");
        let d2 = again
            .iter()
            .find(|x| x.address == addr)
            .expect("still there");
        let got = d2.host_volume.expect("host volume after set");
        assert!(
            (got as i16 - vol as i16).abs() <= 2,
            "volume roundtrip {vol} -> {got}"
        );

        let dir = std::env::temp_dir().join("tws-tester-live");
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("reference.wav");
        crate::reference::write_wav(&wav).unwrap();
        let player = h.play_loop(Some(&sink), &wav).expect("play");
        tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
        drop(player);
        let inhibit = h.inhibit_sleep();
        assert!(inhibit.is_ok(), "systemd-inhibit: {:?}", inhibit.err());
    }

    #[tokio::test]
    async fn live_airpods_pro_split_cells() {
        let h = super::Host::connect().await;
        if h.error().is_some() {
            return;
        }
        let list = h.scan().await.expect("scan");
        let Some(d) = list
            .iter()
            .find(|d| d.address.eq_ignore_ascii_case(AIRPODS_PRO))
        else {
            return;
        };
        if !d.connected {
            return;
        }
        for _ in 0..40 {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            let list = h.scan().await.expect("scan");
            let Some(d) = list
                .iter()
                .find(|x| x.address.eq_ignore_ascii_case(AIRPODS_PRO))
            else {
                continue;
            };
            let left = d
                .cells
                .iter()
                .find(|c| c.name == "left")
                .and_then(|c| c.percent);
            let right = d
                .cells
                .iter()
                .find(|c| c.name == "right")
                .and_then(|c| c.percent);
            if left.is_some() && right.is_some() {
                eprintln!("AirPods Pro split cells {cells:?}", cells = d.cells);
                return;
            }
        }
        panic!("left/right cells did not appear on the Apple family probe");
    }
}

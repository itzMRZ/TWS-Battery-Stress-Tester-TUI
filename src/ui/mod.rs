//! TUI: deck → prep → soak → archive. The process is the soak (ADR 0002).

mod draw;
mod icons;
mod theme;

#[cfg(unix)]
use std::future::pending;
use std::path::{Path, PathBuf};
use std::pin::pin;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::Local;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use ratatui::DefaultTerminal;
use tokio::time::MissedTickBehavior;

use crate::alias::{self, AliasBook};
use crate::death::{DeathWatch, Decision, EventKind, Observation};
use crate::device::{FoundDevice, Sample, SoakKind, Stimulus};
use crate::host::{Host, PlayHandle};
use crate::pack::{self, PackWriter};

const TICK: Duration = Duration::from_millis(200);
const DECK_SCAN: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Deck,
    Prep,
    Soak,
    Archive,
}

pub enum Overlay {
    None,
    Help,
    ConfirmQuit,
    ConfirmStop,
    EditAlias { address: String, draft: String },
}

pub enum NoticeKind {
    Info,
    Success,
    Warning,
    Error,
}

pub struct Notice {
    pub kind: NoticeKind,
    pub title: String,
    pub message: String,
    pub until: Instant,
}

pub struct LiveSoak {
    pub alias: String,
    pub device: FoundDevice,
    pub kind: SoakKind,
    pub stimulus: Stimulus,
    pub started: chrono::DateTime<Local>,
    pub elapsed_ms: u64,
    pub samples: Vec<Sample>,
    pub events: Vec<(u64, EventKind)>,
    pub decision: Decision,
    watch: DeathWatch,
    pack: PackWriter,
    player: PlayHandle,
    play_path: PathBuf,
    play_warned: bool,
    pack_warned: bool,
    _inhibit: Option<PlayHandle>,
    origin: Instant,
    last_sample: Instant,
    last_html: Instant,
    last_host: Instant,
}

pub struct App {
    pub screen: Screen,
    pub overlay: Overlay,
    pub devices: Vec<FoundDevice>,
    pub selected: usize,
    pub aliases: AliasBook,
    pub host: Host,
    pub host_error: Option<String>,
    pub prep_field: usize,
    pub prep_stimulus: Stimulus,
    pub prep_volume: Option<u8>,
    pub prep_codec: Option<String>,
    pub playlist: Vec<PathBuf>,
    pub playlist_idx: usize,
    playlist_loaded: bool,
    pub soak: Option<LiveSoak>,
    pub archive_alias: Option<String>,
    pub archive_items: Vec<String>,
    pub archive_sel: usize,
    pub archive_marks: Vec<usize>,
    pub notice: Option<Notice>,
    pub tick: u64,
    alias_path: PathBuf,
    last_scan: Instant,
    should_quit: bool,
}

impl App {
    pub async fn new() -> Result<Self> {
        let root = pack::library_root();
        std::fs::create_dir_all(pack::runs_root()).ok();
        let alias_path = alias::alias_file(&root);
        let (aliases, alias_warn) = AliasBook::load(&alias_path);
        let host = Host::connect().await;
        let host_error = host.error().map(|s| s.to_string());
        let mut app = Self {
            screen: Screen::Deck,
            overlay: Overlay::None,
            devices: Vec::new(),
            selected: 0,
            aliases,
            host,
            host_error,
            prep_field: 0,
            prep_stimulus: Stimulus::Reference,
            prep_volume: None,
            prep_codec: None,
            playlist: Vec::new(),
            playlist_idx: 0,
            playlist_loaded: false,
            soak: None,
            archive_alias: None,
            archive_items: Vec::new(),
            archive_sel: 0,
            archive_marks: Vec::new(),
            notice: None,
            tick: 0,
            alias_path,
            last_scan: Instant::now(),
            should_quit: false,
        };
        app.refresh_devices().await;
        app.last_scan = Instant::now();
        if let Some(msg) = alias_warn {
            app.notify(NoticeKind::Warning, "aliases", msg);
        }
        Ok(app)
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let mut events = EventStream::new();
        let mut tick = tokio::time::interval(TICK);
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut stop = pin!(wait_terminate());
        while !self.should_quit {
            terminal.draw(|f| draw::draw(f, self))?;
            tokio::select! {
                _ = tick.tick() => {
                    self.on_tick().await;
                }
                maybe = events.next() => {
                    self.on_input(maybe).await;
                }
                _ = &mut stop => {
                    self.request_stop().await;
                }
            }
        }
        Ok(())
    }

    async fn on_input(&mut self, maybe: Option<Result<Event, std::io::Error>>) {
        match maybe {
            Some(Ok(Event::Key(key))) => self.on_key(key).await,
            Some(Ok(Event::Resize(_, _))) => {}
            Some(Ok(_)) => {}
            Some(Err(_)) => tokio::time::sleep(Duration::from_millis(50)).await,
            None => self.request_stop().await,
        }
    }

    async fn request_stop(&mut self) {
        if self.soak.is_some() {
            self.finish_soak("interrupted").await;
        }
        self.should_quit = true;
    }

    fn save_aliases(&mut self) {
        if let Err(e) = self.aliases.save(&self.alias_path) {
            self.notify(NoticeKind::Error, "aliases", e.to_string());
        }
    }

    async fn refresh_devices(&mut self) {
        self.host.reconnect_if_needed().await;
        match self.host.scan().await {
            Ok(list) => {
                let mut dirty = false;
                for d in &list {
                    let prev = self.aliases.alias_of(&d.address).map(str::to_string);
                    let alias = self.aliases.ensure(&d.address, &d.name, d.brand);
                    if prev.as_deref() != Some(alias.as_str()) {
                        dirty = true;
                        if let Some(old) = prev {
                            rename_device_folder(&old, &alias);
                        }
                    }
                }
                if dirty {
                    self.save_aliases();
                }
                self.devices = list;
                if self.selected >= self.devices.len() {
                    self.selected = self.devices.len().saturating_sub(1);
                }
                self.host_error = self.host.error().map(str::to_string);
            }
            Err(e) => self.host_error = Some(e.to_string()),
        }
    }

    async fn on_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        if self
            .notice
            .as_ref()
            .is_some_and(|n| Instant::now() >= n.until)
        {
            self.notice = None;
        }
        match self.screen {
            Screen::Deck if self.soak.is_none() => {
                self.ensure_playlist();
                if self.last_scan.elapsed() >= DECK_SCAN {
                    self.refresh_devices().await;
                    self.last_scan = Instant::now();
                }
            }
            Screen::Prep => self.ensure_playlist(),
            Screen::Soak => self.tick_soak().await,
            _ => {}
        }
    }

    fn ensure_playlist(&mut self) {
        if self.playlist_loaded {
            return;
        }
        self.playlist = music_files();
        self.playlist_loaded = true;
    }

    fn notify(&mut self, kind: NoticeKind, title: &str, message: impl Into<String>) {
        let secs = match kind {
            NoticeKind::Info => 4,
            NoticeKind::Success => 5,
            NoticeKind::Warning => 7,
            NoticeKind::Error => 10,
        };
        self.notice = Some(Notice {
            kind,
            title: title.to_string(),
            message: message.into(),
            until: Instant::now() + Duration::from_secs(secs),
        });
    }

    async fn tick_soak(&mut self) {
        let addr = match &self.soak {
            Some(s) => s.device.address.clone(),
            None => return,
        };
        if let Some((sink, path)) = self.soak.as_mut().and_then(|s| {
            if s.player.alive() {
                None
            } else {
                Some((s.device.sink.clone(), s.play_path.clone()))
            }
        }) {
            match self.host.play_loop(sink.as_deref(), &path) {
                Ok(p) => {
                    if let Some(s) = self.soak.as_mut() {
                        s.player = p;
                        s.play_warned = false;
                    }
                }
                Err(e) => {
                    let msg = self.soak.as_mut().and_then(|s| {
                        if s.play_warned {
                            None
                        } else {
                            s.play_warned = true;
                            Some(format!("playback stopped: {e}"))
                        }
                    });
                    if let Some(msg) = msg {
                        self.notify(NoticeKind::Warning, "audio", msg);
                    }
                }
            }
        }
        let refresh = self
            .soak
            .as_ref()
            .map(|s| s.last_host.elapsed() >= Duration::from_secs(1))
            .unwrap_or(true);
        let snap = if refresh {
            match self.host.scan().await {
                Ok(list) => {
                    self.host_error = None;
                    list.into_iter().find(|d| d.address == addr)
                }
                Err(e) => {
                    self.host_error = Some(e.to_string());
                    None
                }
            }
        } else {
            None
        };
        let mut came_back = false;
        let mut pack_err = None;
        let dead;
        {
            let Some(s) = self.soak.as_mut() else { return };
            s.elapsed_ms = s.origin.elapsed().as_millis() as u64;
            if refresh {
                s.last_host = Instant::now();
            }
            let present = snap
                .as_ref()
                .map(|d| d.connected)
                .unwrap_or(s.device.connected);
            if let Some(d) = snap {
                s.device.cells = d.cells.clone();
                s.device.connected = d.connected;
                s.device.codec = d.codec.clone();
                s.device.host_volume = d.host_volume;
                s.device.sink = d.sink.clone();
            }
            let audio_flowing = present && s.player.alive();
            let percent = s.device.headline_percent();
            let (decision, events) = s.watch.tick(
                s.elapsed_ms,
                Observation {
                    present,
                    audio_flowing,
                    percent,
                },
                false,
            );
            s.decision = decision;
            let now = Instant::now();
            let due =
                now.duration_since(s.last_sample) >= Duration::from_secs(30) || !events.is_empty();
            if due {
                let sample = Sample {
                    t: Local::now(),
                    elapsed_ms: s.elapsed_ms,
                    cells: s.device.cells.clone(),
                    codec: s.device.codec.clone(),
                    host_volume: s.device.host_volume,
                    present,
                };
                if let Err(e) = s.pack.append_sample(&sample) {
                    if !s.pack_warned {
                        s.pack_warned = true;
                        pack_err = Some(e.to_string());
                    }
                }
                s.samples.push(sample);
                s.last_sample = now;
            }
            for e in events {
                if let Err(err) = s.pack.append_event(Local::now(), s.elapsed_ms, e) {
                    if !s.pack_warned {
                        s.pack_warned = true;
                        pack_err = Some(err.to_string());
                    }
                }
                s.events.push((s.elapsed_ms, e));
                if e == EventKind::FalseDeath {
                    came_back = true;
                }
            }
            if now.duration_since(s.last_html) >= Duration::from_secs(5) || due {
                match rewrite_pack(s) {
                    Ok(()) => s.last_html = now,
                    Err(e) => {
                        if !s.pack_warned {
                            s.pack_warned = true;
                            pack_err = Some(e.to_string());
                        }
                    }
                }
            }
            dead = matches!(decision, Decision::Dead);
        }
        if came_back {
            self.notify(NoticeKind::Success, "soak", "came back");
        }
        if let Some(msg) = pack_err {
            self.notify(NoticeKind::Error, "pack", msg);
        }
        if dead {
            self.finish_soak("dead").await;
        }
    }

    async fn on_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
            return;
        }
        if key.kind == KeyEventKind::Repeat && !repeatable_key(&self.overlay, key.code) {
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            if self.soak.is_some() {
                self.overlay = Overlay::ConfirmStop;
            } else {
                self.should_quit = true;
            }
            return;
        }
        match &mut self.overlay {
            Overlay::Help => match key.code {
                KeyCode::Char('?') | KeyCode::Esc => self.overlay = Overlay::None,
                KeyCode::Char('q') => {
                    self.overlay = if self.soak.is_some() {
                        Overlay::ConfirmStop
                    } else {
                        Overlay::ConfirmQuit
                    };
                }
                _ => {}
            },
            Overlay::ConfirmQuit => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => self.should_quit = true,
                KeyCode::Char('n') | KeyCode::Esc => self.overlay = Overlay::None,
                _ => {}
            },
            Overlay::ConfirmStop => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    self.overlay = Overlay::None;
                    self.finish_soak("interrupted").await;
                }
                KeyCode::Char('n') | KeyCode::Esc => self.overlay = Overlay::None,
                _ => {}
            },
            Overlay::EditAlias { address, draft } => match key.code {
                KeyCode::Esc => self.overlay = Overlay::None,
                KeyCode::Enter => {
                    let addr = address.clone();
                    let name = alias::sanitize(draft);
                    if let Some(old) = self.aliases.rename(&addr, &name) {
                        rename_device_folder(&old, &name);
                        self.save_aliases();
                    }
                    self.overlay = Overlay::None;
                }
                KeyCode::Backspace => {
                    draft.pop();
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => draft.push(c),
                _ => {}
            },
            Overlay::None => {
                if key.code == KeyCode::Char('?') {
                    self.overlay = Overlay::Help;
                    return;
                }
                if key.code == KeyCode::Char('q') {
                    if self.soak.is_some() {
                        self.overlay = Overlay::ConfirmStop;
                    } else {
                        self.overlay = Overlay::ConfirmQuit;
                    }
                    return;
                }
                match self.screen {
                    Screen::Deck => self.deck_key(key).await,
                    Screen::Prep => self.prep_key(key).await,
                    Screen::Soak => self.soak_key(key).await,
                    Screen::Archive => self.archive_key(key).await,
                }
            }
        }
    }

    async fn deck_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.overlay = Overlay::ConfirmQuit,
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.devices.is_empty() {
                    self.selected = (self.selected + 1) % self.devices.len();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if !self.devices.is_empty() {
                    self.selected = (self.selected + self.devices.len() - 1) % self.devices.len();
                }
            }
            KeyCode::Home => self.selected = 0,
            KeyCode::End => {
                self.selected = self.devices.len().saturating_sub(1);
            }
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('l') => self.open_prep(),
            KeyCode::Char('a') => {
                if let Some(d) = self.devices.get(self.selected) {
                    let draft = self
                        .aliases
                        .alias_of(&d.address)
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| d.brand.product_label(&d.name));
                    self.overlay = Overlay::EditAlias {
                        address: d.address.clone(),
                        draft,
                    };
                }
            }
            KeyCode::Char('p') => self.open_archive(),
            KeyCode::Char('o') => {
                if let Some(d) = self.devices.get(self.selected) {
                    if let Some(a) = self.aliases.alias_of(&d.address) {
                        let dir = pack::device_dir(a);
                        if let Err(e) = std::fs::create_dir_all(&dir) {
                            self.notify(NoticeKind::Error, "folder", e.to_string());
                        } else if let Err(e) = open::that(&dir) {
                            self.notify(NoticeKind::Error, "folder", e.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn open_prep(&mut self) {
        let Some(d) = self.devices.get(self.selected) else {
            return;
        };
        if !d.connected {
            self.notify(NoticeKind::Warning, "deck", "connect it first");
            return;
        }
        self.prep_field = 0;
        self.prep_stimulus = Stimulus::default_for(d.class);
        self.prep_volume = d.host_volume;
        self.prep_codec = d.codec.clone();
        self.screen = Screen::Prep;
    }

    fn open_archive(&mut self) {
        let Some(d) = self.devices.get(self.selected) else {
            return;
        };
        let alias = self.aliases.alias_of(&d.address).unwrap_or("?").to_string();
        self.archive_items = pack::list_soaks(&alias);
        self.archive_alias = Some(alias);
        self.archive_sel = 0;
        self.archive_marks.clear();
        self.screen = Screen::Archive;
    }

    async fn prep_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.screen = Screen::Deck,
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => {
                self.prep_field = (self.prep_field + 1) % 3;
            }
            KeyCode::Char('k') | KeyCode::Up | KeyCode::BackTab => {
                self.prep_field = (self.prep_field + 2) % 3;
            }
            KeyCode::Char('1') => self.prep_field = 0,
            KeyCode::Char('2') => self.prep_field = 1,
            KeyCode::Char('3') => self.prep_field = 2,
            KeyCode::Char('s') => {
                self.prep_field = 0;
                self.nudge_prep(1).await;
            }
            KeyCode::Char('c') => {
                self.prep_field = 2;
                self.nudge_prep(1).await;
            }
            KeyCode::Char('[') | KeyCode::Char('-') => {
                self.prep_field = 1;
                self.nudge_prep(-1).await;
            }
            KeyCode::Char(']') | KeyCode::Char('=') | KeyCode::Char('+') => {
                self.prep_field = 1;
                self.nudge_prep(1).await;
            }
            KeyCode::Char('h') | KeyCode::Left => self.nudge_prep(-1).await,
            KeyCode::Char('l') | KeyCode::Right => self.nudge_prep(1).await,
            KeyCode::Char(' ') | KeyCode::Enter => self.start_soak().await,
            _ => {}
        }
    }

    async fn nudge_prep(&mut self, dir: i8) {
        match self.prep_field {
            0 => {
                self.prep_stimulus = match self.prep_stimulus {
                    Stimulus::Reference => Stimulus::Playlist,
                    Stimulus::Playlist => Stimulus::Reference,
                };
            }
            1 => {
                let v = self.prep_volume.unwrap_or(50) as i16 + i16::from(dir) * 5;
                self.prep_volume = Some(v.clamp(0, 100) as u8);
            }
            2 => {
                if let Some(d) = self.devices.get(self.selected) {
                    if d.profiles.is_empty() {
                        return;
                    }
                    let cur = self.prep_codec.as_deref().unwrap_or("");
                    let idx = d.profiles.iter().position(|p| p == cur).unwrap_or(0);
                    let next =
                        (idx as i32 + i32::from(dir)).rem_euclid(d.profiles.len() as i32) as usize;
                    self.prep_codec = Some(d.profiles[next].clone());
                }
            }
            _ => {}
        }
    }

    async fn start_soak(&mut self) {
        if self.soak.is_some() {
            return;
        }
        self.ensure_playlist();
        let Some(d) = self.devices.get(self.selected).cloned() else {
            return;
        };
        if let (Some(card), Some(profile)) = (d.card.as_deref(), self.prep_codec.as_deref()) {
            if let Err(e) = self.host.set_profile(card, profile).await {
                self.notify(NoticeKind::Warning, "codec", e.to_string());
            }
        }
        if let (Some(sink), Some(vol)) = (d.sink.as_deref(), self.prep_volume) {
            if let Err(e) = self.host.set_host_volume(sink, vol).await {
                self.notify(NoticeKind::Warning, "volume", e.to_string());
            }
        }
        let alias = self
            .aliases
            .alias_of(&d.address)
            .unwrap_or("device")
            .to_string();
        let kind = SoakKind::from_start_percent(d.headline_percent());
        let started = Local::now();
        let pack = match PackWriter::create(&alias, started, kind, &d, self.prep_stimulus) {
            Ok(p) => p,
            Err(e) => {
                self.notify(NoticeKind::Error, "pack", e.to_string());
                return;
            }
        };
        let wav = pack.dir().join("reference.wav");
        if crate::reference::write_wav(&wav).is_err() {
            self.notify(NoticeKind::Error, "audio", "could not write reference loop");
            return;
        }
        let play_path = if self.prep_stimulus == Stimulus::Playlist {
            self.playlist
                .get(self.playlist_idx)
                .cloned()
                .unwrap_or(wav.clone())
        } else {
            wav
        };
        let player = match self.host.play_loop(d.sink.as_deref(), &play_path) {
            Ok(p) => p,
            Err(e) => {
                self.notify(NoticeKind::Error, "audio", e.to_string());
                return;
            }
        };
        let inhibit = match self.host.inhibit_sleep() {
            Ok(h) => Some(h),
            Err(_) => {
                self.notify(
                    NoticeKind::Warning,
                    "sleep",
                    "could not inhibit sleep; machine may suspend",
                );
                None
            }
        };
        let now = Instant::now();
        self.soak = Some(LiveSoak {
            alias,
            device: d,
            kind,
            stimulus: self.prep_stimulus,
            started,
            elapsed_ms: 0,
            samples: Vec::new(),
            events: Vec::new(),
            decision: Decision::Live,
            watch: DeathWatch::typical(),
            pack,
            player,
            play_path,
            play_warned: false,
            pack_warned: false,
            _inhibit: inhibit,
            origin: now,
            last_sample: now - Duration::from_secs(30),
            last_html: now,
            last_host: now - Duration::from_secs(1),
        });
        self.screen = Screen::Soak;
        self.tick_soak().await;
    }

    async fn soak_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.overlay = Overlay::ConfirmStop,
            KeyCode::Char('o') => {
                if let Some(s) = &self.soak {
                    if let Err(e) = open::that(s.pack.dir()) {
                        self.notify(NoticeKind::Error, "folder", e.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    async fn archive_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('h') => self.screen = Screen::Deck,
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.archive_items.is_empty() {
                    self.archive_sel = (self.archive_sel + 1) % self.archive_items.len();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if !self.archive_items.is_empty() {
                    self.archive_sel = (self.archive_sel + self.archive_items.len() - 1)
                        % self.archive_items.len();
                }
            }
            KeyCode::Home => self.archive_sel = 0,
            KeyCode::End => {
                self.archive_sel = self.archive_items.len().saturating_sub(1);
            }
            KeyCode::Char(' ') => {
                if let Some(i) = self
                    .archive_marks
                    .iter()
                    .position(|x| *x == self.archive_sel)
                {
                    self.archive_marks.remove(i);
                } else {
                    if self.archive_marks.len() == 2 {
                        self.archive_marks.remove(0);
                    }
                    self.archive_marks.push(self.archive_sel);
                }
            }
            KeyCode::Char('v') => {
                if self.archive_marks.len() == 2 {
                    if let Some(alias) = self.archive_alias.clone() {
                        let a = self.archive_items[self.archive_marks[0]].clone();
                        let b = self.archive_items[self.archive_marks[1]].clone();
                        match pack::write_overlay(&alias, &a, &b) {
                            Ok(p) => {
                                if let Err(e) = open::that(&p) {
                                    self.notify(NoticeKind::Error, "overlay", e.to_string());
                                }
                            }
                            Err(_) => {
                                self.notify(
                                    NoticeKind::Error,
                                    "overlay",
                                    "could not write overlay",
                                );
                            }
                        }
                    }
                }
            }
            KeyCode::Char('o') | KeyCode::Enter => {
                let p = match (
                    self.archive_alias.as_deref(),
                    self.archive_items.get(self.archive_sel),
                ) {
                    (Some(alias), Some(name)) => {
                        pack::device_dir(alias).join(name).join("report.html")
                    }
                    _ => return,
                };
                if let Err(e) = open::that(&p) {
                    self.notify(NoticeKind::Error, "pack", e.to_string());
                }
            }
            _ => {}
        }
    }

    async fn finish_soak(&mut self, reason: &str) {
        if let Some(mut s) = self.soak.take() {
            s.decision = if reason == "dead" {
                Decision::Dead
            } else {
                Decision::Interrupted
            };
            let wrote = rewrite_pack_reason(&mut s, reason);
            let dir = s.pack.dir().to_path_buf();
            drop(s);
            match wrote {
                Ok(()) => self.notify(
                    NoticeKind::Success,
                    "pack",
                    format!("pack in {}", dir.display()),
                ),
                Err(e) => self.notify(NoticeKind::Error, "pack", e.to_string()),
            }
        }
        self.screen = Screen::Deck;
        self.refresh_devices().await;
    }
}

async fn wait_terminate() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(_) => {
                pending::<()>().await;
                return;
            }
        };
        let mut int = signal(SignalKind::interrupt()).ok();
        let mut hup = signal(SignalKind::hangup()).ok();
        tokio::select! {
            _ = term.recv() => {}
            _ = async {
                if let Some(s) = int.as_mut() {
                    s.recv().await;
                } else {
                    pending::<()>().await;
                }
            } => {}
            _ = async {
                if let Some(s) = hup.as_mut() {
                    s.recv().await;
                } else {
                    pending::<()>().await;
                }
            } => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

fn repeatable_key(overlay: &Overlay, code: KeyCode) -> bool {
    if matches!(overlay, Overlay::EditAlias { .. }) {
        return true;
    }
    matches!(
        code,
        KeyCode::Up
            | KeyCode::Down
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::Backspace
            | KeyCode::Tab
            | KeyCode::BackTab
            | KeyCode::Char('j' | 'k' | 'h' | 'l' | '[' | ']' | '-' | '=' | '+')
    )
}

fn rewrite_pack(s: &LiveSoak) -> std::io::Result<()> {
    s.pack.rewrite_human(
        &s.alias,
        s.started,
        s.kind,
        &s.device,
        s.stimulus,
        &s.samples,
        &s.events,
        None,
        match s.decision {
            Decision::Live => "in progress",
            Decision::Confirming => "confirming death",
            Decision::Dead => "dead",
            Decision::Interrupted => "interrupted",
        },
    )
}

fn rewrite_pack_reason(s: &mut LiveSoak, reason: &str) -> std::io::Result<()> {
    if reason != "dead" && !matches!(s.events.last(), Some((_, EventKind::Interrupted))) {
        let e = EventKind::Interrupted;
        let _ = s.pack.append_event(Local::now(), s.elapsed_ms, e);
        s.events.push((s.elapsed_ms, e));
    }
    s.pack.rewrite_human(
        &s.alias,
        s.started,
        s.kind,
        &s.device,
        s.stimulus,
        &s.samples,
        &s.events,
        Some(reason),
        reason,
    )
}

fn rename_device_folder(old: &str, new: &str) {
    let from = pack::device_dir(old);
    let to = pack::device_dir(new);
    if from.exists() && from != to {
        let _ = std::fs::rename(from, to);
    }
}

fn music_files() -> Vec<PathBuf> {
    let root = dirs::audio_dir().or_else(|| dirs::home_dir().map(|h| h.join("Music")));
    let Some(root) = root else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect_audio(&root, &mut out, 0);
    out.sort();
    out.truncate(40);
    out
}

fn collect_audio(dir: &Path, out: &mut Vec<PathBuf>, depth: u8) {
    if depth > 2 || out.len() >= 40 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut seen = 0u16;
    for e in rd.flatten() {
        if out.len() >= 40 {
            return;
        }
        seen += 1;
        if seen > 256 {
            break;
        }
        let name = e.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let p = e.path();
        if p.is_dir() {
            collect_audio(&p, out, depth + 1);
        } else if matches!(
            p.extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_ascii_lowercase())
                .as_deref(),
            Some("wav" | "flac" | "ogg" | "mp3" | "m4a" | "opus")
        ) {
            out.push(p);
        }
    }
}

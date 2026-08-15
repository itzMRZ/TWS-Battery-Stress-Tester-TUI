//! Death: playback gone for good, not a reported 0% while audio still flows.

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    ReportedEmptyStillPlaying,
    PercentStuck,
    Blip,
    FalseDeath,
    Confirming,
    Dead,
    Interrupted,
}

impl EventKind {
    /// Stable slug in JSONL, CSV, and chart markers. Renaming breaks existing packs.
    pub fn tag(self) -> &'static str {
        match self {
            Self::ReportedEmptyStillPlaying => "empty_playing",
            Self::PercentStuck => "percent_stuck",
            Self::Blip => "blip",
            Self::FalseDeath => "false_death",
            Self::Confirming => "confirming",
            Self::Dead => "dead",
            Self::Interrupted => "interrupted",
        }
    }
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ReportedEmptyStillPlaying => "firmware says empty, still playing",
            Self::PercentStuck => "percent stuck, still playing",
            Self::Blip => "brief disconnect",
            Self::FalseDeath => "came back (false death)",
            Self::Confirming => "waiting (might be dead)",
            Self::Dead => "dead",
            Self::Interrupted => "stopped",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Live,
    Confirming,
    Dead,
    Interrupted,
}

#[derive(Clone, Copy, Debug)]
pub struct Observation {
    pub present: bool,
    pub audio_flowing: bool,
    pub percent: Option<u8>,
}

#[derive(Debug)]
pub struct DeathWatch {
    grace_ms: u64,
    confirm_ms: u64,
    stuck_ms: u64,
    absent_since: Option<u64>,
    silent_since: Option<u64>,
    confirming_since: Option<u64>,
    last_percent: Option<u8>,
    percent_unchanged_since: Option<u64>,
    emitted_empty: bool,
    emitted_stuck: bool,
    emitted_confirming: bool,
    emitted_dead: bool,
}

impl DeathWatch {
    pub fn new(grace_ms: u64, confirm_ms: u64) -> Self {
        Self {
            grace_ms,
            confirm_ms,
            stuck_ms: 10 * 60 * 1000,
            absent_since: None,
            silent_since: None,
            confirming_since: None,
            last_percent: None,
            percent_unchanged_since: None,
            emitted_empty: false,
            emitted_stuck: false,
            emitted_confirming: false,
            emitted_dead: false,
        }
    }

    pub fn typical() -> Self {
        Self::new(30_000, 15_000)
    }

    pub fn tick(
        &mut self,
        now_ms: u64,
        obs: Observation,
        user_quit: bool,
    ) -> (Decision, Vec<EventKind>) {
        if user_quit {
            return (Decision::Interrupted, vec![EventKind::Interrupted]);
        }

        let mut events = Vec::new();

        if obs.present {
            if let Some(since) = self.absent_since.take() {
                if now_ms.saturating_sub(since) < self.grace_ms {
                    events.push(EventKind::Blip);
                }
            }
            if self.confirming_since.take().is_some() {
                events.push(EventKind::FalseDeath);
                self.emitted_confirming = false;
                self.emitted_dead = false;
            }
        }

        if obs.present && obs.audio_flowing {
            self.silent_since = None;
            if obs.percent == Some(0) && !self.emitted_empty {
                events.push(EventKind::ReportedEmptyStillPlaying);
                self.emitted_empty = true;
            }
            match (self.last_percent, obs.percent) {
                (Some(a), Some(b)) if a == b => {
                    let start = *self.percent_unchanged_since.get_or_insert(now_ms);
                    if now_ms.saturating_sub(start) >= self.stuck_ms && !self.emitted_stuck {
                        events.push(EventKind::PercentStuck);
                        self.emitted_stuck = true;
                    }
                }
                _ => {
                    self.percent_unchanged_since = obs.percent.map(|_| now_ms);
                    if obs.percent != Some(0) {
                        self.emitted_empty = false;
                    }
                    if self.last_percent != obs.percent {
                        self.emitted_stuck = false;
                    }
                }
            }
            self.last_percent = obs.percent;
            return (Decision::Live, events);
        }

        if !obs.present {
            return self.expire(now_ms, events, true);
        }

        self.expire(now_ms, events, false)
    }

    fn expire(
        &mut self,
        now_ms: u64,
        mut events: Vec<EventKind>,
        absent: bool,
    ) -> (Decision, Vec<EventKind>) {
        let since = if absent {
            *self.absent_since.get_or_insert(now_ms)
        } else {
            *self.silent_since.get_or_insert(now_ms)
        };
        if now_ms.saturating_sub(since) < self.grace_ms {
            return (Decision::Live, events);
        }
        let confirm = *self.confirming_since.get_or_insert(now_ms);
        if now_ms.saturating_sub(confirm) >= self.confirm_ms {
            if !self.emitted_dead {
                events.push(EventKind::Dead);
                self.emitted_dead = true;
            }
            return (Decision::Dead, events);
        }
        if !self.emitted_confirming {
            events.push(EventKind::Confirming);
            self.emitted_confirming = true;
        }
        (Decision::Confirming, events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn watch() -> DeathWatch {
        DeathWatch::new(30_000, 15_000)
    }

    fn playing(percent: u8) -> Observation {
        Observation {
            present: true,
            audio_flowing: true,
            percent: Some(percent),
        }
    }

    fn gone() -> Observation {
        Observation {
            present: false,
            audio_flowing: false,
            percent: None,
        }
    }

    #[test]
    fn zero_percent_while_playing_is_not_death() {
        let mut w = watch();
        let (d, ev) = w.tick(0, playing(0), false);
        assert_eq!(d, Decision::Live);
        assert_eq!(ev, vec![EventKind::ReportedEmptyStillPlaying]);
        let (d, ev) = w.tick(1_000, playing(0), false);
        assert_eq!(d, Decision::Live);
        assert!(ev.is_empty());
    }

    #[test]
    fn short_disconnect_is_a_blip() {
        let mut w = watch();
        w.tick(0, playing(50), false);
        let (d, _) = w.tick(1_000, gone(), false);
        assert_eq!(d, Decision::Live);
        let (d, ev) = w.tick(5_000, playing(50), false);
        assert_eq!(d, Decision::Live);
        assert_eq!(ev, vec![EventKind::Blip]);
    }

    #[test]
    fn gone_for_grace_then_confirm_is_dead() {
        let mut w = watch();
        w.tick(0, playing(12), false);
        let (d, _) = w.tick(1_000, gone(), false);
        assert_eq!(d, Decision::Live);
        let (d, ev) = w.tick(31_000, gone(), false);
        assert_eq!(d, Decision::Confirming);
        assert_eq!(ev, vec![EventKind::Confirming]);
        let (d, ev) = w.tick(46_000, gone(), false);
        assert_eq!(d, Decision::Dead);
        assert_eq!(ev, vec![EventKind::Dead]);
    }

    #[test]
    fn comes_back_during_confirm_is_false_death() {
        let mut w = watch();
        w.tick(0, playing(8), false);
        w.tick(1_000, gone(), false);
        let (d, _) = w.tick(32_000, gone(), false);
        assert_eq!(d, Decision::Confirming);
        let (d, ev) = w.tick(35_000, playing(8), false);
        assert_eq!(d, Decision::Live);
        assert_eq!(ev, vec![EventKind::FalseDeath]);
    }

    #[test]
    fn quit_interrupts() {
        let mut w = watch();
        let (d, ev) = w.tick(0, playing(80), true);
        assert_eq!(d, Decision::Interrupted);
        assert_eq!(ev, vec![EventKind::Interrupted]);
    }

    #[test]
    fn tags_are_stable_slugs() {
        assert_eq!(EventKind::ReportedEmptyStillPlaying.tag(), "empty_playing");
        assert_eq!(EventKind::PercentStuck.tag(), "percent_stuck");
        assert_eq!(EventKind::Blip.tag(), "blip");
        assert_eq!(EventKind::FalseDeath.tag(), "false_death");
        assert_eq!(EventKind::Confirming.tag(), "confirming");
        assert_eq!(EventKind::Dead.tag(), "dead");
        assert_eq!(EventKind::Interrupted.tag(), "interrupted");
    }

    #[test]
    fn stuck_percent_while_playing_is_an_event_not_a_stop() {
        let mut w = watch();
        w.tick(0, playing(40), false);
        let (d, ev) = w.tick(10 * 60 * 1000, playing(40), false);
        assert_eq!(d, Decision::Live);
        assert_eq!(ev, vec![EventKind::PercentStuck]);
    }
}

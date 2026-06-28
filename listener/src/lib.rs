//! reasoning-ting core: audio capture → Quindar tone detection → key injection.
//! Shared by the CLI (`reasoning-ting-listen`) and the menu-bar GUI (`reasoning-ting`).

pub mod audio;
pub mod detect;
pub mod focus;
pub mod icon;
pub mod inject;
pub mod prefs;

pub use detect::{Event, Params};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

/// Engine configuration (front-ends build this from CLI args / GUI prefs).
#[derive(Clone)]
pub struct Config {
    pub device: Option<String>,
    pub key: String,        // voice push-to-talk key (matches keybindings.json)
    pub submit_key: String, // tapped on the submit tone (Enter)
    pub params: Params,     // tone freqs + threshold + window
    pub max_hold_secs: f32,
    pub focus_guard: bool,
    pub focus_proc: String,
    pub dry_run: bool,
}

/// Engine status stream — front-ends render this (CLI meter/logs, GUI tray).
#[derive(Debug, Clone)]
pub enum Status {
    Listening { sample_rate: u32 },
    Reconnecting,
    Level {
        peak: f32,
        m_in: f32,
        m_out: f32,
        m_submit: f32,
        held: bool,
    },
    /// A detected event and whether we acted on it (false = focus-guard suppressed).
    Event { event: Event, acted: bool },
    Error(String),
}

/// Run the detect→inject engine until `stop` is set. Calls `on_status` with
/// progress. Blocks the calling thread (run it on its own thread from a GUI).
pub fn run(cfg: &Config, stop: &AtomicBool, mut on_status: impl FnMut(Status)) {
    let key = match inject::parse_key(&cfg.key) {
        Ok(k) => k,
        Err(e) => return on_status(Status::Error(format!("bad --key: {e}"))),
    };
    let submit_key = match inject::parse_key(&cfg.submit_key) {
        Ok(k) => k,
        Err(e) => return on_status(Status::Error(format!("bad --submit-key: {e}"))),
    };
    let guard = focus::make(!cfg.focus_guard, cfg.focus_proc.clone());
    let mut injector = if cfg.dry_run {
        None
    } else {
        match inject::Injector::new(key) {
            Ok(i) => Some(i),
            Err(e) => return on_status(Status::Error(format!("input init: {e}"))),
        }
    };

    while !stop.load(Ordering::Relaxed) {
        let cap = match audio::start(cfg.device.as_deref()) {
            Ok(c) => c,
            Err(e) => {
                on_status(Status::Error(format!("audio start: {e}")));
                std::thread::sleep(Duration::from_secs(2));
                continue;
            }
        };
        let sr = cap.sample_rate as f32;
        on_status(Status::Listening {
            sample_rate: cap.sample_rate,
        });
        let mut det = detect::Detector::new(sr, &cfg.params);
        let max_hold = (cfg.max_hold_secs * sr) as u64;
        let mut held_samples: u64 = 0;
        let mut stalls = 0;
        let meter_interval = (sr / 15.0) as usize;
        let mut meter_ctr = 0usize;
        let mut peak = 0f32;

        loop {
            if stop.load(Ordering::Relaxed) {
                if det.held() {
                    if let Some(i) = injector.as_mut() {
                        i.up();
                    }
                }
                return;
            }
            match cap.rx.recv_timeout(Duration::from_secs(3)) {
                Ok(block) => {
                    stalls = 0;
                    for &s in &block {
                        peak = peak.max(s.abs());
                        if det.held() {
                            held_samples += 1;
                            if held_samples >= max_hold {
                                if let Some(i) = injector.as_mut() {
                                    i.up();
                                }
                                det.set_held(false);
                                held_samples = 0;
                            }
                        }
                        if let Some(ev) = det.push(s) {
                            let acted = match ev {
                                Event::Intro => {
                                    if guard.allowed() {
                                        held_samples = 0;
                                        if let Some(i) = injector.as_mut() {
                                            i.down();
                                        }
                                        true
                                    } else {
                                        det.set_held(false);
                                        false
                                    }
                                }
                                Event::Outro => {
                                    if let Some(i) = injector.as_mut() {
                                        i.up();
                                    }
                                    true
                                }
                                Event::Submit => {
                                    if guard.allowed() {
                                        if let Some(i) = injector.as_mut() {
                                            i.tap(submit_key);
                                        }
                                        true
                                    } else {
                                        false
                                    }
                                }
                            };
                            on_status(Status::Event { event: ev, acted });
                        }
                        meter_ctr += 1;
                        if meter_ctr >= meter_interval {
                            meter_ctr = 0;
                            let (mi, mo, ms) = det.mags();
                            on_status(Status::Level {
                                peak,
                                m_in: mi,
                                m_out: mo,
                                m_submit: ms,
                                held: det.held(),
                            });
                            peak = 0.0;
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    stalls += 1;
                    if stalls >= 3 {
                        if det.held() {
                            if let Some(i) = injector.as_mut() {
                                i.up();
                            }
                            det.set_held(false);
                        }
                        on_status(Status::Reconnecting);
                        break;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    if det.held() {
                        if let Some(i) = injector.as_mut() {
                            i.up();
                        }
                        det.set_held(false);
                    }
                    on_status(Status::Reconnecting);
                    break;
                }
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

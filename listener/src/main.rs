//! some-ting-listen: detect the TING's Quindar tones and drive Claude voice.
//!
//! Squeeze the handle -> firmware emits 2525 Hz -> we hold the voice key down;
//! release -> 2475 Hz -> we release the key. Focus-guarded so it only fires
//! into a focused Claude window.

mod audio;
mod detect;
mod focus;
mod inject;

use anyhow::Result;
use clap::Parser;
use detect::{Detector, Event, Params};
use std::io::{IsTerminal, Write};
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(about = "Detect TING Quindar tones -> Claude voice push-to-talk")]
struct Args {
    /// Validate against a WAV file instead of live audio.
    #[arg(long)]
    wav: Option<String>,
    /// Input device name substring (default: system default input).
    #[arg(long)]
    device: Option<String>,
    /// List input devices and exit.
    #[arg(long)]
    list_devices: bool,
    /// Key bound to Claude's /voice (must match ~/.claude/keybindings.json).
    /// F12 — F13+ aren't physical keys most terminals transmit.
    #[arg(long, default_value = "f12")]
    key: String,
    /// Detection magnitude threshold.
    #[arg(long, default_value_t = 0.008)]
    threshold: f32,
    /// Safety: force key-up after this many seconds held.
    #[arg(long, default_value_t = 30.0)]
    max_hold: f32,
    /// Inject regardless of which window is focused.
    #[arg(long)]
    no_focus_guard: bool,
    /// Process name to look for in the focused window's subtree (Linux).
    #[arg(long, default_value = "claude")]
    focus_proc: String,
    /// Detect only; print events, send no keystrokes.
    #[arg(long)]
    dry_run: bool,
    /// Validate the Claude keybinding: after a countdown, hold the key ~2s
    /// (simulates one squeeze) so you can watch voice trigger. No audio/TING.
    #[arg(long)]
    test_key: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.list_devices {
        return audio::list_devices();
    }
    if args.test_key {
        return run_test_key(&args);
    }
    let params = Params {
        threshold: args.threshold,
        ..Default::default()
    };
    match &args.wav {
        Some(path) => run_wav(path, &params),
        None => run_live(&args, params),
    }
}

/// Offline: run the detector over a WAV (real-capture validation).
fn run_wav(path: &str, params: &Params) -> Result<()> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let sr = spec.sample_rate as f32;
    let ch = spec.channels as usize;
    println!("wav: {path}  {sr} Hz  {ch}ch  fmt={:?}", spec.sample_format);

    // read channel 0 as f32
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .filter_map(|s| s.ok())
            .enumerate()
            .filter(|(i, _)| i % ch == 0)
            .map(|(_, s)| s)
            .collect(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .filter_map(|s| s.ok())
                .enumerate()
                .filter(|(i, _)| i % ch == 0)
                .map(|(_, s)| s as f32 * scale)
                .collect()
        }
    };

    let mut det = Detector::new(sr, params);
    let mut n = 0;
    for (i, &s) in samples.iter().enumerate() {
        if let Some(ev) = det.push(s) {
            let t = i as f32 / sr;
            println!("  t={t:6.2}s  {}", label(ev));
            n += 1;
        }
    }
    println!("{n} events");
    Ok(())
}

fn run_live(args: &Args, params: Params) -> Result<()> {
    let key = inject::parse_key(&args.key)?;
    let guard = focus::make(args.no_focus_guard, args.focus_proc.clone());
    let mut injector = if args.dry_run {
        None
    } else {
        Some(inject::Injector::new(key)?)
    };
    // Live input meter only when stderr is an interactive TTY (not a service/pipe).
    let meter = std::io::stderr().is_terminal();
    let threshold = params.threshold;

    // Reconnect loop: a live mic streams continuously (even silence), so a
    // sustained read timeout means the device died -> rebuild the stream.
    loop {
        let cap = match audio::start(args.device.as_deref()) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("audio start failed: {e}; retrying in 2s");
                std::thread::sleep(Duration::from_secs(2));
                continue;
            }
        };
        let sr = cap.sample_rate as f32;
        let mut det = Detector::new(sr, &params);
        let max_hold_samples = (args.max_hold * sr) as u64;
        let mut held_samples: u64 = 0;
        let mut stalls = 0;
        let meter_interval = (sr / 15.0) as usize; // ~66 ms refresh
        let mut meter_ctr = 0usize;
        let mut peak = 0f32;
        eprintln!(
            "listening @ {sr} Hz  key={}  focus_guard={}  dry_run={}{}",
            args.key,
            !args.no_focus_guard,
            args.dry_run,
            if meter { "  (live meter below)" } else { "" }
        );

        loop {
            match cap.rx.recv_timeout(Duration::from_secs(3)) {
                Ok(block) => {
                    stalls = 0;
                    for &s in &block {
                        peak = peak.max(s.abs());
                        if det.held() {
                            held_samples += 1;
                            if held_samples >= max_hold_samples {
                                if let Some(inj) = injector.as_mut() {
                                    inj.up();
                                }
                                det.set_held(false);
                                held_samples = 0;
                                log_event(meter, &format!("(safety key-up after {:.0}s held)", args.max_hold));
                            }
                        }
                        if let Some(ev) = det.push(s) {
                            match ev {
                                Event::Intro => {
                                    if !guard.allowed() {
                                        det.set_held(false);
                                        log_event(meter, "INTRO  (ignored: target window not focused)");
                                        continue;
                                    }
                                    held_samples = 0;
                                    if let Some(inj) = injector.as_mut() {
                                        inj.down();
                                    }
                                    log_event(meter, &format!("INTRO/2525 -> START (keydown {})", args.key));
                                }
                                Event::Outro => {
                                    if let Some(inj) = injector.as_mut() {
                                        inj.up();
                                    }
                                    log_event(meter, &format!("OUTRO/2475 -> SEND  (keyup {})", args.key));
                                }
                            }
                        }
                        meter_ctr += 1;
                        if meter && meter_ctr >= meter_interval {
                            meter_ctr = 0;
                            let (mi, mo) = det.mags();
                            draw_meter(peak, mi, mo, threshold, det.held());
                            peak = 0.0;
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    stalls += 1;
                    if stalls >= 3 {
                        eprintln!("\naudio stalled (~9s no input); reconnecting...");
                        if det.held() {
                            if let Some(inj) = injector.as_mut() {
                                inj.up();
                            }
                        }
                        break;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    eprintln!("\naudio stream ended; reconnecting...");
                    if det.held() {
                        if let Some(inj) = injector.as_mut() {
                            inj.up();
                        }
                    }
                    break;
                }
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// Print an event line; when the live meter is on, clear the meter line first
/// so the log doesn't get clobbered by the in-place bar.
fn log_event(meter: bool, msg: &str) {
    if meter {
        eprint!("\r\x1b[K");
        eprintln!("{msg}");
    } else {
        println!("{msg}");
    }
}

/// In-place input meter: level bar + live 2525/2475 magnitudes vs threshold.
fn draw_meter(peak: f32, m_in: f32, m_out: f32, thr: f32, held: bool) {
    let db = 20.0 * (peak + 1e-9).log10();
    let filled = (((db + 60.0) / 60.0).clamp(0.0, 1.0) * 24.0) as usize;
    let bar: String = (0..24)
        .map(|i| if i < filled { '#' } else { ' ' })
        .collect();
    let hot = if m_in.max(m_out) > thr {
        if m_in >= m_out {
            "2525!"
        } else {
            "2475!"
        }
    } else {
        "     "
    };
    let state = if held { "KEYED" } else { "     " };
    eprint!(
        "\r\x1b[Kin |{bar}| {db:>4.0}dB  2525={m_in:.4} 2475={m_out:.4} thr={thr:.3} {hot} {state}"
    );
    let _ = std::io::stderr().flush();
}

/// Validate the Claude keybinding without the TING: hold the key ~2s after a
/// countdown so you can watch voice record + send in a focused Claude window.
fn run_test_key(args: &Args) -> Result<()> {
    let key = inject::parse_key(&args.key)?;
    let mut inj = inject::Injector::new(key)?;
    eprintln!(
        "TEST: focus your Claude window. Holding '{}' for 2s in:",
        args.key
    );
    for n in (1..=5).rev() {
        eprintln!("  {n}...");
        std::thread::sleep(Duration::from_secs(1));
    }
    eprintln!("keydown {}", args.key);
    inj.down();
    std::thread::sleep(Duration::from_secs(2));
    inj.up();
    eprintln!("keyup {} — did Claude record then send?", args.key);
    Ok(())
}

fn label(ev: Event) -> &'static str {
    match ev {
        Event::Intro => "INTRO/2525 -> START",
        Event::Outro => "OUTRO/2475 -> SEND",
    }
}

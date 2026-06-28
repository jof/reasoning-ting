//! reasoning-ting-listen: detect the TING's Quindar tones and drive Claude voice.
//!
//! Squeeze the handle -> firmware emits 2525 Hz -> we hold the voice key down;
//! release -> 2475 Hz -> we release the key. Focus-guarded so it only fires
//! into a focused Claude window.

use anyhow::Result;
use clap::Parser;
use reasoning_ting::detect::Detector;
use reasoning_ting::{audio, inject, Config, Event, Params, Status};
use std::io::{IsTerminal, Write};
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
    /// Safety: force key-up after this many seconds held (detection is reliable,
    /// so this is just a backstop against a missed release tone).
    #[arg(long, default_value_t = 600.0)]
    max_hold: f32,
    /// Key tapped on the submit tone (must match a Claude binding; default Enter).
    #[arg(long, default_value = "enter")]
    submit_key: String,
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
    /// Live spectrum / dominant-frequency view (diagnostic; interactive TTY).
    #[arg(long)]
    spectrum: bool,
    /// Record live audio to a WAV (via the daemon's own capture pipeline) for
    /// offline analysis / tuning. Squeeze a few times while it records.
    #[arg(long, value_name = "PATH")]
    record: Option<String>,
    /// Seconds to record with --record.
    #[arg(long, default_value_t = 20.0)]
    seconds: f32,
    /// Intro (press) tone frequency, Hz.
    #[arg(long, default_value_t = 2525.0)]
    f_intro: f32,
    /// Outro (release) tone frequency, Hz.
    #[arg(long, default_value_t = 2475.0)]
    f_outro: f32,
    /// Submit tone frequency, Hz (other button -> Enter).
    #[arg(long, default_value_t = 3000.0)]
    f_submit: f32,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.list_devices {
        return audio::list_devices();
    }
    if args.test_key {
        return run_test_key(&args);
    }
    if args.spectrum {
        return run_spectrum(&args);
    }
    if let Some(path) = args.record.clone() {
        return run_record(&args, &path, args.seconds);
    }
    let params = Params {
        threshold: args.threshold,
        f_intro: args.f_intro,
        f_outro: args.f_outro,
        f_submit: args.f_submit,
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
    let meter = std::io::stderr().is_terminal();
    let threshold = params.threshold;
    let key = args.key.clone();
    let submit = args.submit_key.clone();
    let f_submit = args.f_submit;
    let cfg = Config {
        device: args.device.clone(),
        key: args.key.clone(),
        submit_key: args.submit_key.clone(),
        params,
        max_hold_secs: args.max_hold,
        focus_guard: !args.no_focus_guard,
        focus_proc: args.focus_proc.clone(),
        dry_run: args.dry_run,
    };
    let stop = std::sync::atomic::AtomicBool::new(false);
    reasoning_ting::run(&cfg, &stop, |st| match st {
        Status::Listening { sample_rate } => eprintln!(
            "listening @ {sample_rate} Hz  key={key}  focus_guard={}  dry_run={}{}",
            cfg.focus_guard,
            cfg.dry_run,
            if meter { "  (live meter below)" } else { "" }
        ),
        Status::Reconnecting => eprintln!("\nreconnecting..."),
        Status::Level {
            peak,
            m_in,
            m_out,
            m_submit,
            held,
        } => {
            if meter {
                draw_meter(peak, m_in, m_out, m_submit, threshold, held);
            }
        }
        Status::Event { event, acted } => {
            log_event(meter, &describe_event(event, acted, &key, &submit, f_submit))
        }
        Status::Error(e) => eprintln!("\nerror: {e}"),
    });
    Ok(())
}

/// Format a detection event for the CLI log.
fn describe_event(ev: Event, acted: bool, key: &str, submit: &str, f_submit: f32) -> String {
    match ev {
        Event::Intro if acted => format!("INTRO/2525 -> START (keydown {key})"),
        Event::Intro => "INTRO  (ignored: target window not focused)".into(),
        Event::Outro => format!("OUTRO/2475 -> SEND  (keyup {key})"),
        Event::Submit if acted => format!("SUBMIT/{f_submit:.0} -> ENTER (tap {submit})"),
        Event::Submit => "SUBMIT  (ignored: target window not focused)".into(),
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

/// In-place input meter: level bar + live intro/outro/submit magnitudes vs threshold.
fn draw_meter(peak: f32, m_in: f32, m_out: f32, m_submit: f32, thr: f32, held: bool) {
    let db = 20.0 * (peak + 1e-9).log10();
    let filled = (((db + 60.0) / 60.0).clamp(0.0, 1.0) * 18.0) as usize;
    let bar: String = (0..18)
        .map(|i| if i < filled { '#' } else { ' ' })
        .collect();
    let top = m_in.max(m_out).max(m_submit);
    let hot = if top > thr {
        if top == m_in {
            "IN! "
        } else if top == m_out {
            "OUT!"
        } else {
            "SUB!"
        }
    } else {
        "    "
    };
    let state = if held { "KEYED" } else { "     " };
    eprint!(
        "\r\x1b[Kin |{bar}| {db:>4.0}dB in={m_in:.4} out={m_out:.4} sub={m_submit:.4} thr={thr:.3} {hot} {state}"
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

/// Live spectrum + dominant-frequency view (diagnostic). Shows a 0–5 kHz bar,
/// the peak frequency, and the exact 2525/2475 magnitudes so you can see where
/// the tone actually lands and how strong it is.
fn run_spectrum(args: &Args) -> Result<()> {
    use rustfft::num_complex::Complex;
    if !std::io::stderr().is_terminal() {
        anyhow::bail!("--spectrum needs an interactive terminal");
    }
    let cap = audio::start(args.device.as_deref())?;
    let sr = cap.sample_rate as f32;
    let n = 4096usize;
    let fft = rustfft::FftPlanner::new().plan_fft_forward(n);
    let hann: Vec<f32> = (0..n)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (n as f32 - 1.0)).cos())
        .collect();
    let bin_at = |hz: f32| ((hz * n as f32 / sr).round() as usize).min(n / 2 - 1);
    let mut ring = vec![0f32; n];
    let (mut pos, mut filled, mut ctr) = (0usize, 0usize, 0usize);
    let interval = (sr / 15.0) as usize;
    eprintln!("spectrum @ {sr} Hz — squeeze to see the tone; watch 2525/2475. Ctrl-C to quit.");
    for block in cap.rx {
        for &s in &block {
            ring[pos] = s;
            pos = (pos + 1) % n;
            if filled < n {
                filled += 1;
            }
            ctr += 1;
            if ctr >= interval && filled == n {
                ctr = 0;
                let mut buf: Vec<Complex<f32>> = (0..n)
                    .map(|k| Complex {
                        re: ring[(pos + k) % n] * hann[k],
                        im: 0.0,
                    })
                    .collect();
                fft.process(&mut buf);
                let half = n / 2;
                let mag: Vec<f32> = buf[..half].iter().map(|c| c.norm() / n as f32).collect();
                let (lo, hi) = (bin_at(50.0), bin_at(8000.0));
                let (mut bi, mut bm) = (lo, 0f32);
                for (i, &m) in mag.iter().enumerate().take(hi).skip(lo) {
                    if m > bm {
                        bm = m;
                        bi = i;
                    }
                }
                let peak_hz = bi as f32 * sr / n as f32;
                let bar = spectrum_bar(&mag, sr, n, 56, 5000.0);
                eprint!(
                    "\r\x1b[K0|{bar}|5k peak={peak_hz:>5.0}Hz({bm:.3}) 2525={:.4} 2475={:.4}",
                    mag[bin_at(2525.0)],
                    mag[bin_at(2475.0)]
                );
                let _ = std::io::stderr().flush();
            }
        }
    }
    Ok(())
}

/// Record live audio (the daemon's own cpal pipeline) to a 16-bit mono WAV,
/// so analysis sees exactly what the detector sees.
fn run_record(args: &Args, path: &str, secs: f32) -> Result<()> {
    let cap = audio::start(args.device.as_deref())?;
    let sr = cap.sample_rate;
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: sr,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    let total = (secs * sr as f32) as u64;
    let tty = std::io::stderr().is_terminal();
    eprintln!("recording {secs:.0}s to {path} @ {sr} Hz — squeeze 3-4 times (engage, say a word, release)...");
    let mut n = 0u64;
    'outer: for block in cap.rx {
        for &s in &block {
            w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)?;
            n += 1;
            if n >= total {
                break 'outer;
            }
        }
        if tty {
            eprint!("\r{:>4.1}s / {secs:.0}s", n as f32 / sr as f32);
            let _ = std::io::stderr().flush();
        }
    }
    w.finalize()?;
    eprintln!("\nsaved {path}");
    Ok(())
}

/// Render a one-line spectrum bar over [0, fmax] Hz using block glyphs.
fn spectrum_bar(mag: &[f32], sr: f32, n: usize, cols: usize, fmax: f32) -> String {
    const BLOCKS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let mut out = String::with_capacity(cols);
    for c in 0..cols {
        let f0 = c as f32 / cols as f32 * fmax;
        let f1 = (c + 1) as f32 / cols as f32 * fmax;
        let b0 = ((f0 * n as f32 / sr) as usize).min(n / 2 - 1);
        let b1 = ((f1 * n as f32 / sr) as usize).max(b0 + 1).min(n / 2);
        let m = mag[b0..b1].iter().cloned().fold(0f32, f32::max);
        let db = 20.0 * (m + 1e-9).log10();
        let lvl = (((db + 60.0) / 60.0).clamp(0.0, 1.0) * 8.0) as usize;
        out.push(BLOCKS[lvl.min(8)]);
    }
    out
}

fn label(ev: Event) -> &'static str {
    match ev {
        Event::Intro => "INTRO/2525 -> START",
        Event::Outro => "OUTRO/2475 -> SEND",
        Event::Submit => "SUBMIT/3000 -> ENTER",
    }
}

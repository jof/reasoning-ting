//! some-ting menu-bar app: a status-tray icon that runs the detection engine
//! (shared `some_ting` lib) and exposes controls. Build:
//!   cargo build --release --features gui --bin some-ting
//! (Linux needs gtk3 + libayatana-appindicator dev packages; macOS/Windows don't.)
//!
//! Run with `--dry-run` to detect + show status WITHOUT sending keystrokes.
//!
//! Menu: status · Pause/Resume · Input device · Sensitivity · Focus guard ·
//! Write Claude keybinding · Quit. Device/sensitivity/focus-guard choices are
//! persisted (see `some_ting::prefs`). TODO: Setup wizard, launch-at-login.

use some_ting::icon::{self, IconState};
use some_ting::prefs::Prefs;
use some_ting::{audio, Config, Event, Params, Status};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use tao::event_loop::{ControlFlow, EventLoop};
use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    Icon, TrayIcon, TrayIconBuilder,
};

const SENS: [(&str, f32); 3] = [
    ("High (0.002)", 0.002),
    ("Medium (0.008)", 0.008),
    ("Low (0.020)", 0.020),
];

fn make_icon(state: IconState) -> Icon {
    Icon::from_rgba(icon::rgba(state), icon::SIZE, icon::SIZE).expect("icon")
}

fn state_label(state: IconState) -> &'static str {
    match state {
        IconState::Idle => "idle",
        IconState::Listening => "listening",
        IconState::Keyed => "KEYED (voice held)",
        IconState::Paused => "paused",
    }
}

/// Set the tray icon only when the state actually changes (avoid churn — the
/// engine emits Level ~15×/s). Updates both the icon pixmap and the tooltip:
/// some XEmbed bridges (snixembed → i3bar) are slow to redraw a changed pixmap,
/// so the tooltip gives a second, reliable feedback channel.
fn set_state(tray: &Option<TrayIcon>, cur: &mut IconState, want: IconState) {
    if *cur != want {
        *cur = want;
        if let Some(t) = tray {
            let _ = t.set_icon(Some(make_icon(want)));
            let _ = t.set_tooltip(Some(format!("some-ting — {}", state_label(want))));
        }
    }
}

/// One consistent log line per user-visible event:
///   `some-ting │ <action>  <detail>`
/// Front of every normal-usage line so squeeze/release/submit read as a
/// uniform stream (see the match arms in main()).
fn log_line(action: &str, detail: &str) {
    if detail.is_empty() {
        eprintln!("some-ting │ {action}");
    } else {
        eprintln!("some-ting │ {action:<9} {detail}");
    }
}

fn base_config(dry_run: bool, prefs: &Prefs) -> Config {
    Config {
        device: prefs.device.clone(),
        key: "f12".into(),
        submit_key: "enter".into(),
        params: Params {
            threshold: prefs.threshold,
            ..Params::default()
        },
        max_hold_secs: 600.0,
        focus_guard: prefs.focus_guard,
        focus_proc: "claude".into(),
        dry_run,
    }
}

/// Snapshot the live config back into persisted prefs.
fn save_prefs(cfg: &Config) {
    Prefs {
        device: cfg.device.clone(),
        threshold: cfg.params.threshold,
        focus_guard: cfg.focus_guard,
    }
    .save();
}

fn spawn_engine(cfg: &Config, tx: mpsc::Sender<Status>) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let s = stop.clone();
    let cfg = cfg.clone();
    std::thread::spawn(move || {
        some_ting::run(&cfg, &s, move |st| {
            let _ = tx.send(st);
        });
    });
    stop
}

fn restart(slot: &mut Option<Arc<AtomicBool>>, cfg: &Config, tx: &mpsc::Sender<Status>, paused: bool) {
    if let Some(old) = slot.take() {
        old.store(true, Ordering::Relaxed);
    }
    if !paused {
        *slot = Some(spawn_engine(cfg, tx.clone()));
    }
}

/// Write the Claude voice keybinding if absent (never clobber an existing file).
fn write_keybinding() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    if home.is_empty() {
        return "can't find $HOME".into();
    }
    let path = std::path::PathBuf::from(&home).join(".claude/keybindings.json");
    if path.exists() {
        return "keybindings.json exists — add f12 → voice:pushToTalk by hand".into();
    }
    let content = r#"{
  "$schema": "https://www.schemastore.org/claude-code-keybindings.json",
  "bindings": [
    { "context": "Chat", "bindings": { "f12": "voice:pushToTalk" } }
  ]
}
"#;
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    match std::fs::write(&path, content) {
        Ok(_) => "wrote f12 → voice:pushToTalk (restart Claude Code)".into(),
        Err(e) => format!("write failed: {e}"),
    }
}

fn main() {
    let dry_run = std::env::args().any(|a| a == "--dry-run");
    let no_focus_guard = std::env::args().any(|a| a == "--no-focus-guard");

    // Persisted prefs are the baseline; --no-focus-guard still forces it off.
    let mut prefs = Prefs::load();
    if no_focus_guard {
        prefs.focus_guard = false;
    }

    let (tx, rx) = mpsc::channel::<Status>();
    let mut cfg = base_config(dry_run, &prefs);
    log_line(
        "start",
        &format!(
            "key={} submit={} focus-guard={}{}",
            cfg.key,
            cfg.submit_key,
            if cfg.focus_guard { "on" } else { "off" },
            if cfg.dry_run { " dry-run" } else { "" },
        ),
    );
    let mut engine: Option<Arc<AtomicBool>> = Some(spawn_engine(&cfg, tx.clone()));
    let mut paused = false;

    let event_loop = EventLoop::new();

    let menu = Menu::new();
    let status = MenuItem::new("starting…", false, None);
    let pause = MenuItem::new("Pause", true, None);

    let dev_menu = Submenu::new("Input device", true);
    let mut dev_items: Vec<(CheckMenuItem, Option<String>)> = Vec::new();
    let def = CheckMenuItem::new("System default", true, cfg.device.is_none(), None);
    dev_menu.append(&def).unwrap();
    dev_items.push((def, None));
    for name in audio::input_device_names() {
        let checked = cfg.device.as_deref() == Some(name.as_str());
        let it = CheckMenuItem::new(&name, true, checked, None);
        dev_menu.append(&it).unwrap();
        dev_items.push((it, Some(name)));
    }

    let sens_menu = Submenu::new("Sensitivity", true);
    let mut sens_items: Vec<(CheckMenuItem, f32)> = Vec::new();
    for (label, thr) in SENS {
        let checked = (cfg.params.threshold - thr).abs() < 1e-6;
        let it = CheckMenuItem::new(label, true, checked, None);
        sens_menu.append(&it).unwrap();
        sens_items.push((it, thr));
    }

    // Focus guard: when on, only inject while a Claude window is focused.
    let focus_guard = CheckMenuItem::new(
        "Focus guard (Claude windows only)",
        true,
        cfg.focus_guard,
        None,
    );

    let keybind = MenuItem::new("Write Claude keybinding (f12)", true, None);
    let quit = MenuItem::new("Quit some-ting", true, None);

    menu.append(&status).unwrap();
    menu.append(&PredefinedMenuItem::separator()).unwrap();
    menu.append(&pause).unwrap();
    menu.append(&dev_menu).unwrap();
    menu.append(&sens_menu).unwrap();
    menu.append(&focus_guard).unwrap();
    menu.append(&PredefinedMenuItem::separator()).unwrap();
    menu.append(&keybind).unwrap();
    menu.append(&PredefinedMenuItem::separator()).unwrap();
    menu.append(&quit).unwrap();

    let menu_events = MenuEvent::receiver();
    let mut tray: Option<TrayIcon> = None;
    let mut tray_tried = false;
    let mut icon_state = IconState::Idle;
    let mut refresh_tick: u32 = 0;

    event_loop.run(move |_event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100));

        // Re-assert the current icon a few times a second, not just on change.
        // Some XEmbed/SNI hosts (snixembed → i3bar) silently drop set_icon
        // calls, which would otherwise leave the tray stuck on a stale state
        // until the next transition ("only half the transitions show"). The
        // loop wakes ~10×/s (WaitUntil above); re-push every 4th wake (~2.5/s).
        refresh_tick = refresh_tick.wrapping_add(1);
        if refresh_tick.is_multiple_of(4) {
            if let Some(t) = &tray {
                let _ = t.set_icon(Some(make_icon(icon_state)));
            }
        }

        if !tray_tried {
            tray_tried = true;
            match TrayIconBuilder::new()
                .with_menu(Box::new(menu.clone()))
                .with_tooltip("some-ting — TING push-to-talk")
                .with_icon(make_icon(IconState::Idle))
                .build()
            {
                Ok(t) => tray = Some(t),
                Err(e) => eprintln!("tray unavailable ({e}); running headless (no SNI host?)"),
            }
        }

        while let Ok(st) = rx.try_recv() {
            match st {
                Status::Listening { sample_rate } => {
                    let dev = cfg.device.as_deref().unwrap_or("default");
                    log_line("listening", &format!("{dev} @ {sample_rate} Hz · standing by"));
                    status.set_text(format!("● listening @ {sample_rate} Hz"));
                    set_state(&tray, &mut icon_state, IconState::Listening);
                }
                Status::Reconnecting => {
                    log_line("reconnect", "audio dropped, retrying…");
                    status.set_text("… reconnecting");
                    set_state(&tray, &mut icon_state, IconState::Idle);
                }
                Status::Event { event, acted } => {
                    // One uniform line per event. acted=false = focus guard
                    // suppressed it (the confusing "nothing happened" case).
                    let suppressed = "ignored — no Claude window focused";
                    let (action, detail, short) = match (event, acted) {
                        (Event::Intro, true) => {
                            ("squeeze", format!("voice key down ({})", cfg.key), "voice ON")
                        }
                        (Event::Intro, false) => ("squeeze", suppressed.into(), "ignored"),
                        (Event::Outro, _) => {
                            ("release", format!("voice key up ({})", cfg.key), "voice OFF")
                        }
                        (Event::Submit, true) => {
                            ("submit", format!("{} tapped", cfg.submit_key), "submit")
                        }
                        (Event::Submit, false) => ("submit", suppressed.into(), "ignored"),
                    };
                    log_line(action, &detail);
                    status.set_text(format!("{action} · {short}"));
                    // Drive the tray icon straight off the edge events too, not
                    // just the ~15 Hz Level stream — slow XEmbed bridges
                    // (snixembed) redraw late, so kicking it at the exact
                    // squeeze/release moment makes the state feel responsive.
                    match (event, acted) {
                        (Event::Intro, true) => {
                            set_state(&tray, &mut icon_state, IconState::Keyed)
                        }
                        (Event::Outro, _) => {
                            set_state(&tray, &mut icon_state, IconState::Listening)
                        }
                        _ => {}
                    }
                }
                Status::Level { held, .. } => {
                    set_state(
                        &tray,
                        &mut icon_state,
                        if held { IconState::Keyed } else { IconState::Listening },
                    );
                }
                Status::Error(e) => {
                    log_line("error", &e);
                    status.set_text(format!("error: {e}"));
                    set_state(&tray, &mut icon_state, IconState::Idle);
                }
            }
        }

        if let Ok(ev) = menu_events.try_recv() {
            let id = ev.id;
            if id == quit.id() {
                if let Some(s) = &engine {
                    s.store(true, Ordering::Relaxed);
                }
                *control_flow = ControlFlow::Exit;
            } else if id == pause.id() {
                paused = !paused;
                pause.set_text(if paused { "Resume" } else { "Pause" });
                if paused {
                    if let Some(s) = engine.take() {
                        s.store(true, Ordering::Relaxed);
                    }
                    log_line("paused", "");
                    status.set_text("paused");
                    set_state(&tray, &mut icon_state, IconState::Paused);
                } else {
                    log_line("resumed", "");
                    engine = Some(spawn_engine(&cfg, tx.clone()));
                }
            } else if id == focus_guard.id() {
                cfg.focus_guard = !cfg.focus_guard;
                focus_guard.set_checked(cfg.focus_guard);
                log_line("focus-guard", if cfg.focus_guard { "on — Claude windows only" } else { "off — inject anywhere" });
                restart(&mut engine, &cfg, &tx, paused);
                save_prefs(&cfg);
            } else if id == keybind.id() {
                status.set_text(write_keybinding());
            } else {
                for (it, dev) in &dev_items {
                    if id == it.id() {
                        cfg.device = dev.clone();
                        for (o, _) in &dev_items {
                            o.set_checked(false);
                        }
                        it.set_checked(true);
                        restart(&mut engine, &cfg, &tx, paused);
                        save_prefs(&cfg);
                    }
                }
                for (it, thr) in &sens_items {
                    if id == it.id() {
                        cfg.params.threshold = *thr;
                        for (o, _) in &sens_items {
                            o.set_checked(false);
                        }
                        it.set_checked(true);
                        restart(&mut engine, &cfg, &tx, paused);
                        save_prefs(&cfg);
                    }
                }
            }
        }
    });
}

//! some-ting menu-bar app: a status-tray icon that runs the detection engine
//! (shared `some_ting` lib) and exposes controls. Build:
//!   cargo build --release --features gui --bin some-ting
//! (Linux needs gtk3 + libayatana-appindicator dev packages; macOS/Windows don't.)
//!
//! Run with `--dry-run` to detect + show status WITHOUT sending keystrokes.
//!
//! Menu: status · Pause/Resume · Input device · Sensitivity · Write Claude
//! keybinding · Quit. TODO: Setup wizard, persisted prefs, launch-at-login.

use some_ting::icon::{self, IconState};
use some_ting::{audio, Config, Params, Status};
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

/// Set the tray icon only when the state actually changes (avoid churn — the
/// engine emits Level ~15×/s).
fn set_state(tray: &Option<TrayIcon>, cur: &mut IconState, want: IconState) {
    if *cur != want {
        *cur = want;
        if let Some(t) = tray {
            let _ = t.set_icon(Some(make_icon(want)));
        }
    }
}

fn base_config(dry_run: bool) -> Config {
    Config {
        device: None,
        key: "f12".into(),
        submit_key: "enter".into(),
        params: Params::default(),
        max_hold_secs: 600.0,
        focus_guard: true,
        focus_proc: "claude".into(),
        dry_run,
    }
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

    let (tx, rx) = mpsc::channel::<Status>();
    let mut cfg = base_config(dry_run);
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

    let keybind = MenuItem::new("Write Claude keybinding (f12)", true, None);
    let quit = MenuItem::new("Quit some-ting", true, None);

    menu.append(&status).unwrap();
    menu.append(&PredefinedMenuItem::separator()).unwrap();
    menu.append(&pause).unwrap();
    menu.append(&dev_menu).unwrap();
    menu.append(&sens_menu).unwrap();
    menu.append(&PredefinedMenuItem::separator()).unwrap();
    menu.append(&keybind).unwrap();
    menu.append(&PredefinedMenuItem::separator()).unwrap();
    menu.append(&quit).unwrap();

    let menu_events = MenuEvent::receiver();
    let mut tray: Option<TrayIcon> = None;
    let mut tray_tried = false;
    let mut icon_state = IconState::Idle;

    event_loop.run(move |_event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100));

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
                    let _ = status.set_text(format!("● listening @ {sample_rate} Hz"));
                    set_state(&tray, &mut icon_state, IconState::Listening);
                }
                Status::Reconnecting => {
                    let _ = status.set_text("… reconnecting");
                    set_state(&tray, &mut icon_state, IconState::Idle);
                }
                Status::Event { event, acted } => {
                    let _ = status.set_text(format!(
                        "{event:?}{}",
                        if acted { "" } else { " (ignored)" }
                    ));
                }
                Status::Level { held, .. } => {
                    set_state(
                        &tray,
                        &mut icon_state,
                        if held { IconState::Keyed } else { IconState::Listening },
                    );
                }
                Status::Error(e) => {
                    let _ = status.set_text(format!("error: {e}"));
                    set_state(&tray, &mut icon_state, IconState::Idle);
                }
            }
        }

        if let Ok(ev) = menu_events.try_recv() {
            let id = ev.id;
            if &id == quit.id() {
                if let Some(s) = &engine {
                    s.store(true, Ordering::Relaxed);
                }
                *control_flow = ControlFlow::Exit;
            } else if &id == pause.id() {
                paused = !paused;
                let _ = pause.set_text(if paused { "Resume" } else { "Pause" });
                if paused {
                    if let Some(s) = engine.take() {
                        s.store(true, Ordering::Relaxed);
                    }
                    let _ = status.set_text("paused");
                    set_state(&tray, &mut icon_state, IconState::Paused);
                } else {
                    engine = Some(spawn_engine(&cfg, tx.clone()));
                }
            } else if &id == keybind.id() {
                let _ = status.set_text(write_keybinding());
            } else {
                for (it, dev) in &dev_items {
                    if &id == it.id() {
                        cfg.device = dev.clone();
                        for (o, _) in &dev_items {
                            o.set_checked(false);
                        }
                        it.set_checked(true);
                        restart(&mut engine, &cfg, &tx, paused);
                    }
                }
                for (it, thr) in &sens_items {
                    if &id == it.id() {
                        cfg.params.threshold = *thr;
                        for (o, _) in &sens_items {
                            o.set_checked(false);
                        }
                        it.set_checked(true);
                        restart(&mut engine, &cfg, &tx, paused);
                    }
                }
            }
        }
    });
}

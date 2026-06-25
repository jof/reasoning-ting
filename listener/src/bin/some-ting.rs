//! some-ting menu-bar app: a status-tray icon that runs the detection engine
//! (shared `some_ting` lib) on a worker thread and shows live status.
//!
//! Build: `cargo build --release --features gui --bin some-ting`
//! (Linux needs gtk3 + libayatana-appindicator dev packages; macOS/Windows don't.)
//!
//! Scaffold: status line + colored icon (idle/listening/keyed) + Quit. Next:
//! device picker, sensitivity, pause/resume, "Setup…" (permissions + keybinding),
//! launch-at-login, and the macOS .app bundle.

use some_ting::{Config, Params, Status};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use tao::event_loop::{ControlFlow, EventLoop};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIconBuilder,
};

fn solid_icon(rgb: [u8; 3]) -> Icon {
    let (w, h) = (18u32, 18u32);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for _ in 0..w * h {
        rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
    }
    Icon::from_rgba(rgba, w, h).expect("icon")
}

fn default_config() -> Config {
    Config {
        device: None,
        key: "f12".into(),
        submit_key: "enter".into(),
        params: Params::default(),
        max_hold_secs: 600.0,
        focus_guard: true,
        focus_proc: "claude".into(),
        dry_run: false,
    }
}

fn main() {
    // Engine on a worker thread; status flows back over a channel.
    let (tx, rx) = mpsc::channel::<Status>();
    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = stop.clone();
        std::thread::spawn(move || {
            some_ting::run(&default_config(), &stop, move |st| {
                let _ = tx.send(st);
            });
        });
    }

    let event_loop = EventLoop::new();
    let menu = Menu::new();
    let status = MenuItem::new("starting…", false, None);
    let quit = MenuItem::new("Quit some-ting", true, None);
    menu.append(&status).unwrap();
    menu.append(&PredefinedMenuItem::separator()).unwrap();
    menu.append(&quit).unwrap();

    let menu_events = MenuEvent::receiver();
    let mut tray = None; // created on first loop tick (required on macOS)

    event_loop.run(move |_event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100));

        if tray.is_none() {
            tray = Some(
                TrayIconBuilder::new()
                    .with_menu(Box::new(menu.clone()))
                    .with_tooltip("some-ting — TING push-to-talk")
                    .with_icon(solid_icon([130, 130, 130]))
                    .build()
                    .expect("build tray"),
            );
        }

        while let Ok(st) = rx.try_recv() {
            match st {
                Status::Listening { sample_rate } => {
                    let _ = status.set_text(format!("● listening @ {sample_rate} Hz"));
                    if let Some(t) = &tray {
                        let _ = t.set_icon(Some(solid_icon([0, 170, 60])));
                    }
                }
                Status::Reconnecting => {
                    let _ = status.set_text("… reconnecting");
                }
                Status::Event { event, acted } => {
                    let tag = if acted { "" } else { " (ignored)" };
                    let _ = status.set_text(format!("{event:?}{tag}"));
                }
                Status::Level { held, .. } => {
                    if let Some(t) = &tray {
                        let c = if held { [220, 40, 40] } else { [0, 170, 60] };
                        let _ = t.set_icon(Some(solid_icon(c)));
                    }
                }
                Status::Error(e) => {
                    let _ = status.set_text(format!("error: {e}"));
                }
            }
        }

        if let Ok(ev) = menu_events.try_recv() {
            if ev.id == quit.id() {
                stop.store(true, Ordering::Relaxed);
                *control_flow = ControlFlow::Exit;
            }
        }
    });
}

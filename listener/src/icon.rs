//! Procedural status-tray icons (RGBA8), shared so the live tray and the PNG
//! previews are pixel-identical. A filled "transmit" dot whose COLOR is the
//! state signal:
//!   Idle      — grey disc with a darker hub (running, not listening)
//!   Listening — green disc with a darker hub (armed / standing by)
//!   Keyed     — solid red disc (voice key held / transmitting)
//!   Paused    — grey disc with a pause glyph (||)
//!
//! Every state paints the SAME fully-opaque disc footprint (only the color and
//! an inner glyph differ). That's deliberate: some XEmbed/SNI tray hosts
//! (snixembed → i3bar) don't clear the old pixmap before drawing the new one,
//! so a transparent-background icon would let the previous state show through
//! ("red under green"). An identical opaque footprint overpaints it cleanly.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconState {
    Idle,
    Listening,
    Keyed,
    Paused,
}

pub const SIZE: u32 = 22;

fn color(s: IconState) -> (u8, u8, u8) {
    match s {
        IconState::Idle => (150, 150, 150),
        IconState::Listening => (40, 185, 95), // standing by / armed → green
        IconState::Keyed => (228, 55, 55),     // voice key held / recording → red
        IconState::Paused => (120, 120, 120),
    }
}

/// 1px antialiased edge: `e` is (radius − distance); ~0 right at the edge.
fn aa(e: f32) -> f32 {
    (e + 0.5).clamp(0.0, 1.0)
}

/// RGBA8 buffer, SIZE×SIZE, premultiplied-friendly (straight alpha).
///
/// Every state is a filled, fully-opaque disc of the SAME radius — only the
/// color and an opaque inner glyph change between states. That identical opaque
/// footprint is what lets a fresh icon completely overpaint the previous one on
/// tray hosts that don't clear before drawing (see the module docs).
pub fn rgba(state: IconState) -> Vec<u8> {
    let n = SIZE as i32;
    let (cr, cg, cb) = color(state);
    let c = (n as f32 - 1.0) / 2.0;
    let r_out = c - 1.5;
    let r_in = r_out - 3.2; // inner-glyph radius (hub / pause bars)
    let mut buf = vec![0u8; (n * n * 4) as usize];

    for y in 0..n {
        for x in 0..n {
            let dx = x as f32 - c;
            let dy = y as f32 - c;
            let d = (dx * dx + dy * dy).sqrt();

            // Base: one filled opaque disc for EVERY state (AA only at the rim).
            let a = aa(r_out - d);

            // Inner glyph, drawn in an opaque contrasting shade so the opaque
            // footprint stays identical across states (clean overpaint).
            let (mut rr, mut gg, mut bb) = (cr, cg, cb);
            match state {
                // Keyed = solid disc (transmitting), no inner glyph.
                IconState::Keyed => {}
                // A darker hub reads as "armed/idle, not transmitting".
                IconState::Listening if d < r_in - 1.0 => (rr, gg, bb) = (24, 120, 62),
                IconState::Idle if d < r_in - 1.0 => (rr, gg, bb) = (105, 105, 105),
                IconState::Paused => {
                    let in_bar = dx.abs() > 1.1 && dx.abs() < 2.7 && dy.abs() < r_in;
                    if in_bar {
                        (rr, gg, bb) = (60, 60, 60);
                    }
                }
                _ => {}
            }

            let i = ((y * n + x) * 4) as usize;
            buf[i] = rr;
            buf[i + 1] = gg;
            buf[i + 2] = bb;
            buf[i + 3] = (a.clamp(0.0, 1.0) * 255.0) as u8;
        }
    }
    buf
}

/// Alpha of the center pixel for `state` (0..=255).
#[cfg(test)]
fn center_alpha(state: IconState) -> u8 {
    let n = SIZE as i32;
    let c = (n - 1) / 2;
    let buf = rgba(state);
    buf[(((c * n + c) * 4) + 3) as usize]
}

#[cfg(test)]
fn opaque_count(state: IconState) -> usize {
    rgba(state)
        .chunks_exact(4)
        .filter(|px| px[3] > 200)
        .count()
}

/// Write each state's RGBA to `<dir>/<state>.rgba` (raw, SIZE×SIZE×4) for preview.
pub fn dump_rgba(dir: &str) -> std::io::Result<()> {
    for (name, st) in [
        ("idle", IconState::Idle),
        ("listening", IconState::Listening),
        ("keyed", IconState::Keyed),
        ("paused", IconState::Paused),
    ] {
        std::fs::write(format!("{dir}/{name}.rgba"), rgba(st))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [IconState; 4] = [
        IconState::Idle,
        IconState::Listening,
        IconState::Keyed,
        IconState::Paused,
    ];

    #[test]
    fn buffer_is_correct_size_and_drawn() {
        for s in ALL {
            let buf = rgba(s);
            assert_eq!(buf.len(), (SIZE * SIZE * 4) as usize, "{s:?} wrong size");
            assert!(opaque_count(s) > 0, "{s:?} drew nothing");
        }
    }

    #[test]
    fn identical_opaque_footprint() {
        // The clean-overpaint invariant: every state fills the SAME opaque disc,
        // so a fresh icon fully covers the previous one even on tray hosts that
        // don't clear before drawing. If footprints differ, stale pixels of the
        // old state would show through ("red under green").
        let base = opaque_count(IconState::Idle);
        for s in ALL {
            assert_eq!(opaque_count(s), base, "{s:?} opaque footprint differs");
        }
    }

    #[test]
    fn every_state_is_a_filled_disc() {
        // No hollow centers — the middle pixel is opaque for all states.
        for s in ALL {
            assert!(center_alpha(s) > 200, "{s:?} center not opaque");
        }
    }

    #[test]
    fn color_distinguishes_listening_and_keyed() {
        // The primary signal is color: listening reads green, keyed reads red.
        let (lr, lg, lb) = color(IconState::Listening);
        assert!(lg > lr && lg > lb, "listening should read green");
        let (kr, kg, kb) = color(IconState::Keyed);
        assert!(kr > kg && kr > kb, "keyed should read red");
    }

    #[test]
    fn every_state_renders_to_a_valid_icon_buffer() {
        // Guards the GUI's Icon::from_rgba(...).expect("icon") call: the buffer
        // must be exactly SIZE*SIZE*4 for tray-icon to accept it.
        for s in ALL {
            assert_eq!(rgba(s).len(), (SIZE as usize).pow(2) * 4);
        }
    }
}

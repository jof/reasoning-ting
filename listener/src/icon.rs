//! Procedural status-tray icons (RGBA8), shared so the live tray and the PNG
//! previews are pixel-identical. A circular "transmit" motif:
//!   Idle      — grey hollow ring (running, not listening)
//!   Listening — green ring with a center dot (armed/ready)
//!   Keyed     — solid red disc (recording / key held)
//!   Paused    — dim ring with a pause glyph (||)

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
        IconState::Listening => (40, 185, 95),
        IconState::Keyed => (228, 55, 55),
        IconState::Paused => (140, 140, 140),
    }
}

/// 1px antialiased edge: `e` is (radius − distance); ~0 right at the edge.
fn aa(e: f32) -> f32 {
    (e + 0.5).clamp(0.0, 1.0)
}

/// RGBA8 buffer, SIZE×SIZE, premultiplied-friendly (straight alpha).
pub fn rgba(state: IconState) -> Vec<u8> {
    let n = SIZE as i32;
    let (cr, cg, cb) = color(state);
    let c = (n as f32 - 1.0) / 2.0;
    let r_out = c - 1.5;
    let ring_w = 3.2;
    let r_in = r_out - ring_w;
    let mut buf = vec![0u8; (n * n * 4) as usize];

    for y in 0..n {
        for x in 0..n {
            let dx = x as f32 - c;
            let dy = y as f32 - c;
            let d = (dx * dx + dy * dy).sqrt();

            let mut a = match state {
                IconState::Keyed => aa(r_out - d), // filled disc
                _ => aa(r_out - d).min(aa(d - r_in)), // ring
            };
            if state == IconState::Listening {
                a = a.max(aa(r_in - 1.8 - d)); // center dot
            }
            if state == IconState::Paused {
                // two vertical bars over the ring center
                let in_bar = dx.abs() > 1.1 && dx.abs() < 2.7 && dy.abs() < r_in;
                if in_bar {
                    a = 1.0;
                }
            }

            let i = ((y * n + x) * 4) as usize;
            buf[i] = cr;
            buf[i + 1] = cg;
            buf[i + 2] = cb;
            buf[i + 3] = (a.clamp(0.0, 1.0) * 255.0) as u8;
        }
    }
    buf
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

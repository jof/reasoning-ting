//! Cross-platform key synthesis via enigo (X11 / macOS CGEvent / Windows).
//! Push-to-talk: key is held DOWN for the squeeze, released on let-go — so the
//! squeeze duration maps exactly onto Claude's voice recording window.

use anyhow::{anyhow, Result};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

pub struct Injector {
    enigo: Enigo,
    key: Key,
}

impl Injector {
    pub fn new(key: Key) -> Result<Self> {
        let enigo = Enigo::new(&Settings::default())
            .map_err(|e| anyhow!("enigo init failed: {e} (macOS needs Accessibility permission)"))?;
        Ok(Self { enigo, key })
    }
    pub fn down(&mut self) {
        let _ = self.enigo.key(self.key, Direction::Press);
    }
    pub fn up(&mut self) {
        let _ = self.enigo.key(self.key, Direction::Release);
    }
}

/// Parse a key name (matching what's set in ~/.claude/keybindings.json).
/// Supports f1..f20, space, enter, tab, and single printable characters.
pub fn parse_key(s: &str) -> Result<Key> {
    let l = s.trim().to_ascii_lowercase();
    let fkey = |n: u8| -> Result<Key> {
        Ok(match n {
            1 => Key::F1,
            2 => Key::F2,
            3 => Key::F3,
            4 => Key::F4,
            5 => Key::F5,
            6 => Key::F6,
            7 => Key::F7,
            8 => Key::F8,
            9 => Key::F9,
            10 => Key::F10,
            11 => Key::F11,
            12 => Key::F12,
            13 => Key::F13,
            14 => Key::F14,
            15 => Key::F15,
            16 => Key::F16,
            17 => Key::F17,
            18 => Key::F18,
            19 => Key::F19,
            20 => Key::F20,
            _ => return Err(anyhow!("unsupported function key: f{n}")),
        })
    };
    if let Some(num) = l.strip_prefix('f') {
        if let Ok(n) = num.parse::<u8>() {
            return fkey(n);
        }
    }
    Ok(match l.as_str() {
        "space" => Key::Space,
        "enter" | "return" => Key::Return,
        "tab" => Key::Tab,
        _ if l.chars().count() == 1 => Key::Unicode(l.chars().next().unwrap()),
        _ => return Err(anyhow!("unrecognized key: {s:?}")),
    })
}

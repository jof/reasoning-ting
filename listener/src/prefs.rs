//! Persisted GUI preferences — device, sensitivity, focus guard — so the menu-
//! bar app remembers your choices across restarts.
//!
//! Stored at `$XDG_CONFIG_HOME/reasoning-ting/config` (default `~/.config/...`) in a
//! dead-simple `key=value` line format — no serde dependency, and parsing
//! splits on the *first* `=` only so ALSA device names like
//! `front:CARD=BRIO,DEV=0` survive a round-trip intact.

use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct Prefs {
    /// Input device substring; `None` = system default.
    pub device: Option<String>,
    pub threshold: f32,
    pub focus_guard: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            device: None,
            threshold: 0.008, // matches Params::default()
            focus_guard: true,
        }
    }
}

/// `~/.config/reasoning-ting/config` (honoring `$XDG_CONFIG_HOME`). `None` if no HOME.
pub fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("reasoning-ting").join("config"))
}

impl Prefs {
    /// Load prefs, falling back to defaults for a missing file or any unknown/
    /// malformed line (forward-compatible: unknown keys are ignored).
    pub fn load() -> Prefs {
        match path().and_then(|p| std::fs::read_to_string(p).ok()) {
            Some(s) => Self::parse(&s),
            None => Prefs::default(),
        }
    }

    pub fn parse(s: &str) -> Prefs {
        let mut p = Prefs::default();
        for line in s.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let (k, v) = (k.trim(), v.trim());
            match k {
                "device" => p.device = if v.is_empty() { None } else { Some(v.to_string()) },
                "threshold" => {
                    if let Ok(f) = v.parse::<f32>() {
                        p.threshold = f;
                    }
                }
                "focus_guard" => p.focus_guard = v == "true",
                _ => {}
            }
        }
        p
    }

    pub fn serialize(&self) -> String {
        format!(
            "# reasoning-ting preferences\ndevice={}\nthreshold={}\nfocus_guard={}\n",
            self.device.as_deref().unwrap_or(""),
            self.threshold,
            self.focus_guard,
        )
    }

    /// Best-effort write (creates the config dir). Errors are swallowed — a
    /// failed save must never take down the tray.
    pub fn save(&self) {
        if let Some(p) = path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(p, self.serialize());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_preserves_device_with_equals_signs() {
        let p = Prefs {
            device: Some("front:CARD=BRIO,DEV=0".into()),
            threshold: 0.002,
            focus_guard: false,
        };
        let back = Prefs::parse(&p.serialize());
        assert_eq!(back, p);
    }

    #[test]
    fn missing_and_unknown_keys_fall_back_to_defaults() {
        let p = Prefs::parse("threshold=0.02\nfuture_key=whatever\n");
        assert_eq!(p.threshold, 0.02);
        assert_eq!(p.device, None); // unspecified
        assert!(p.focus_guard); // default
    }

    #[test]
    fn empty_device_is_none() {
        assert_eq!(Prefs::parse("device=\n").device, None);
        assert_eq!(
            Prefs::parse("device=hw:0\n").device,
            Some("hw:0".to_string())
        );
    }

    #[test]
    fn garbage_threshold_keeps_default() {
        assert_eq!(Prefs::parse("threshold=notanumber\n").threshold, 0.008);
    }
}

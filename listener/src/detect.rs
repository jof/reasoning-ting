//! Quindar tone detector — portable core (no platform deps).
//!
//! Mirrors the validated Python pipeline: a sliding ~85 ms window, windowed-DFT
//! magnitude at 2525 Hz (intro/press) and 2475 Hz (outro/release), with a
//! threshold + dominance-ratio + refractory gate, and a press/release state
//! machine. Proven on real captures at ~290x discrimination.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// 2525 Hz burst — handle squeezed, talk START.
    Intro,
    /// 2475 Hz burst — handle released, talk STOP.
    Outro,
}

pub struct Params {
    pub f_intro: f32,
    pub f_outro: f32,
    pub win_secs: f32,
    pub hop_secs: f32,
    pub threshold: f32,
    pub min_ratio: f32,
    pub refractory_secs: f32,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            f_intro: 2525.0,
            f_outro: 2475.0,
            win_secs: 0.085,
            hop_secs: 0.021,
            threshold: 0.008,
            min_ratio: 4.0,
            refractory_secs: 0.30,
        }
    }
}

pub struct Detector {
    win: usize,
    hop: usize,
    // hann-windowed complex reference vectors: (re, im) of w[i]*exp(-j2pi f i / sr)
    ref_in: Vec<(f32, f32)>,
    ref_out: Vec<(f32, f32)>,
    buf: Vec<f32>, // ring buffer of the last `win` samples
    pos: usize,    // index of the oldest sample in `buf`
    filled: usize,
    since_hop: usize,
    threshold: f32,
    min_ratio: f32,
    refractory: usize,
    since_evt: usize,
    held: bool,
}

impl Detector {
    pub fn new(sample_rate: f32, p: &Params) -> Self {
        let win = (p.win_secs * sample_rate).round() as usize;
        let hop = ((p.hop_secs * sample_rate).round() as usize).max(1);
        let mk = |f: f32| -> Vec<(f32, f32)> {
            (0..win)
                .map(|i| {
                    let t = i as f32;
                    let hann =
                        0.5 - 0.5 * (2.0 * std::f32::consts::PI * t / (win as f32 - 1.0)).cos();
                    let ang = 2.0 * std::f32::consts::PI * f * t / sample_rate;
                    (hann * ang.cos(), -hann * ang.sin())
                })
                .collect()
        };
        Self {
            win,
            hop,
            ref_in: mk(p.f_intro),
            ref_out: mk(p.f_outro),
            buf: vec![0.0; win],
            pos: 0,
            filled: 0,
            since_hop: 0,
            threshold: p.threshold,
            min_ratio: p.min_ratio,
            refractory: (p.refractory_secs * sample_rate) as usize,
            since_evt: usize::MAX / 2,
            held: false,
        }
    }

    /// True while the daemon believes the handle is squeezed (key held down).
    pub fn held(&self) -> bool {
        self.held
    }

    /// Force the held state (used by a max-hold safety release in the caller).
    pub fn set_held(&mut self, v: bool) {
        self.held = v;
    }

    /// Feed one sample; returns an event at a tone edge, else None.
    pub fn push(&mut self, s: f32) -> Option<Event> {
        // write into ring buffer
        self.buf[self.pos] = s;
        self.pos = (self.pos + 1) % self.win;
        if self.filled < self.win {
            self.filled += 1;
        }
        self.since_hop += 1;
        self.since_evt = self.since_evt.saturating_add(1);

        if self.since_hop < self.hop || self.filled < self.win {
            return None;
        }
        self.since_hop = 0;

        // windowed-DFT magnitude at both tones (iterate oldest->newest)
        let (mut ri, mut ii, mut ro, mut io) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for k in 0..self.win {
            let x = self.buf[(self.pos + k) % self.win];
            let (cr, ci) = self.ref_in[k];
            let (dr, di) = self.ref_out[k];
            ri += x * cr;
            ii += x * ci;
            ro += x * dr;
            io += x * di;
        }
        let w = self.win as f32;
        let m_in = (ri * ri + ii * ii).sqrt() / w;
        let m_out = (ro * ro + io * io).sqrt() / w;

        let (hi, lo, is_intro) = if m_in >= m_out {
            (m_in, m_out, true)
        } else {
            (m_out, m_in, false)
        };
        if hi > self.threshold
            && hi > self.min_ratio * (lo + 1e-9)
            && self.since_evt > self.refractory
        {
            self.since_evt = 0;
            if is_intro && !self.held {
                self.held = true;
                return Some(Event::Intro);
            } else if !is_intro && self.held {
                self.held = false;
                return Some(Event::Outro);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn feed_tone(d: &mut Detector, sr: f32, freq: f32, secs: f32, amp: f32) -> Vec<Event> {
        let n = (sr * secs) as usize;
        let mut evs = vec![];
        for i in 0..n {
            let s = amp * (2.0 * PI * freq * i as f32 / sr).sin();
            if let Some(e) = d.push(s) {
                evs.push(e);
            }
        }
        evs
    }
    fn feed_silence(d: &mut Detector, sr: f32, secs: f32) {
        for _ in 0..(sr * secs) as usize {
            d.push(0.0);
        }
    }

    #[test]
    fn detects_intro_then_outro() {
        let sr = 48000.0;
        let mut d = Detector::new(sr, &Params::default());
        // squeeze: intro tone, talk (silence), release: outro tone
        let e1 = feed_tone(&mut d, sr, 2525.0, 0.2, 0.4);
        feed_silence(&mut d, sr, 1.0);
        let e2 = feed_tone(&mut d, sr, 2475.0, 0.2, 0.4);
        assert_eq!(e1.first(), Some(&Event::Intro), "intro on 2525 squeeze");
        assert_eq!(e2.first(), Some(&Event::Outro), "outro on 2475 release");
    }

    #[test]
    fn ignores_silence_and_voice_band() {
        let sr = 48000.0;
        let mut d = Detector::new(sr, &Params::default());
        feed_silence(&mut d, sr, 0.5);
        // a 300 Hz "voice-ish" tone must not trigger
        let evs = feed_tone(&mut d, sr, 300.0, 0.5, 0.3);
        assert!(evs.is_empty(), "voice-band tone must not trigger: {evs:?}");
    }

    #[test]
    fn no_double_intro_without_release() {
        let sr = 48000.0;
        let mut d = Detector::new(sr, &Params::default());
        let a = feed_tone(&mut d, sr, 2525.0, 0.2, 0.4);
        feed_silence(&mut d, sr, 0.5);
        let b = feed_tone(&mut d, sr, 2525.0, 0.2, 0.4); // second intro w/o outro
        assert_eq!(a.first(), Some(&Event::Intro));
        assert!(b.is_empty(), "must not re-fire intro while held: {b:?}");
    }
}

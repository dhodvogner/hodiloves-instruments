//! Gated reverb and the drum bus (transient punch, saturation, glue compression).

use plugin_common::dsp::fast_tanh;

// ---------------------------------------------------------------------------------------------
// Reverb: four diffusing all-passes into an eight-line feedback delay network
// ---------------------------------------------------------------------------------------------

struct Delay {
    buf: Vec<f32>,
    pos: usize,
}

impl Delay {
    fn new(len: usize) -> Self {
        Self { buf: vec![0.0; len.max(1)], pos: 0 }
    }

    #[inline]
    fn read(&self) -> f32 {
        self.buf[self.pos]
    }

    #[inline]
    fn write(&mut self, x: f32) {
        self.buf[self.pos] = x;
        self.pos += 1;
        if self.pos == self.buf.len() {
            self.pos = 0;
        }
    }

    fn clear(&mut self) {
        self.buf.iter_mut().for_each(|x| *x = 0.0);
    }
}

const AP_LEN: [usize; 4] = [113, 337, 211, 431];
const LINE_LEN: [usize; 8] = [1423, 1637, 1811, 2003, 2251, 2473, 2707, 2939];

pub struct Reverb {
    sr: f32,
    ap: [Delay; 4],
    lines: [Delay; 8],
    lp: [f32; 8],
    gain: [f32; 8],
    damp: f32,
    last_size: f32,
    last_tone: f32,
}

impl Reverb {
    pub fn new(sample_rate: f32) -> Self {
        let scale = sample_rate / 48_000.0;
        let len = |n: usize| (n as f32 * scale) as usize;
        let mut r = Self {
            sr: sample_rate,
            ap: AP_LEN.map(|n| Delay::new(len(n))),
            lines: LINE_LEN.map(|n| Delay::new(len(n))),
            lp: [0.0; 8],
            gain: [0.0; 8],
            damp: 0.5,
            last_size: -1.0,
            last_tone: -1.0,
        };
        r.set(0.6, 0.6);
        r
    }

    pub fn reset(&mut self) {
        self.ap.iter_mut().for_each(Delay::clear);
        self.lines.iter_mut().for_each(Delay::clear);
        self.lp = [0.0; 8];
    }

    /// `size` 0..1 sets the decay time (0.3 to 4 s), `tone` 0..1 the brightness of the tail.
    pub fn set(&mut self, size: f32, tone: f32) {
        if size == self.last_size && tone == self.last_tone {
            return;
        }
        self.last_size = size;
        self.last_tone = tone;
        let rt60 = 0.3 + 3.7 * size * size;
        for (g, line) in self.gain.iter_mut().zip(self.lines.iter()) {
            *g = 10f32.powf(-3.0 * line.buf.len() as f32 / (rt60 * self.sr));
        }
        let cutoff = 1200.0 * (12.0f32).powf(tone);
        self.damp = 1.0 - (-std::f32::consts::TAU * cutoff / self.sr).exp();
    }

    /// Mono in, stereo out (wet only).
    #[inline]
    pub fn process(&mut self, x: f32) -> (f32, f32) {
        let mut d = x;
        for ap in self.ap.iter_mut() {
            let delayed = ap.read();
            let v = d + 0.6 * delayed;
            ap.write(v);
            d = delayed - 0.6 * v;
        }
        let mut o = [0.0f32; 8];
        let mut sum = 0.0;
        for i in 0..8 {
            self.lp[i] += (self.lines[i].read() - self.lp[i]) * self.damp;
            o[i] = self.lp[i] * self.gain[i];
            sum += o[i];
        }
        // Householder feedback matrix: every line feeds every other, with no energy gain.
        let mix = sum * 0.25;
        for i in 0..8 {
            self.lines[i].write(d + o[i] - mix);
        }
        ((o[0] - o[2] + o[4] - o[6]) * 0.4, (o[1] - o[3] + o[5] - o[7]) * 0.4)
    }
}

/// The gate of the gated reverb: every hit that feeds the reverb holds it open for a set time,
/// then it shuts within a few milliseconds.
pub struct Gate {
    hold: u32,
    env: f32,
    attack: f32,
    release: f32,
}

impl Gate {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            hold: 0,
            env: 0.0,
            attack: 1.0 - (-1.0 / (0.001 * sample_rate)).exp(),
            release: 1.0 - (-1.0 / (0.012 * sample_rate)).exp(),
        }
    }

    pub fn reset(&mut self) {
        self.hold = 0;
        self.env = 0.0;
    }

    pub fn open(&mut self, samples: u32) {
        self.hold = self.hold.max(samples);
    }

    /// Gain for this sample. `gated` false leaves the reverb fully open.
    #[inline]
    pub fn next(&mut self, gated: bool) -> f32 {
        let target = if !gated || self.hold > 0 { 1.0 } else { 0.0 };
        self.hold = self.hold.saturating_sub(1);
        let k = if target > self.env { self.attack } else { self.release };
        self.env += (target - self.env) * k;
        self.env
    }
}

// ---------------------------------------------------------------------------------------------
// Drum bus
// ---------------------------------------------------------------------------------------------

fn one_pole(seconds: f32, sample_rate: f32) -> f32 {
    1.0 - (-1.0 / (seconds * sample_rate)).exp()
}

pub struct Bus {
    fast: f32,
    slow: f32,
    comp: f32,
    k_fast_up: f32,
    k_slow_up: f32,
    k_down: f32,
    k_comp_up: f32,
    k_comp_down: f32,
}

impl Bus {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            fast: 0.0,
            slow: 0.0,
            comp: 0.0,
            k_fast_up: one_pole(0.0005, sample_rate),
            k_slow_up: one_pole(0.015, sample_rate),
            k_down: one_pole(0.03, sample_rate),
            k_comp_up: one_pole(0.008, sample_rate),
            k_comp_down: one_pole(0.15, sample_rate),
        }
    }

    pub fn reset(&mut self) {
        self.fast = 0.0;
        self.slow = 0.0;
        self.comp = 0.0;
    }

    /// `punch`, `drive` and `glue` are 0..1.
    #[inline]
    pub fn process(&mut self, l: f32, r: f32, punch: f32, drive: f32, glue: f32) -> (f32, f32) {
        let level = l.abs().max(r.abs());

        // Punch: the gap between a fast and a slow envelope is the transient; boost it.
        self.fast += (level - self.fast) * if level > self.fast { self.k_fast_up } else { self.k_down };
        self.slow += (level - self.slow) * if level > self.slow { self.k_slow_up } else { self.k_down };
        let transient = ((self.fast - self.slow) / (self.fast + 1e-4)).clamp(0.0, 1.0);
        let g = 1.0 + 1.5 * punch * transient;
        let (mut l, mut r) = (l * g, r * g);

        // Drive: tape-style saturation, blended in so that 0 % is exactly clean.
        if drive > 0.001 {
            let amount = 1.0 + 5.0 * drive;
            let mix = drive.sqrt();
            let makeup = 1.0 / amount.sqrt();
            l += mix * (fast_tanh(l * amount) * makeup - l);
            r += mix * (fast_tanh(r * amount) * makeup - r);
        }

        // Glue: one compressor over the whole kit, 4:1, with automatic make-up gain.
        if glue > 0.001 {
            let level = l.abs().max(r.abs());
            self.comp += (level - self.comp) * if level > self.comp { self.k_comp_up } else { self.k_comp_down };
            let threshold = 1.0 - 0.88 * glue;
            let gain = if self.comp > threshold { (threshold / self.comp).powf(0.75) } else { 1.0 };
            let makeup = threshold.powf(-0.4);
            l *= gain * makeup;
            r *= gain * makeup;
        }
        (l, r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverb_rings_and_stays_stable() {
        let mut r = Reverb::new(48_000.0);
        r.set(1.0, 1.0);
        let mut peak = 0.0f32;
        let mut late = 0.0f32;
        for i in 0..96_000 {
            let (l, rr) = r.process(if i < 480 { 1.0 } else { 0.0 });
            assert!(l.is_finite() && rr.is_finite());
            peak = peak.max(l.abs()).max(rr.abs());
            if i > 48_000 {
                late = late.max(l.abs()).max(rr.abs());
            }
        }
        assert!(peak < 20.0, "reverb blew up: {peak}");
        assert!(late > 1e-4, "a four second reverb is silent after one second");
        assert!(late < peak, "reverb is not decaying");
    }

    #[test]
    fn gate_opens_fast_and_shuts_after_the_hold() {
        let mut g = Gate::new(48_000.0);
        assert!(g.next(true) < 0.01);
        g.open(4800);
        let mut v = 0.0;
        for _ in 0..480 {
            v = g.next(true);
        }
        assert!(v > 0.99, "gate only at {v} after 10 ms");
        for _ in 0..(4800 + 4800) {
            v = g.next(true);
        }
        assert!(v < 0.001, "gate still at {v} 100 ms after the hold ended");
        assert!((0..2000).map(|_| g.next(false)).last().unwrap() > 0.99);
    }

    #[test]
    fn bus_is_transparent_at_zero_and_bounded_when_pushed() {
        let mut b = Bus::new(48_000.0);
        let (l, r) = b.process(0.4, -0.3, 0.0, 0.0, 0.0);
        assert!((l - 0.4).abs() < 1e-6 && (r + 0.3).abs() < 1e-6);
        let mut peak = 0.0f32;
        for i in 0..48_000 {
            let x = if i % 12_000 < 2000 { 1.5 } else { 0.0 };
            let (l, _) = b.process(x, x, 1.0, 1.0, 1.0);
            peak = peak.max(l.abs());
        }
        assert!(peak.is_finite() && peak < 3.0, "bus peak {peak}");
    }
}

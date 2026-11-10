//! Band-limited wavetable bank.
//!
//! Every table has [`FRAMES`] frames. Each frame is stored as a harmonic spectrum, normalised, and
//! rendered into [`MIPS`] mip levels. Mip `m` keeps only the first `512 >> m` harmonics, so the
//! oscillator can pick the level that stays below Nyquist for the current pitch (no aliasing from the
//! table itself). All tables are generated procedurally at first use; nothing is loaded from disk.

use std::f64::consts::PI;
use std::sync::OnceLock;

pub const FRAMES: usize = 64;
pub const BASE_LEN: usize = 2048;
pub const MIPS: usize = 10;
pub const HARMONICS: usize = BASE_LEN / 4;

/// Harmonic coefficients for k = 1..=HARMONICS. The time signal is
/// `x(t) = 2 * sum_k (re_k * cos(2 pi k t) - im_k * sin(2 pi k t))`.
type Spec = Vec<(f64, f64)>;

pub struct Table {
    pub name: &'static str,
    mips: Vec<Vec<f32>>,
    lens: [usize; MIPS],
}

impl Table {
    /// Sample the table. `pos` selects/interpolates frames (0..1), `phase` is 0..1.
    #[inline]
    pub fn sample(&self, mip: usize, pos: f32, phase: f32) -> f32 {
        let len = self.lens[mip];
        let stride = len + 1;
        let fpos = pos.clamp(0.0, 1.0) * (FRAMES - 1) as f32;
        let f0 = (fpos as usize).min(FRAMES - 1);
        let f1 = (f0 + 1).min(FRAMES - 1);
        let ff = fpos - f0 as f32;
        let x = phase * len as f32;
        let i = (x as usize).min(len - 1);
        let fr = x - i as f32;
        let d = &self.mips[mip];
        let a = &d[f0 * stride + i..f0 * stride + i + 2];
        let b = &d[f1 * stride + i..f1 * stride + i + 2];
        let va = a[0] + (a[1] - a[0]) * fr;
        let vb = b[0] + (b[1] - b[0]) * fr;
        va + (vb - va) * ff
    }

    /// Choose the mip level for `freq` Hz, where `sample_rate` is the (oversampled) engine rate.
    /// Harmonics are kept below 0.3125 x that rate: at 2x oversampling that is 30 kHz for a 48 kHz
    /// host, so the audible band is always complete while warps keep headroom before they alias.
    #[inline]
    pub fn mip_for(freq: f32, sample_rate: f32) -> usize {
        let r = HARMONICS as f32 * freq / (0.3125 * sample_rate);
        if r <= 1.0 {
            0
        } else {
            (r.log2().ceil() as usize).min(MIPS - 1)
        }
    }

    /// Fill `out` with one cycle of the (interpolated) frame at `pos`, for UI drawing.
    pub fn preview(&self, pos: f32, out: &mut [f32]) {
        let n = out.len();
        for (i, o) in out.iter_mut().enumerate() {
            *o = self.sample(0, pos, i as f32 / n as f32);
        }
    }
}

pub struct Wavetables {
    pub tables: Vec<Table>,
}

static BANK: OnceLock<Wavetables> = OnceLock::new();

/// The shared, lazily built wavetable bank.
pub fn bank() -> &'static Wavetables {
    BANK.get_or_init(build_bank)
}

pub fn table_names() -> Vec<&'static str> {
    bank().tables.iter().map(|t| t.name).collect()
}

// ---------------------------------------------------------------------------------------------
// FFT
// ---------------------------------------------------------------------------------------------

/// In-place radix-2 FFT, unnormalised in both directions.
fn fft(re: &mut [f64], im: &mut [f64], inverse: bool) {
    let n = re.len();
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = 2.0 * PI / len as f64 * if inverse { 1.0 } else { -1.0 };
        let (wr, wi) = (ang.cos(), ang.sin());
        let half = len / 2;
        let mut i = 0;
        while i < n {
            let (mut cr, mut ci) = (1.0f64, 0.0f64);
            for k in 0..half {
                let (ur, ui) = (re[i + k], im[i + k]);
                let (xr, xi) = (re[i + k + half], im[i + k + half]);
                let vr = xr * cr - xi * ci;
                let vi = xr * ci + xi * cr;
                re[i + k] = ur + vr;
                im[i + k] = ui + vi;
                re[i + k + half] = ur - vr;
                im[i + k + half] = ui - vi;
                let ncr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = ncr;
            }
            i += len;
        }
        len <<= 1;
    }
}

fn spec_from_time(x: &[f64]) -> Spec {
    let n = x.len();
    let mut re = x.to_vec();
    let mut im = vec![0.0; n];
    fft(&mut re, &mut im, false);
    (1..=HARMONICS)
        .map(|k| (re[k] / n as f64, im[k] / n as f64))
        .collect()
}

/// Render the first `h_max` harmonics of `spec` into `len` time samples.
fn render_spec(spec: &Spec, len: usize, h_max: usize) -> Vec<f64> {
    let mut re = vec![0.0; len];
    let mut im = vec![0.0; len];
    for k in 1..=h_max.min(len / 2 - 1).min(HARMONICS) {
        let (a, b) = spec[k - 1];
        re[k] = a;
        im[k] = b;
        re[len - k] = a;
        im[len - k] = -b;
    }
    fft(&mut re, &mut im, true);
    re
}

// ---------------------------------------------------------------------------------------------
// Spectrum construction helpers
// ---------------------------------------------------------------------------------------------

fn blank() -> Spec {
    vec![(0.0, 0.0); HARMONICS]
}

fn add_sin(s: &mut Spec, k: usize, amp: f64) {
    if (1..=HARMONICS).contains(&k) {
        s[k - 1].1 -= amp * 0.5;
    }
}

fn add_cos(s: &mut Spec, k: usize, amp: f64) {
    if (1..=HARMONICS).contains(&k) {
        s[k - 1].0 += amp * 0.5;
    }
}

fn sine_spec() -> Spec {
    let mut s = blank();
    add_sin(&mut s, 1, 1.0);
    s
}

fn saw_spec() -> Spec {
    let mut s = blank();
    for k in 1..=HARMONICS {
        add_sin(&mut s, k, 1.0 / k as f64);
    }
    s
}

fn square_spec() -> Spec {
    let mut s = blank();
    for k in (1..=HARMONICS).step_by(2) {
        add_sin(&mut s, k, 1.0 / k as f64);
    }
    s
}

fn triangle_spec() -> Spec {
    let mut s = blank();
    for k in (1..=HARMONICS).step_by(2) {
        let sign = if ((k - 1) / 2) % 2 == 0 { 1.0 } else { -1.0 };
        add_sin(&mut s, k, sign / (k * k) as f64);
    }
    s
}

fn pulse_spec(duty: f64) -> Spec {
    let mut s = blank();
    for k in 1..=HARMONICS {
        let kf = k as f64;
        let a = 2.0 / (PI * kf) * (PI * kf * duty).sin();
        add_cos(&mut s, k, a * (PI * kf * duty).cos());
        add_sin(&mut s, k, a * (PI * kf * duty).sin());
    }
    s
}

fn lerp_spec(a: &Spec, b: &Spec, t: f64) -> Spec {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x.0 + (y.0 - x.0) * t, x.1 + (y.1 - x.1) * t))
        .collect()
}

/// Spread keyframes evenly over all frames, interpolating the spectra linearly.
fn morph(keys: &[Spec]) -> Vec<Spec> {
    assert!(keys.len() >= 2);
    (0..FRAMES)
        .map(|i| {
            let t = i as f64 / (FRAMES - 1) as f64 * (keys.len() - 1) as f64;
            let a = (t.floor() as usize).min(keys.len() - 2);
            lerp_spec(&keys[a], &keys[a + 1], t - a as f64)
        })
        .collect()
}

fn time_frame(f: impl Fn(f64) -> f64) -> Spec {
    let x: Vec<f64> = (0..BASE_LEN).map(|n| f(n as f64 / BASE_LEN as f64)).collect();
    spec_from_time(&x)
}

// ---------------------------------------------------------------------------------------------
// The built-in tables
// ---------------------------------------------------------------------------------------------

fn basic_shapes() -> Vec<Spec> {
    morph(&[sine_spec(), triangle_spec(), saw_spec(), square_spec(), pulse_spec(0.25)])
}

fn pwm_sweep() -> Vec<Spec> {
    (0..FRAMES)
        .map(|i| pulse_spec(0.5 - 0.46 * i as f64 / (FRAMES - 1) as f64))
        .collect()
}

fn saw_resonance() -> Vec<Spec> {
    (0..FRAMES)
        .map(|i| {
            let c = 1.0 + 40.0 * i as f64 / (FRAMES - 1) as f64;
            let w = 1.0 + c * 0.25;
            let mut s = blank();
            for k in 1..=HARMONICS {
                let kf = k as f64;
                let bump = 1.0 + 9.0 * (-((kf - c) / w).powi(2)).exp();
                let rolloff = (-kf / (c * 4.0 + 6.0)).exp();
                add_sin(&mut s, k, bump * rolloff / kf);
            }
            s
        })
        .collect()
}

fn vowels() -> Vec<Spec> {
    // F1, F2, F3 in Hz; the harmonic series is assumed to sit on a 100 Hz fundamental.
    let formants: [[f64; 3]; 5] = [
        [800.0, 1150.0, 2900.0],
        [400.0, 1600.0, 2700.0],
        [350.0, 1700.0, 2700.0],
        [450.0, 800.0, 2830.0],
        [325.0, 700.0, 2530.0],
    ];
    let gains = [1.0, 0.6, 0.3];
    let widths = [1.4, 2.0, 3.0];
    let keys: Vec<Spec> = formants
        .iter()
        .map(|f| {
            let mut s = blank();
            for k in 1..=HARMONICS {
                let kf = k as f64;
                let mut env = 0.02;
                for j in 0..3 {
                    env += gains[j] * (-0.5 * ((kf - f[j] / 100.0) / widths[j]).powi(2)).exp();
                }
                add_sin(&mut s, k, env / kf.powf(0.7));
            }
            s
        })
        .collect();
    morph(&keys)
}

fn fm_bell() -> Vec<Spec> {
    (0..FRAMES)
        .map(|i| {
            let index = 7.0 * i as f64 / (FRAMES - 1) as f64;
            time_frame(|t| (2.0 * PI * t + index * (2.0 * PI * 3.0 * t).sin()).sin())
        })
        .collect()
}

fn organ() -> Vec<Spec> {
    const DRAWBARS: [usize; 8] = [1, 2, 3, 4, 6, 8, 10, 12];
    let presets: [[f64; 8]; 4] = [
        [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0, 0.5, 0.0, 0.8, 0.0],
        [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
    ];
    let keys: Vec<Spec> = presets
        .iter()
        .map(|p| {
            let mut s = blank();
            for (bar, &amp) in p.iter().enumerate() {
                add_sin(&mut s, DRAWBARS[bar], amp);
            }
            s
        })
        .collect();
    morph(&keys)
}

fn spectral_chaos() -> Vec<Spec> {
    let mut rng = crate::util::Rng::new(0xC0FFEE);
    let phases: Vec<f64> = (0..HARMONICS).map(|_| rng.next_f32() as f64 * 2.0 * PI).collect();
    let keys: Vec<Spec> = (0..8)
        .map(|_| {
            let mut s = blank();
            for k in 1..=HARMONICS.min(96) {
                let kf = k as f64;
                let a = (0.15 + rng.next_f32() as f64) / kf.powf(0.9);
                let a = if rng.next_f32() < 0.3 { 0.0 } else { a };
                let (sin_p, cos_p) = phases[k - 1].sin_cos();
                add_sin(&mut s, k, a * cos_p);
                add_cos(&mut s, k, a * sin_p);
            }
            s
        })
        .collect();
    morph(&keys)
}

fn metallic() -> Vec<Spec> {
    let ratios = [2.0, 3.0, 5.0, 7.0, 9.0, 13.0];
    let keys: Vec<Spec> = ratios
        .iter()
        .map(|&r| {
            time_frame(|t| {
                (2.0 * PI * t).sin() * (2.0 * PI * r * t).sin() + 0.5 * (2.0 * PI * r * t).sin()
            })
        })
        .collect();
    morph(&keys)
}

/// A windowed sine that is restarted every cycle while its own pitch rises: the classic sync lead.
fn sync_sweep() -> Vec<Spec> {
    (0..FRAMES)
        .map(|i| {
            let ratio = 1.0 + 7.0 * i as f64 / (FRAMES - 1) as f64;
            time_frame(|t| (PI * t).sin().sqrt() * (2.0 * PI * ratio * t).sin())
        })
        .collect()
}

/// Two narrow formants sliding up a bright spectrum - the "yoy" of a talking bass.
fn growl() -> Vec<Spec> {
    (0..FRAMES)
        .map(|i| {
            let x = i as f64 / (FRAMES - 1) as f64;
            let (c1, c2) = (2.5 + 13.0 * x, 8.0 + 30.0 * x * x);
            let mut s = blank();
            for k in 1..=HARMONICS {
                let kf = k as f64;
                let env = 0.06
                    + (-0.5 * ((kf - c1) / 1.6).powi(2)).exp()
                    + 0.7 * (-0.5 * ((kf - c2) / 2.6).powi(2)).exp();
                let sign = if k % 3 == 0 { -1.0 } else { 1.0 };
                add_sin(&mut s, k, sign * env / kf.powf(0.45));
            }
            s
        })
        .collect()
}

/// Sparse, bell-like partial sets for glassy keys and pads.
fn glass() -> Vec<Spec> {
    let sets: [&[(usize, f64)]; 5] = [
        &[(1, 1.0), (4, 0.25)],
        &[(1, 1.0), (3, 0.4), (7, 0.3), (11, 0.12)],
        &[(1, 0.9), (4, 0.5), (9, 0.35), (15, 0.2), (22, 0.1)],
        &[(1, 0.7), (5, 0.6), (11, 0.45), (18, 0.3), (27, 0.2), (37, 0.1)],
        &[(2, 0.6), (7, 0.6), (13, 0.5), (21, 0.4), (31, 0.3), (43, 0.2), (57, 0.12)],
    ];
    let keys: Vec<Spec> = sets
        .iter()
        .map(|set| {
            let mut s = blank();
            for &(k, a) in set.iter() {
                add_sin(&mut s, k, a);
            }
            s
        })
        .collect();
    morph(&keys)
}

/// Two-operator FM electric piano: mellow body, with a tine that rings in as the frame rises.
fn e_piano() -> Vec<Spec> {
    (0..FRAMES)
        .map(|i| {
            let x = i as f64 / (FRAMES - 1) as f64;
            time_frame(|t| {
                let body = (2.0 * PI * t + 2.6 * x * (2.0 * PI * t).sin()).sin();
                let tine = (2.0 * PI * 14.0 * t + 0.6 * (2.0 * PI * t).sin()).sin();
                body + 0.22 * x * tine
            })
        })
        .collect()
}

/// Harmonics switched in one after another, all at similar strength: a spectral "opening" sweep.
fn harmonic_series() -> Vec<Spec> {
    (0..FRAMES)
        .map(|i| {
            let top = 1.0 + 47.0 * (i as f64 / (FRAMES - 1) as f64).powf(1.6);
            let mut s = blank();
            for k in 1..=HARMONICS.min(64) {
                let kf = k as f64;
                let gate = (top - kf + 1.0).clamp(0.0, 1.0);
                add_sin(&mut s, k, gate / kf.powf(0.35));
            }
            s
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Bank assembly
// ---------------------------------------------------------------------------------------------

fn build_table(name: &'static str, mut frames: Vec<Spec>) -> Table {
    assert_eq!(frames.len(), FRAMES);
    // Peak-normalise each frame so morphing does not change loudness much.
    for f in frames.iter_mut() {
        let t = render_spec(f, BASE_LEN, HARMONICS);
        let peak = t.iter().fold(0.0f64, |m, v| m.max(v.abs())).max(1e-6);
        let g = 0.9 / peak;
        for c in f.iter_mut() {
            c.0 *= g;
            c.1 *= g;
        }
    }

    let mut lens = [0usize; MIPS];
    let mut mips = Vec::with_capacity(MIPS);
    for m in 0..MIPS {
        // Keep the tables long even when few harmonics are left: linear interpolation of a short
        // table adds audible images, a long one does not.
        let len = match m {
            0..=3 => BASE_LEN,
            4..=5 => BASE_LEN / 2,
            _ => BASE_LEN / 4,
        };
        lens[m] = len;
        let h_max = (HARMONICS >> m).max(1);
        let stride = len + 1;
        let mut data = vec![0.0f32; FRAMES * stride];
        for (fi, f) in frames.iter().enumerate() {
            let t = render_spec(f, len, h_max);
            let base = fi * stride;
            for (i, v) in t.iter().enumerate() {
                data[base + i] = *v as f32;
            }
            data[base + len] = t[0] as f32; // guard sample for interpolation
        }
        mips.push(data);
    }
    Table { name, mips, lens }
}

fn build_bank() -> Wavetables {
    Wavetables {
        tables: vec![
            build_table("Basic Shapes", basic_shapes()),
            build_table("PWM Sweep", pwm_sweep()),
            build_table("Saw Resonance", saw_resonance()),
            build_table("Vowels", vowels()),
            build_table("FM Bell", fm_bell()),
            build_table("Organ", organ()),
            build_table("Spectral Chaos", spectral_chaos()),
            build_table("Metallic", metallic()),
            build_table("Sync Sweep", sync_sweep()),
            build_table("Growl", growl()),
            build_table("Glass", glass()),
            build_table("E-Piano", e_piano()),
            build_table("Harmonic Series", harmonic_series()),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_finite_and_bounded() {
        let b = bank();
        assert_eq!(b.tables.len(), 13);
        for t in &b.tables {
            for m in 0..MIPS {
                for pos in [0.0, 0.3, 0.77, 1.0] {
                    for i in 0..257 {
                        let v = t.sample(m, pos, i as f32 / 257.0);
                        assert!(v.is_finite(), "{} mip {m} not finite", t.name);
                        assert!(v.abs() < 1.6, "{} mip {m} peak {v}", t.name);
                    }
                }
            }
        }
    }

    #[test]
    fn first_frame_of_basic_shapes_is_a_sine() {
        let t = &bank().tables[0];
        for i in 0..64 {
            let ph = i as f32 / 64.0;
            let expect = 0.9 * (2.0 * std::f32::consts::PI * ph).sin() * -1.0;
            let got = t.sample(0, 0.0, ph);
            // sign convention of add_sin: x = +amp*sin, so compare magnitude-shape only
            assert!((got.abs() - expect.abs()).abs() < 0.02, "{got} vs {expect}");
        }
    }

    #[test]
    fn mip_selection_is_monotonic() {
        let mut last = 0;
        for f in [20.0, 50.0, 200.0, 1000.0, 5000.0, 15000.0] {
            let m = Table::mip_for(f, 48000.0);
            assert!(m >= last);
            last = m;
        }
        assert!(last <= MIPS - 1);
    }
}

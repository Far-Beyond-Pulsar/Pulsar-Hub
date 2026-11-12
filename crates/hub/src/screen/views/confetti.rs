//! A burst of confetti: a pure function of the time since it started, drawn as
//! absolutely placed rectangles. The caller redraws every frame while it plays.

use gpui::prelude::*;
use gpui::*;

/// How long the confetti plays.
pub const DURATION_SECS: f32 = 3.2;

const COUNT: u32 = 90;
const GRAVITY: f32 = 1100.0;
const PALETTE: [(f32, f32, f32); 7] = [
    (0.98, 0.85, 0.58), // red
    (0.08, 0.95, 0.58), // orange
    (0.13, 0.95, 0.58), // amber
    (0.38, 0.70, 0.50), // green
    (0.55, 0.85, 0.58), // blue
    (0.74, 0.75, 0.62), // violet
    (0.90, 0.80, 0.65), // pink
];

#[derive(Clone, Debug, PartialEq)]
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub hue: f32,
    pub saturation: f32,
    pub lightness: f32,
    pub alpha: f32,
    pub round: bool,
}

/// A deterministic value in `0..1` for particle `i` and purpose `k`.
fn random(i: u32, k: u32) -> f32 {
    let mut x = i.wrapping_mul(2_654_435_761) ^ k.wrapping_mul(0x9E37_79B9);
    x ^= x >> 15;
    x = x.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    x = x.wrapping_mul(0xC2B2_AE35);
    x ^= x >> 16;
    (x & 0xFFFF) as f32 / 65_535.0
}

/// Every particle `t` seconds after a burst from `origin` (pixels): thrown
/// upward in a fan, then pulled down by gravity, fading out at the end.
pub fn particles(t: f32, origin: (f32, f32)) -> Vec<Particle> {
    if !(0.0..DURATION_SECS).contains(&t) {
        return Vec::new();
    }
    let fade = ((DURATION_SECS - t) / 0.9).clamp(0.0, 1.0);
    (0..COUNT)
        .map(|i| {
            // Mostly upward, spread over about 140 degrees.
            let angle = -std::f32::consts::FRAC_PI_2 + (random(i, 1) - 0.5) * 2.45;
            let speed = 380.0 + random(i, 2) * 760.0;
            let drag = 1.0 - 0.35 * (t / DURATION_SECS);
            let (vx, vy) = (angle.cos() * speed, angle.sin() * speed);
            let (hue, saturation, lightness) = PALETTE[(random(i, 3) * PALETTE.len() as f32) as usize % PALETTE.len()];
            let size = 6.0 + random(i, 4) * 8.0;
            Particle {
                x: origin.0 + vx * t * drag + (t * 9.0 + random(i, 5) * 6.28).sin() * 14.0,
                y: origin.1 + vy * t * drag + 0.5 * GRAVITY * t * t,
                width: size,
                height: if random(i, 6) > 0.5 { size } else { size * 0.45 },
                hue,
                saturation,
                lightness,
                alpha: fade,
                round: random(i, 7) > 0.7,
            }
        })
        .collect()
}

/// The confetti as an element: transparent to the mouse, drawn over whatever
/// is behind it. Nothing when `started` is `None` or the burst is over.
pub fn render_confetti(started: Option<std::time::Instant>, window: &Window) -> impl IntoElement {
    let Some(started) = started else {
        return div().into_any_element();
    };
    let size = window.viewport_size();
    let origin = (f32::from(size.width) * 0.5, f32::from(size.height) * 0.62);
    div()
        .absolute()
        .inset_0()
        .overflow_hidden()
        .children(
            particles(started.elapsed().as_secs_f32(), origin)
                .into_iter()
                .map(|p| {
                    div()
                        .absolute()
                        .left(px(p.x))
                        .top(px(p.y))
                        .w(px(p.width))
                        .h(px(p.height))
                        .when(p.round, |this| this.rounded_full())
                        .when(!p.round, |this| this.rounded_sm())
                        .bg(hsla(p.hue, p.saturation, p.lightness, p.alpha))
                }),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    // Named imports, not `super::*`: that glob brings in `gpui::test`, which
    // shadows the standard `#[test]`.
    use super::{particles, Particle, COUNT, DURATION_SECS};

    #[test]
    fn nothing_before_the_start_or_after_the_end() {
        assert!(particles(-0.1, (0.0, 0.0)).is_empty());
        assert!(particles(DURATION_SECS, (0.0, 0.0)).is_empty());
    }

    #[test]
    fn the_burst_rises_then_falls_and_fades() {
        let origin = (500.0, 500.0);
        let top = |t: f32| {
            particles(t, origin)
                .iter()
                .map(|p| p.y)
                .fold(f32::MAX, f32::min)
        };
        assert!(top(0.3) < origin.1 - 100.0, "particles are thrown upward");
        let early = particles(0.2, origin);
        let late = particles(DURATION_SECS - 0.1, origin);
        assert_eq!(early.len(), COUNT as usize);
        assert!(early.iter().all(|p| p.alpha == 1.0));
        assert!(late.iter().all(|p| p.alpha < 0.2), "faded out at the end");
        let lowest = |ps: &[Particle]| ps.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!(lowest(&late) > lowest(&early), "gravity pulls them down");
    }

    #[test]
    fn it_is_deterministic() {
        assert_eq!(particles(1.0, (10.0, 20.0)), particles(1.0, (10.0, 20.0)));
    }
}

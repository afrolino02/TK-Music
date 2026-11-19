//! The visualizer's scenes: drawings that fill the whole fullscreen player around the cover,
//! rather than a spectrum along its bottom edge. Each one is painted twice, once wide and soft
//! under a blur for its glow and once sharp on top.

use gpui::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::sync::OnceLock;
use std::time::Instant;

use gpui::{
    Background, BorderStyle, Bounds, Div, Hsla, PathBuilder, Pixels, Point, Window, canvas, div,
    hsla, linear_color_stop, linear_gradient, point, px, quad, size, transparent_black,
};

use crate::visualizer::{Levels, VisualizerStyle};

/// How far the glow layer is blurred.
const GLOW: f32 = 14.;
/// The aurora is mostly glow, so its blur is far wider.
const AURORA_GLOW: f32 = 34.;
/// How much slower everything moves with calm motion on.
const CALM_PACE: f32 = 0.5;
/// How much smaller every reaction is with calm motion on.
const CALM_REACH: f32 = 0.65;
/// The gap between two bars of the mirrored bars.
const BAR_GAP: f32 = 3.;
/// How many particles drift out from the cover.
const PARTICLES: u32 = 110;
/// How many ribbons the aurora has.
const RIBBONS: usize = 4;
/// How many points trace one ribbon of the aurora.
const RIBBON_STEPS: usize = 48;
/// How many grooves the record shows.
const GROOVES: usize = 14;
/// How many rings the pulse sends out at once.
const RINGS: usize = 5;
/// How many pieces the border line is drawn in, each its own colour.
const BORDER_PIECES: usize = 180;
/// How far in from the window edge the border line runs.
const BORDER_INSET: f32 = 1.5;
/// How often a new set of bolts is struck, per second.
const STRIKES: f32 = 14.;
/// How many kinks a bolt has.
const KINKS: usize = 9;
/// Where the lasers stand along the bottom edge, as shares of the width.
const LASER_FEET: [f32; 3] = [0.08, 0.5, 0.92];

/// One moment of a scene: the spectrum, the colours and where the cover sits.
#[derive(Clone)]
pub(crate) struct Frame {
    bands: Vec<f32>,
    bass: f32,
    mid: f32,
    high: f32,
    level: f32,
    /// Seconds since the first scene was drawn, slowed by calm motion.
    time: f32,
    /// How large reactions are, smaller with calm motion.
    reach: f32,
    calm: bool,
    rainbow: bool,
    color: Hsla,
    second: Hsla,
    cover: Option<Bounds<Pixels>>,
    corner: f32,
}

impl Frame {
    pub(crate) fn new(
        levels: &Levels,
        color: Hsla,
        rainbow: bool,
        calm: bool,
        cover: Option<Bounds<Pixels>>,
        corner: Option<Pixels>,
    ) -> Self {
        let bands = levels.mixed();
        let average = |from: usize, to: usize| {
            let part = bands.get(from..to.min(bands.len())).unwrap_or(&[]);
            match part.is_empty() {
                true => 0.,
                false => part.iter().sum::<f32>() / part.len() as f32,
            }
        };
        let (bass, mid, high, level) = (
            average(0, 6),
            average(6, 18),
            average(18, 32),
            average(0, bands.len()),
        );
        let pace = match calm {
            true => CALM_PACE,
            false => 1.,
        };
        let second = Hsla {
            h: (color.h + 0.11).rem_euclid(1.),
            s: color.s.max(0.5),
            l: (color.l + 0.1).min(0.8),
            a: 1.,
        };
        Self {
            bands,
            bass,
            mid,
            high,
            level,
            time: clock() * pace,
            reach: match calm {
                true => CALM_REACH,
                false => 1.,
            },
            calm,
            rainbow,
            color,
            second,
            cover,
            corner: corner.map_or(0., |corner| corner.as_f32()),
        }
    }

    /// The colour at `place` along a scene, from 0 to 1: a rainbow that slowly turns, or the
    /// cover's colour fading into its neighbour and back.
    fn at(&self, place: f32) -> Hsla {
        if self.rainbow {
            return hsla((place + self.time * 0.03).rem_euclid(1.), 0.9, 0.62, 1.);
        }
        let swing = ((place + self.time * 0.02) * 2.).rem_euclid(2.);
        let share = match swing > 1. {
            true => 2. - swing,
            false => swing,
        };
        mix(self.color, self.second, share)
    }

    /// The band under `place` from 0 to 1, mirrored so the low end sits at both ends.
    fn band(&self, place: f32) -> f32 {
        let count = self.bands.len();
        if count == 0 {
            return 0.;
        }
        let folded = 1. - (place.rem_euclid(1.) * 2. - 1.).abs();
        let index = ((1. - folded) * (count - 1) as f32).round() as usize;
        self.bands[index.min(count - 1)]
    }

    /// The band for bar `index` of `count`, low in the middle and high at both ends.
    fn mirrored(&self, index: usize, count: usize) -> f32 {
        let half = self.bands.len();
        if half == 0 || count == 0 {
            return 0.;
        }
        let from_middle = (index as f32 + 0.5 - count as f32 / 2.).abs() / (count as f32 / 2.);
        let band = (from_middle * half as f32) as usize;
        self.bands[band.min(half - 1)]
    }

    /// The middle of the cover and half its side, or the middle of the scene when nothing tells
    /// where the cover is.
    fn hub(&self, bounds: Bounds<Pixels>) -> (f32, f32, f32) {
        match self.cover.filter(|cover| cover.size.width > Pixels::ZERO) {
            Some(cover) => {
                let middle = cover.center();
                let side = cover.size.width.min(cover.size.height).as_f32();
                (middle.x.as_f32(), middle.y.as_f32(), side / 2.)
            }
            None => {
                let middle = bounds.center();
                let side = bounds.size.width.min(bounds.size.height).as_f32();
                (middle.x.as_f32(), middle.y.as_f32(), side * 0.2)
            }
        }
    }
}

/// The scene for `style`, glow under sharp, filling its parent.
pub(crate) fn scene(style: VisualizerStyle, frame: Frame) -> Div {
    let blur = match style {
        VisualizerStyle::Aurora => AURORA_GLOW,
        _ => GLOW,
    };
    let soft = frame.clone();
    div()
        .size_full()
        .relative()
        .child(
            div().absolute().inset_0().blur(px(blur)).child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| paint(style, bounds, &soft, true, window),
                )
                .size_full(),
            ),
        )
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| paint(style, bounds, &frame, false, window),
            )
            .absolute()
            .inset_0(),
        )
}

fn paint(
    style: VisualizerStyle,
    bounds: Bounds<Pixels>,
    frame: &Frame,
    glow: bool,
    window: &mut Window,
) {
    if bounds.size.width <= Pixels::ZERO || bounds.size.height <= Pixels::ZERO {
        return;
    }
    let area = Area::of(bounds);
    match style {
        VisualizerStyle::Mirror => mirror(area, frame, glow, window),
        VisualizerStyle::Ring => ring(area, bounds, frame, glow, window),
        VisualizerStyle::Particles => particles(area, bounds, frame, glow, window),
        VisualizerStyle::Aurora => aurora(area, frame, glow, window),
        VisualizerStyle::Vinyl => vinyl(bounds, frame, glow, window),
        VisualizerStyle::Pulse => pulse(area, bounds, frame, glow, window),
        VisualizerStyle::Border => border(area, frame, glow, window),
        VisualizerStyle::Electricity => electricity(area, bounds, frame, glow, window),
        VisualizerStyle::Lasers => lasers(area, frame, glow, window),
        VisualizerStyle::None
        | VisualizerStyle::Bars
        | VisualizerStyle::Wave
        | VisualizerStyle::Both => {}
    }
}

/// A scene's box in plain numbers.
#[derive(Clone, Copy)]
struct Area {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}

impl Area {
    fn of(bounds: Bounds<Pixels>) -> Self {
        Self {
            left: bounds.origin.x.as_f32(),
            top: bounds.origin.y.as_f32(),
            width: bounds.size.width.as_f32(),
            height: bounds.size.height.as_f32(),
        }
    }

    fn short(self) -> f32 {
        self.width.min(self.height)
    }
}

/// Bars mirrored out from the middle, low notes in the centre, reaching up and down.
fn mirror(area: Area, frame: &Frame, glow: bool, window: &mut Window) {
    let count = frame.bands.len() * 2;
    if count == 0 {
        return;
    }
    let width = (area.width - BAR_GAP * (count - 1) as f32) / count as f32;
    if width <= 0. {
        return;
    }
    let middle = area.top + area.height / 2.;
    let alpha = match glow {
        true => 0.5,
        false => 0.55,
    };
    for index in 0..count {
        let band = frame.mirrored(index, count);
        let reach = (band * area.height * 0.36 * frame.reach).max(2.);
        let left = area.left + (width + BAR_GAP) * index as f32;
        let color = frame.at(index as f32 / count as f32).opacity(alpha);
        rounded(
            window,
            (left, middle - reach),
            (width, reach * 2.),
            width / 2.,
            color,
        );
    }
}

/// Bars standing out around the cover like the rays of a sun, turning slowly.
fn ring(area: Area, bounds: Bounds<Pixels>, frame: &Frame, glow: bool, window: &mut Window) {
    let (x, y, radius) = frame.hub(bounds);
    let count = frame.bands.len() * 2;
    if count == 0 {
        return;
    }
    let base = radius * 1.1 + 6.;
    let longest = area.short() * 0.16 * frame.reach;
    let turn = frame.time * 0.08;
    let thick = (TAU * base / count as f32 * 0.5).max(2.);
    let (width, alpha) = match glow {
        true => (thick * 2.2, 0.6),
        false => (thick, 0.85),
    };
    for index in 0..count {
        let band = frame.mirrored(index, count);
        let angle = turn + index as f32 / count as f32 * TAU - FRAC_PI_2;
        let length = 3. + band * longest;
        let (cos, sin) = (angle.cos(), angle.sin());
        line(
            window,
            &[
                (x + cos * base, y + sin * base),
                (x + cos * (base + length), y + sin * (base + length)),
            ],
            width,
            frame.at(index as f32 / count as f32).opacity(alpha),
        );
    }
}

/// Specks drifting out from the cover, larger on the bass and brighter as the track gets louder.
fn particles(area: Area, bounds: Bounds<Pixels>, frame: &Frame, glow: bool, window: &mut Window) {
    let (x, y, radius) = frame.hub(bounds);
    let far = (area.width * area.width + area.height * area.height).sqrt() * 0.5;
    for index in 0..PARTICLES {
        let seed = index * 5;
        let heading = random(seed) * TAU;
        let speed = 0.05 + random(seed + 1) * 0.12;
        let life = (frame.time * speed + random(seed + 2)).fract();
        let angle = heading + life * 0.5 * (random(seed + 3) - 0.5);
        let distance = radius * 1.05 + life * far;
        let mut speck = (1.2 + random(seed + 4) * 2.8) * (1. + frame.bass * 1.6 * frame.reach);
        let mut alpha = (PI * life).sin() * (0.35 + frame.level * 0.9).min(1.);
        if glow {
            speck *= 3.;
            alpha *= 0.5;
        }
        disc(
            window,
            (x + angle.cos() * distance, y + angle.sin() * distance),
            speck,
            frame.at(index as f32 / PARTICLES as f32).opacity(alpha),
        );
    }
}

/// Ribbons of light rolling across the top half, thicker where the spectrum is loud.
fn aurora(area: Area, frame: &Frame, glow: bool, window: &mut Window) {
    let alpha = match glow {
        true => 0.55,
        false => 0.16,
    };
    for ribbon in 0..RIBBONS {
        let k = ribbon as f32;
        let mut upper = Vec::with_capacity(RIBBON_STEPS + 1);
        let mut lower = Vec::with_capacity(RIBBON_STEPS + 1);
        for step in 0..=RIBBON_STEPS {
            let along = step as f32 / RIBBON_STEPS as f32;
            let swell = (along * TAU * (0.7 + 0.25 * k) + frame.time * (0.35 + 0.1 * k) + k * 1.7)
                .sin()
                * area.height
                * 0.05
                + (along * TAU * 2.3 - frame.time * 0.5 + k).sin()
                    * area.height
                    * 0.025
                    * (1. + frame.mid * 2. * frame.reach);
            let top = area.top + area.height * (0.16 + 0.1 * k) + swell;
            let depth = area.height
                * (0.06 + (0.08 + 0.3 * frame.band(along * 0.5 + k * 0.13)) * frame.reach);
            let x = area.left + area.width * along;
            upper.push((x, top));
            lower.push((x, top + depth));
        }
        lower.reverse();
        upper.extend(lower);
        let color = frame.at(k / RIBBONS as f32);
        shape(
            window,
            &upper,
            linear_gradient(
                180.,
                linear_color_stop(color.opacity(alpha), 0.),
                linear_color_stop(color.opacity(0.), 1.),
            ),
        );
    }
}

/// A record peeking out from behind the cover, turning, its grooves catching the light with the
/// music.
fn vinyl(bounds: Bounds<Pixels>, frame: &Frame, glow: bool, window: &mut Window) {
    let (x, y, radius) = frame.hub(bounds);
    let (x, radius) = match frame.cover {
        Some(_) => (x + radius * 0.62, radius * 0.97),
        None => (x, radius * 1.6),
    };
    if glow {
        disc(
            window,
            (x, y),
            radius * 1.04,
            frame.at(0.5).opacity(0.25 + frame.bass * 0.5 * frame.reach),
        );
        return;
    }
    disc(window, (x, y), radius, hsla(0., 0., 0.05, 0.95));
    for groove in 0..GROOVES {
        let share = groove as f32 / GROOVES as f32;
        let band = frame.band(share * 0.5);
        circle(
            window,
            (x, y),
            radius * (0.4 + 0.58 * share),
            1.,
            hsla(0., 0., 1., 0.05 + 0.12 * band * frame.reach),
        );
    }
    let spin = frame.time * TAU * 0.3;
    for half in [0., PI] {
        let from = spin + half;
        let mut sheen = vec![(x, y)];
        sheen.extend((0..=8).map(|step| {
            let angle = from + 0.35 * step as f32 / 8.;
            (x + angle.cos() * radius, y + angle.sin() * radius)
        }));
        shape(window, &sheen, hsla(0., 0., 1., 0.06));
    }
    let label = radius * 0.33;
    disc(window, (x, y), label, frame.at(0.2));
    disc(
        window,
        (x + spin.cos() * label * 0.6, y + spin.sin() * label * 0.6),
        label * 0.12,
        hsla(0., 0., 1., 0.5),
    );
    disc(window, (x, y), label * 0.1, hsla(0., 0., 0.05, 1.));
}

/// Rings going out from the cover, bolder on the bass.
fn pulse(area: Area, bounds: Bounds<Pixels>, frame: &Frame, glow: bool, window: &mut Window) {
    let (x, y, radius) = frame.hub(bounds);
    let far = area.short() * 0.55;
    if glow {
        disc(
            window,
            (x, y),
            radius * 1.15,
            frame.at(0.).opacity(0.3 * frame.bass * frame.reach),
        );
    }
    for ring in 0..RINGS {
        let phase = (frame.time * 0.35 + ring as f32 / RINGS as f32).fract();
        let mut width = 1.5 + frame.bass * 5. * frame.reach;
        let mut alpha = (1. - phase).powf(1.5) * (0.2 + frame.bass * 0.8).min(1.);
        if glow {
            width *= 3.;
            alpha *= 0.7;
        }
        circle(
            window,
            (x, y),
            radius * 1.02 + phase * far,
            width,
            frame.at(phase).opacity(alpha),
        );
    }
}

/// A thin line all round the edge with a soft glow that swells on the beat and a light that
/// runs along it.
fn border(area: Area, frame: &Frame, glow: bool, window: &mut Window) {
    let path = resample(
        &perimeter(
            Area {
                left: area.left + BORDER_INSET,
                top: area.top + BORDER_INSET,
                width: area.width - BORDER_INSET * 2.,
                height: area.height - BORDER_INSET * 2.,
            },
            (frame.corner - BORDER_INSET).max(0.),
        ),
        BORDER_PIECES,
    );
    for (index, pair) in path.windows(2).enumerate() {
        let place = index as f32 / BORDER_PIECES as f32;
        let travel = 0.5 + 0.5 * (TAU * (place * 2. - frame.time * 0.25)).sin();
        let band = frame.band(place);
        let (width, alpha) = match glow {
            true => (
                6. + (frame.bass * 10. + band * 6.) * frame.reach,
                (0.3 + frame.level * 0.7) * (0.6 + 0.4 * travel),
            ),
            false => (2., (0.55 + frame.level * 0.45) * (0.5 + 0.5 * travel)),
        };
        line(window, pair, width, frame.at(place).opacity(alpha.min(1.)));
    }
}

/// Bolts striking out from the cover, more of them on the highs, longer on the bass.
fn electricity(area: Area, bounds: Bounds<Pixels>, frame: &Frame, glow: bool, window: &mut Window) {
    if frame.level < 0.04 {
        return;
    }
    let (x, y, radius) = frame.hub(bounds);
    let rate = match frame.calm {
        true => STRIKES / 2.,
        false => STRIKES,
    };
    let strike = (clock() * rate) as u32;
    let bolts = (2. + frame.high * 5. + frame.bass * 3.).round().min(8.) as u32;
    for bolt in 0..bolts {
        let seed = strike.wrapping_mul(31).wrapping_add(bolt * 97);
        let angle = random(seed) * TAU;
        let length =
            area.short() * (0.12 + random(seed + 1) * 0.3) * (0.5 + frame.bass) * frame.reach;
        let start = radius * 1.03;
        let jagged = |from: (f32, f32), angle: f32, length: f32, kinks: usize, salt: u32| {
            let (cos, sin) = (angle.cos(), angle.sin());
            (0..=kinks)
                .map(|step| {
                    let along = length * step as f32 / kinks as f32;
                    let swerve = match step == 0 {
                        true => 0.,
                        false => (random(seed + salt + step as u32) - 0.5) * length * 0.18,
                    };
                    (
                        from.0 + cos * along - sin * swerve,
                        from.1 + sin * along + cos * swerve,
                    )
                })
                .collect::<Vec<_>>()
        };
        let trunk = jagged(
            (x + angle.cos() * start, y + angle.sin() * start),
            angle,
            length,
            KINKS,
            10,
        );
        let fork = jagged(
            trunk[KINKS / 2],
            angle + (random(seed + 40) - 0.5) * 1.2,
            length * 0.4,
            4,
            50,
        );
        let flicker = 0.6 + 0.4 * random(seed + 99);
        let color = frame.at(bolt as f32 / bolts.max(1) as f32);
        let (width, color) = match glow {
            true => (5., color.opacity(0.7 * flicker)),
            false => (1.4, mix(color, hsla(0., 0., 1., 1.), 0.6).opacity(flicker)),
        };
        line(window, &trunk, width, color);
        line(window, &fork, width * 0.7, color);
    }
}

/// Beams sweeping up from the bottom edge, flaring with the middle of the spectrum.
fn lasers(area: Area, frame: &Frame, glow: bool, window: &mut Window) {
    let length = (area.width * area.width + area.height * area.height).sqrt();
    for (foot, share) in LASER_FEET.into_iter().enumerate() {
        for twin in 0..2 {
            let k = (foot * 2 + twin) as f32;
            let angle =
                -FRAC_PI_2 + (frame.time * (0.4 + 0.13 * k) + k * 1.3).sin() * 0.75 * frame.reach;
            let from = (area.left + area.width * share, area.top + area.height + 2.);
            let (cos, sin) = (angle.cos(), angle.sin());
            let to = (from.0 + cos * length, from.1 + sin * length);
            let flare = 6. + frame.mid * 18. * frame.reach;
            let (nx, ny) = (-sin, cos);
            let color = frame.at(k / 6.);
            let spread = match glow {
                true => flare * 1.8,
                false => flare,
            };
            shape(
                window,
                &[
                    (from.0 + nx, from.1 + ny),
                    (to.0 + nx * spread, to.1 + ny * spread),
                    (to.0 - nx * spread, to.1 - ny * spread),
                    (from.0 - nx, from.1 - ny),
                ],
                color.opacity(match glow {
                    true => 0.25,
                    false => 0.12 * (0.4 + frame.level),
                }),
            );
            if !glow {
                line(
                    window,
                    &[from, to],
                    1.2,
                    mix(color, hsla(0., 0., 1., 1.), 0.4)
                        .opacity((0.4 + frame.high * 1.2).min(0.9)),
                );
                disc(window, from, 4., color.opacity(0.8));
            }
        }
    }
}

/// Seconds since the first scene was drawn.
pub(crate) fn clock() -> f32 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f32()
}

/// A steady pseudo-random number from 0 to 1 for `seed`.
fn random(seed: u32) -> f32 {
    let mut value = seed.wrapping_mul(0x9E37_79B9).wrapping_add(0x7F4A_7C15);
    value ^= value >> 15;
    value = value.wrapping_mul(0x2C1B_3C6D);
    value ^= value >> 12;
    value = value.wrapping_mul(0x297A_2D39);
    value ^= value >> 15;
    (value >> 8) as f32 / (1u32 << 24) as f32
}

/// `from` fading into `to` by `share`, the hue going the short way round.
fn mix(from: Hsla, to: Hsla, share: f32) -> Hsla {
    let mut turn = to.h - from.h;
    if turn > 0.5 {
        turn -= 1.;
    } else if turn < -0.5 {
        turn += 1.;
    }
    Hsla {
        h: (from.h + turn * share).rem_euclid(1.),
        s: from.s + (to.s - from.s) * share,
        l: from.l + (to.l - from.l) * share,
        a: from.a + (to.a - from.a) * share,
    }
}

/// The edge of `area` with corners rounded by `radius`, closed, going clockwise.
fn perimeter(area: Area, radius: f32) -> Vec<(f32, f32)> {
    let radius = radius.min(area.width / 2.).min(area.height / 2.).max(0.);
    let (left, top) = (area.left, area.top);
    let (right, bottom) = (left + area.width, top + area.height);
    let corners = [
        ((right - radius, top + radius), -FRAC_PI_2),
        ((right - radius, bottom - radius), 0.),
        ((left + radius, bottom - radius), FRAC_PI_2),
        ((left + radius, top + radius), PI),
    ];
    let mut points = Vec::new();
    for ((x, y), from) in corners {
        for step in 0..=8 {
            let angle = from + FRAC_PI_2 * step as f32 / 8.;
            points.push((x + radius * angle.cos(), y + radius * angle.sin()));
        }
    }
    if let Some(first) = points.first().copied() {
        points.push(first);
    }
    points
}

/// The line through `points` cut into `pieces` stretches of equal length.
fn resample(points: &[(f32, f32)], pieces: usize) -> Vec<(f32, f32)> {
    let lengths: Vec<f32> = points
        .windows(2)
        .map(|pair| ((pair[1].0 - pair[0].0).powi(2) + (pair[1].1 - pair[0].1).powi(2)).sqrt())
        .collect();
    let total: f32 = lengths.iter().sum();
    if total <= 0. || pieces == 0 {
        return points.to_vec();
    }
    let mut out = Vec::with_capacity(pieces + 1);
    let (mut segment, mut walked) = (0, 0.);
    for piece in 0..=pieces {
        let goal = total * piece as f32 / pieces as f32;
        while segment < lengths.len() - 1 && walked + lengths[segment] < goal {
            walked += lengths[segment];
            segment += 1;
        }
        let share = match lengths[segment] > 0. {
            true => ((goal - walked) / lengths[segment]).clamp(0., 1.),
            false => 0.,
        };
        let (from, to) = (points[segment], points[segment + 1]);
        out.push((
            from.0 + (to.0 - from.0) * share,
            from.1 + (to.1 - from.1) * share,
        ));
    }
    out
}

fn at(place: (f32, f32)) -> Point<Pixels> {
    point(px(place.0), px(place.1))
}

fn line(window: &mut Window, points: &[(f32, f32)], width: f32, color: Hsla) {
    if points.len() < 2 || color.a <= 0. {
        return;
    }
    let mut builder = PathBuilder::stroke(px(width));
    builder.move_to(at(points[0]));
    for place in &points[1..] {
        builder.line_to(at(*place));
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

fn shape(window: &mut Window, points: &[(f32, f32)], color: impl Into<Background>) {
    if points.len() < 3 {
        return;
    }
    let mut builder = PathBuilder::fill();
    let points: Vec<Point<Pixels>> = points.iter().copied().map(at).collect();
    builder.add_polygon(&points, true);
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

fn rounded(window: &mut Window, from: (f32, f32), extent: (f32, f32), radius: f32, color: Hsla) {
    if color.a <= 0. {
        return;
    }
    window.paint_quad(quad(
        Bounds::new(at(from), size(px(extent.0), px(extent.1))),
        px(radius),
        color,
        px(0.),
        transparent_black(),
        BorderStyle::Solid,
    ));
}

fn disc(window: &mut Window, middle: (f32, f32), radius: f32, color: Hsla) {
    rounded(
        window,
        (middle.0 - radius, middle.1 - radius),
        (radius * 2., radius * 2.),
        radius,
        color,
    );
}

fn circle(window: &mut Window, middle: (f32, f32), radius: f32, width: f32, color: Hsla) {
    if color.a <= 0. {
        return;
    }
    window.paint_quad(quad(
        Bounds::new(
            at((middle.0 - radius, middle.1 - radius)),
            size(px(radius * 2.), px(radius * 2.)),
        ),
        px(radius),
        transparent_black(),
        px(width),
        color,
        BorderStyle::Solid,
    ));
}

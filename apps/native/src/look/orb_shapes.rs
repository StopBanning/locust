//! Geometry from thinking-orbs 0.3.1 (engine: globe, rubik, wave, web, braid, ribbon, ring, morph and orbits).
//!
//! Copyright (c) 2026 Jakub Antalik
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Shape {
    Globe,
    Rubik,
    Wave,
    Web,
    Braid,
    Ribbon,
    Ring,
    Morph,
    Orbits,
}

pub(super) struct Preset {
    pub(super) shape: Shape,
    pub(super) size: u32,
    pub(super) speed: f64,
}

struct Tuning {
    density: f64,
    radius: f64,
}

impl Preset {
    pub(super) fn frame(&self, time: f64) -> Frame {
        let tuning = match (self.shape, self.size) {
            (Shape::Globe, _) => Tuning {
                density: 0.42,
                radius: 1.15,
            },
            (Shape::Rubik, _) => Tuning {
                density: 0.35,
                radius: 1.05,
            },
            (Shape::Wave, 20) => Tuning {
                density: 0.105,
                radius: 1.6,
            },
            (Shape::Wave, _) => Tuning {
                density: 0.341,
                radius: 1.0,
            },
            (Shape::Web, _) => Tuning {
                density: 1.35,
                radius: 0.95,
            },
            (Shape::Braid, _) => Tuning {
                density: 0.5,
                radius: 1.0,
            },
            (Shape::Ribbon, _) => Tuning {
                density: 0.25,
                radius: 0.85,
            },
            (Shape::Ring, _) => Tuning {
                density: 0.028,
                radius: 1.622,
            },
            (Shape::Morph, _) => Tuning {
                density: 0.53,
                radius: 1.011,
            },
            (Shape::Orbits, _) => Tuning {
                density: 0.238,
                radius: 2.4,
            },
        };
        let mut frame = Frame {
            size: f64::from(self.size),
            ..Frame::default()
        };
        match self.shape {
            Shape::Globe | Shape::Rubik | Shape::Wave => {
                sphere(&mut frame, self.shape, time, tuning)
            }
            Shape::Web => web(&mut frame, time, tuning),
            Shape::Braid => braid(&mut frame, time, tuning),
            Shape::Ribbon | Shape::Ring => {
                ribbon(&mut frame, time, tuning, self.shape == Shape::Ring)
            }
            Shape::Morph => morph(&mut frame, time, tuning),
            Shape::Orbits => orbits(&mut frame, time, tuning),
        }
        frame
            .dots
            .sort_by(|a, b| a.center[2].total_cmp(&b.center[2]));
        frame
    }
}

type Point = [f64; 3];

#[derive(Default)]
pub(super) struct Frame {
    pub(super) size: f64,
    pub(super) dots: Vec<Dot>,
    pub(super) lines: Vec<Line>,
}

pub(super) struct Dot {
    pub(super) center: Point,
    pub(super) radius: f64,
    pub(super) shade: f64,
    pub(super) alpha: f64,
}

pub(super) struct Line {
    pub(super) from: Point,
    pub(super) to: Point,
    pub(super) width: f64,
    pub(super) alpha: f64,
}

impl Frame {
    fn dot(&mut self, center: Point, radius: f64, shade: f64, alpha: f64) {
        if alpha >= 0.02 {
            self.dots.push(Dot {
                center,
                radius: radius.max(0.3),
                shade,
                alpha,
            });
        }
    }

    fn radius_scale(&self) -> f64 {
        (self.size / 300.0).powf(0.6)
    }

    fn camera(&self, yaw: f64, pitch: f64, scale: f64) -> impl Fn(Point) -> Point + use<> {
        let center = self.size / 2.0;
        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = pitch.sin_cos();
        move |[x, y, z]| {
            let depth = -x * sy + z * cy;
            [
                center + (x * cy + z * sy) * scale,
                center - (y * cp - depth * sp) * scale,
                y * sp + depth * cp,
            ]
        }
    }
}

fn unit(index: usize, count: usize) -> f64 {
    index as f64 / count as f64
}

fn count(base: usize, factor: f64) -> usize {
    ((base as f64 * factor).round() as usize).max(1)
}

fn depth(value: f64) -> f64 {
    (value + 1.0) / 2.0
}

fn hash(x: f64, y: f64) -> f64 {
    ((x * 12.9898 + y * 78.233).sin() * 43758.5453).rem_euclid(1.0)
}

fn scale(point: Point, factor: f64) -> Point {
    point.map(|component| component * factor)
}

fn radial(angle: f64, y: f64, radius: f64) -> Point {
    [angle.cos() * radius, y, angle.sin() * radius]
}

fn fibonacci(index: usize, total: usize) -> Point {
    let y = 1.0 - 2.0 * (index as f64 + 0.5) / total as f64;
    radial(
        index as f64 * PI * (3.0 - 5.0_f64.sqrt()),
        y,
        (1.0 - y * y).sqrt(),
    )
}

fn ghost_sphere(frame: &mut Frame, camera: &impl Fn(Point) -> Point, points: usize, radius: f64) {
    for index in 0..points {
        let point = camera(scale(fibonacci(index, points), radius));
        frame.dot(
            point,
            0.8 * frame.radius_scale(),
            0.78,
            0.1 + 0.22 * depth(point[2] / radius),
        );
    }
}

// The Rubik's latitude dots pass through 14 fixed slice rotations, then unwind in reverse.
fn rubik_moves(time: f64) -> [(usize, f64, f64, bool); 14] {
    let phase = time.rem_euclid(28.0 * 0.42 + 1.2);
    let turn = (phase / 0.42).floor() as usize;
    let progress = 1.0 - (1.0 - ((phase / 0.42).fract() / 0.7).min(1.0)).powi(3);
    std::array::from_fn(|index| {
        let id = index as f64;
        let axis = (hash(id, 2.3) * 3.0).floor() as usize;
        let lower = (hash(id, 5.9) * 4.0).floor() * 0.5 - 1.0;
        let direction = if hash(id, 7.7) < 0.5 { 1.0 } else { -1.0 };
        let (amount, active) = if turn < 14 {
            (
                if index < turn {
                    1.0
                } else if index == turn {
                    progress
                } else {
                    0.0
                },
                index == turn,
            )
        } else if turn < 28 {
            let undo = 27 - turn;
            (
                if index < undo {
                    1.0
                } else if index == undo {
                    1.0 - progress
                } else {
                    0.0
                },
                index == undo,
            )
        } else {
            (0.0, false)
        };
        (axis, lower, amount * direction * FRAC_PI_2, active)
    })
}

fn scramble(mut point: Point, moves: &[(usize, f64, f64, bool)]) -> (Point, bool) {
    let mut turning = false;
    for &(axis, lower, angle, active) in moves {
        if angle == 0.0 || !(lower..lower + 0.5).contains(&point[axis]) {
            continue;
        }
        let (sine, cosine) = angle.sin_cos();
        let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
        (point[a], point[b]) = (
            point[a] * cosine - point[b] * sine,
            point[a] * sine + point[b] * cosine,
        );
        turning |= active;
    }
    (point, turning)
}

fn sphere(frame: &mut Frame, shape: Shape, time: f64, tuning: Tuning) {
    let globe = shape == Shape::Globe;
    let wave = shape == Shape::Wave;
    let radius = frame.size / 2.0 * if wave { 0.874 } else { 0.82 };
    let (yaw, pitch) = match shape {
        Shape::Globe => (time * 0.5, 0.4 + 0.06 * (time * 0.35).sin()),
        Shape::Wave => (time * 0.18, 0.38),
        _ => (time * 0.55, 0.35 + 0.1 * (time * 0.9).sin()),
    };
    let project = frame.camera(yaw, pitch, radius);
    let rings = count(if globe { 17 } else { 15 }, tuning.density.sqrt()).max(2);
    let density = count(if globe { 44 } else { 40 }, tuning.density.sqrt()).max(2);
    let moves = if shape == Shape::Rubik {
        Some(rubik_moves(time))
    } else {
        None
    };
    let size = tuning.radius * frame.radius_scale();

    for ring in 0..=rings {
        let latitude = -FRAC_PI_2 + unit(ring, rings) * PI;
        let (y, ring_radius) = latitude.sin_cos();
        let dots = count(density, ring_radius.abs());
        let wave_height = if wave {
            0.62 * (time * 2.1 - ring as f64 * 0.52).sin()
                + 0.38 * (time * 1.27 + ring as f64 * 0.83).sin()
        } else {
            0.0
        };
        for index in 0..dots {
            let longitude = unit(index, dots) * TAU;
            let point = radial(longitude, y, ring_radius);
            let (point, turning) = moves
                .as_ref()
                .map_or((point, false), |m| scramble(point, m));
            let drawn = if wave {
                scale(point, 0.88 + 0.105 * wave_height)
            } else {
                point
            };
            let center = project(drawn);
            let z = depth(center[2]);
            let scan = if globe {
                let meridian = longitude + time * 0.5 - time * (0.5 + 1.2 * 4.08);
                let offset = meridian.sin().atan2(meridian.cos());
                (-offset * offset / 0.18).exp() * center[2].max(0.0)
            } else {
                0.0
            };
            let crest = wave_height.max(0.0);
            let active = if turning { 1.0 } else { 0.0 };
            let radius = ((0.6 + 1.7 * z + 0.3 * active) * size + scan * frame.radius_scale())
                * (1.0 + 0.4 * crest);
            let shade = if wave {
                0.66 - 0.56 * z - 0.1 * crest
            } else {
                0.62 - 0.54 * z - 0.14 * active
            };
            frame.dot(
                center,
                radius,
                shade,
                if globe {
                    0.45 + 0.55 * scan.min(1.0)
                } else {
                    1.0
                },
            );
        }
    }
}

fn braid(frame: &mut Frame, time: f64, tuning: Tuning) {
    let radius = frame.size / 2.0 * 0.76;
    let project = frame.camera(time * 0.4, 0.3, radius);
    let size = frame.radius_scale() * tuning.radius;
    let points = count(52, tuning.density);
    ghost_sphere(
        frame,
        &frame.camera(time * 0.4, 0.3, 1.0),
        count(150, tuning.density),
        radius,
    );
    for strand in 0..3 {
        let phase = unit(strand, 3) * TAU;
        for index in 0..points {
            let y = ((unit(index, points) + time * 0.045).rem_euclid(1.0) * 2.0 - 1.0) * 0.96;
            let angle = y * PI * 3.0 + phase;
            let ripple = 1.0 + 0.075 * (angle * 2.0 + time * 0.8).sin();
            let center = project(scale(radial(angle, y, (1.0 - y * y).sqrt()), ripple));
            let z = depth(center[2]);
            let fade = ((1.0 - y.abs()) / 0.1).min(1.0);
            frame.dot(
                center,
                (1.2 + 1.8 * z) * size,
                0.55 - 0.45 * z,
                fade * (0.45 + 0.55 * z),
            );
        }
    }
}

fn ribbon(frame: &mut Frame, time: f64, tuning: Tuning, ring: bool) {
    let radius = frame.size / 2.0 * 0.78;
    let project = frame.camera(0.0, 0.3, 1.0);
    let size = tuning.radius * frame.radius_scale();
    if !ring {
        ghost_sphere(frame, &project, count(150, tuning.density), radius);
    }
    let band = if ring { 3.968 } else { 3.9 };
    let wobble = if ring { 0.565 } else { 1.0 };
    let distance = if ring {
        radius / (1.0 + 0.85 * 0.23 * wobble)
    } else {
        radius
    };
    let (sin_tilt, cos_tilt) = if ring { -0.3_f64 } else { 0.55_f64 }.sin_cos();
    let lanes = count(count(5, tuning.density.sqrt()).max(2), band);
    let points = count(88, tuning.density.sqrt()).max(2);
    let middle = (lanes - 1) as f64 / 2.0;
    for lane in 0..lanes {
        let offset = lane as f64 - middle;
        let rim = offset.abs() / middle.max(1.0);
        for index in 0..points {
            let angle = unit(index, points) * TAU;
            let displacement = (0.16 * (angle * 3.0 - time * 1.7 + lane as f64 * 0.22).sin()
                + 0.07 * (angle * 5.0 + time * 1.1).sin())
                * wobble;
            let across = offset * 0.075 + if ring { 0.0 } else { displacement };
            let (y, x) = angle.sin_cos();
            let factor = distance * if ring { 1.0 + displacement } else { 1.0 }
                / (1.0 + across * across).sqrt();
            let center = project(scale(
                [
                    x,
                    cos_tilt * y - sin_tilt * across,
                    sin_tilt * y + cos_tilt * across,
                ],
                factor,
            ));
            let z = depth(center[2] / radius);
            frame.dot(
                center,
                (1.1 + 1.7 * z) * (1.0 - 0.25 * rim) * size,
                0.52 - 0.44 * z + 0.18 * rim,
                0.4 + 0.6 * z,
            );
        }
    }
}

fn mix(from: f64, to: f64, amount: f64) -> f64 {
    from + (to - from) * amount
}

fn smooth(amount: f64) -> f64 {
    amount * amount * (3.0 - 2.0 * amount)
}

fn noise(x: f64, y: f64) -> f64 {
    let (column, row) = (x.floor(), y.floor());
    let across = smooth(x.fract());
    let near = mix(hash(column, row), hash(column + 1.0, row), across);
    let far = mix(
        hash(column, row + 1.0),
        hash(column + 1.0, row + 1.0),
        across,
    );
    mix(near, far, smooth(y.fract()))
}

fn length(point: Point) -> f64 {
    point.iter().map(|value| value * value).sum::<f64>().sqrt()
}

fn web(frame: &mut Frame, time: f64, tuning: Tuning) {
    let project = frame.camera(time * 0.12, 0.32, frame.size / 2.0 * 0.8);
    let size = frame.radius_scale() * tuning.radius;
    let total = count(30, tuning.density);
    let nodes: Vec<Point> = (0..total)
        .map(|index| {
            let id = index as f64;
            let [x, y, z] = fibonacci(index, total);
            let moved = [
                x + 0.6 * (noise(id * 0.31 + 9.0, time * 0.24) - 0.5),
                y + 0.6 * (noise(id * 0.53 + 27.0, time * 0.21) - 0.5),
                z + 0.6 * (noise(id * 0.77 + 55.0, time * 0.27) - 0.5),
            ];
            scale(moved, 1.0 / length(moved))
        })
        .collect();
    let projected: Vec<Point> = nodes.iter().copied().map(&project).collect();
    for (index, &from) in projected.iter().enumerate() {
        for (other, &to) in projected.iter().enumerate().skip(index + 1) {
            let difference = std::array::from_fn(|axis| nodes[index][axis] - nodes[other][axis]);
            let distance = length(difference);
            let alpha = (1.0 - distance / 0.72) * (0.3 + 0.55 * depth((from[2] + to[2]) / 2.0));
            if alpha >= 0.02 {
                frame.lines.push(Line {
                    from,
                    to,
                    width: (0.8 * frame.radius_scale()).max(0.6),
                    alpha,
                });
            }
        }
        let z = depth(from[2]);
        let pulse = 1.0 + 0.25 * (time * 1.4 + index as f64 * 2.7).sin();
        frame.dot(from, (1.4 + 1.8 * z) * pulse * size, 0.55 - 0.45 * z, 1.0);
    }
    for index in 0..count(5, tuning.density) {
        let id = index as f64;
        let phase = time * 0.55 + id * 7.31;
        let start = (hash(phase.floor(), id * 3.1 + 1.7) * total as f64) as usize;
        let end = (hash(phase.floor(), id * 5.7 + 4.2) * total as f64) as usize;
        if start != end {
            let point = std::array::from_fn(|axis| {
                mix(nodes[start][axis], nodes[end][axis], phase.fract())
            });
            let center = project(scale(point, 1.0 / length(point).max(1e-6)));
            let z = depth(center[2]);
            frame.dot(center, (2.1 + 1.8 * z) * size, 0.05, 0.5 + 0.5 * z);
        }
    }
}

fn outline(points: &[[f64; 2]], fraction: f64) -> [f64; 2] {
    let edges = points.iter().zip(points.iter().cycle().skip(1));
    let distance = |a: &[f64; 2], b: &[f64; 2]| (b[0] - a[0]).hypot(b[1] - a[1]);
    let mut left = fraction * edges.clone().map(|(a, b)| distance(a, b)).sum::<f64>();
    for (a, b) in edges {
        let edge = distance(a, b);
        if left <= edge {
            let t = if edge == 0.0 { 0.0 } else { left / edge };
            return [mix(a[0], b[0], t), mix(a[1], b[1], t)];
        }
        left -= edge;
    }
    points[0]
}

fn morph(frame: &mut Frame, time: f64, tuning: Tuning) {
    let phase = time.rem_euclid(6.9);
    let shape = (phase / 2.3).floor() as usize;
    let local = phase - shape as f64 * 2.3;
    let blend = smooth(((local - 1.4) / 0.9).clamp(0.0, 1.0));
    let vertex = |shape, u| match shape {
        0 => {
            let angle: f64 = -FRAC_PI_2 + u * TAU;
            [angle.cos() * 0.24, angle.sin() * 0.24]
        }
        1 => outline(&[[0.0, -0.26], [0.24, 0.16], [-0.24, 0.16]], u),
        _ => outline(
            &[
                [0.0, -0.2],
                [0.2, -0.2],
                [0.2, 0.2],
                [-0.2, 0.2],
                [-0.2, -0.2],
            ],
            u,
        ),
    };
    let points: Vec<_> = (0..160)
        .map(|index| {
            let from = vertex(shape, unit(index, 160));
            let to = vertex((shape + 1) % 3, unit(index, 160));
            std::array::from_fn(|axis| mix(from[axis], to[axis], blend))
        })
        .collect();
    let puff = (1.0 + 0.02 * (local * 3.1).sin()) * 1.45;
    let radius = (0.021 * tuning.radius * 1.35 * 1.45 * frame.size).max(0.35);
    let dots = count(34, tuning.density).max(6);
    for index in 0..dots {
        let [x, y] = outline(&points, unit(index, dots));
        frame.dot(
            [
                frame.size * (0.5 + x * puff),
                frame.size * (0.5 + y * puff),
                0.0,
            ],
            radius,
            0.1,
            1.0,
        );
    }
}

fn orbits(frame: &mut Frame, time: f64, tuning: Tuning) {
    let radius = frame.size / 2.0 * 0.82;
    let project = frame.camera(time * 0.12, 0.3, 1.0);
    let size = frame.radius_scale() * tuning.radius;
    for index in 0..count(12, tuning.density) {
        let id = index as f64;
        let axis = hash(id, 1.7);
        let tilt = hash(id, 5.2);
        let motion = hash(id, 8.9);
        let radius = radius * (0.45 + 0.52 * axis);
        let y = 2.0 * tilt - 1.0;
        let [x, y, z] = radial(axis * TAU, y, (1.0 - y * y).sqrt());
        let norm = x.hypot(y).max(1e-6);
        let along = [-y / norm, x / norm, 0.0];
        let across = [-z * along[1], z * along[0], x * along[1] - y * along[0]];
        let orbit = |angle: f64| {
            let (sine, cosine) = angle.sin_cos();
            project(std::array::from_fn(|axis| {
                radius * (along[axis] * cosine + across[axis] * sine)
            }))
        };
        let ghosts = count(40, tuning.density);
        for point in 0..ghosts {
            let center = orbit(unit(point, ghosts) * TAU);
            frame.dot(
                center,
                0.9 * size,
                0.72,
                0.5 * (0.4 + 0.6 * depth(center[2] / radius)),
            );
        }
        let speed = (0.25 + 0.55 * motion) * if motion > 0.5 { 1.0 } else { -1.0 };
        for point in 0..3 {
            let center = orbit(time * speed + unit(point, 3) * TAU + tilt * 6.0);
            let z = depth(center[2] / radius);
            frame.dot(center, (1.2 + 1.6 * z) * size, 0.3 - 0.22 * z, 1.0);
        }
    }
}

//! Seeded hoards and hand-drawn stories. Small dressing stays off traversal routes.
use super::*;
impl Workshop {
    pub(super) fn ring(&mut self, mat: usize, p: Vec3, radius: f32, q: Quat, thickness: f32) {
        for i in 0..20 {
            let a = i as f32 * TAU / 20.;
            let b = (i + 1) as f32 * TAU / 20.;
            self.beam(
                mat,
                p + q * Vec3::new(a.cos(), 0., a.sin()) * radius,
                p + q * Vec3::new(b.cos(), 0., b.sin()) * radius,
                thickness,
            );
        }
    }
    pub(super) fn bone(&mut self, p: Vec3, q: Quat, length: f32) {
        self.beam(BONE, p, p + q * Vec3::Z * length, 0.013);
        for z in [0., length] {
            for x in [-0.012, 0.012] {
                self.shape(
                    BONE,
                    Sphere::new(0.018).mesh().uv(8, 6),
                    p + q * Vec3::new(x, 0., z),
                    Vec3::ONE,
                    q,
                    Vec3::ONE,
                    false,
                );
            }
        }
    }
    pub(super) fn trinket(&mut self, p: Vec3, q: Quat) {
        let turn = q * Quat::from_rotation_y(self.rng.range(-3., 3.));
        let size = self.rng.range(0.8, 1.3);
        match (self.rng.unit() * 9.).min(8.) as usize {
            0 => {
                // Mismatched narrow bottles with corks and wrapped necks.
                self.pot(p, 0.047 * size, 0.13 * size);
                self.beam(
                    WOOD,
                    p + Vec3::Y * 0.13 * size,
                    p + Vec3::Y * 0.165 * size,
                    0.019,
                );
                self.ring(ROPE, p + Vec3::Y * 0.115 * size, 0.028, turn, 0.006);
            }
            1 => {
                self.bone(p + Vec3::Y * 0.025, turn, 0.14 * size);
                self.bone(
                    p + Vec3::new(0.035, 0.05, 0.015),
                    turn * Quat::from_rotation_y(1.2),
                    0.11 * size,
                );
            }
            2 => {
                // Pilfered key ring, each key has a bow, shaft and uneven teeth.
                self.ring(IRON, p + Vec3::Y * 0.012, 0.05, turn, 0.007);
                for i in 0..3 {
                    let q = turn * Quat::from_rotation_y(i as f32 * 0.8);
                    let a = p + q * Vec3::new(0., 0.018, 0.05);
                    self.ring(IRON, a, 0.018, q, 0.004);
                    self.beam(IRON, a, a + q * Vec3::Z * 0.10, 0.006);
                    for z in [0.07, 0.095] {
                        self.beam(
                            IRON,
                            a + q * Vec3::Z * z,
                            a + q * Vec3::new(0.022, 0., z),
                            0.005,
                        );
                    }
                }
            }
            3 => {
                // Unequal little coins and stone game counters.
                for i in 0..7 {
                    let offset = Vec3::new(
                        self.rng.range(-0.065, 0.065),
                        0.006 + (i % 3) as f32 * 0.007,
                        self.rng.range(-0.055, 0.055),
                    );
                    self.shape(
                        if i % 3 == 0 { BONE } else { IRON },
                        Cylinder::new(0.025, 0.008).mesh().resolution(12).build(),
                        p + offset,
                        Vec3::ONE,
                        turn,
                        Vec3::ONE,
                        false,
                    );
                }
            }
            4 => {
                // Gathered mushrooms in an open earthen bowl.
                self.pot(p, 0.082, 0.065);
                for _ in 0..5 {
                    let offset = Vec3::new(
                        self.rng.range(-0.035, 0.035),
                        0.05,
                        self.rng.range(-0.035, 0.035),
                    );
                    let height = self.rng.range(0.045, 0.09);
                    self.beam(BONE, p + offset, p + offset + Vec3::Y * height, 0.008);
                    self.shape(
                        CLAY,
                        Sphere::new(1.).mesh().uv(12, 8),
                        p + offset + Vec3::Y * height,
                        Vec3::new(0.03, 0.013, 0.03),
                        turn,
                        Vec3::new(0.82, 0.63, 0.43),
                        false,
                    );
                }
            }
            5 => {
                // A wrapped hide roll, lashed rather than neatly folded.
                self.shape(
                    CLOTH,
                    Cylinder::new(0.043, 0.16).mesh().resolution(12).build(),
                    p + Vec3::Y * 0.045,
                    Vec3::ONE,
                    turn * Quat::from_rotation_x(1.57),
                    Vec3::new(1.3, 1.1, 0.7),
                    false,
                );
                for z in [-0.045, 0.04] {
                    self.ring(
                        ROPE,
                        p + turn * Vec3::new(0., 0.045, z),
                        0.046,
                        turn * Quat::from_rotation_x(1.57),
                        0.004,
                    );
                }
            }
            6 => {
                // Stone-headed mallet and battered metal knife.
                self.beam(
                    WOOD,
                    p + Vec3::Y * 0.025,
                    p + turn * Vec3::new(0., 0.025, 0.17),
                    0.014,
                );
                self.block(
                    STONE,
                    p + turn * Vec3::new(0., 0.04, 0.16),
                    Vec3::new(0.105, 0.062, 0.052),
                    turn,
                    false,
                );
                self.block(
                    IRON,
                    p + turn * Vec3::new(0.075, 0.01, 0.04),
                    Vec3::new(0.028, 0.008, 0.12),
                    turn,
                    false,
                );
            }
            7 => {
                // A cracked vessel surrounded by shards.
                self.pot(p, 0.058 * size, 0.055);
                for _ in 0..4 {
                    let offset = Vec3::new(
                        self.rng.range(-0.095, 0.095),
                        0.008,
                        self.rng.range(-0.09, 0.09),
                    );
                    self.shape(
                        CLAY,
                        Tetrahedron::default().mesh().build(),
                        p + offset,
                        Vec3::new(0.035, 0.009, 0.028),
                        turn,
                        Vec3::ONE,
                        false,
                    );
                }
            }
            _ => {
                // A crudely carved face fetish with oversized ears and nail eyes.
                self.shape(
                    WOOD,
                    Sphere::new(1.).mesh().uv(12, 10),
                    p + Vec3::Y * 0.08,
                    Vec3::new(0.045, 0.075, 0.033),
                    turn,
                    Vec3::ONE,
                    false,
                );
                for sign in [-1., 1.] {
                    self.shape(
                        WOOD,
                        Cone::new(0.02, 0.07).mesh().resolution(7).build(),
                        p + turn * Vec3::new(sign * 0.043, 0.12, 0.),
                        Vec3::ONE,
                        turn * Quat::from_rotation_z(-sign * 0.65),
                        Vec3::ONE,
                        false,
                    );
                    self.shape(
                        BONE,
                        Sphere::new(0.008).mesh().uv(8, 6),
                        p + turn * Vec3::new(sign * 0.017, 0.10, 0.03),
                        Vec3::ONE,
                        turn,
                        Vec3::ONE,
                        false,
                    );
                }
            }
        }
    }
    // Thin pigment ribbons, attached to the wall, with irregular brush edges.
    fn stroke(&mut self, origin: Vec3, right: Vec3, points: &[Vec2], color: Vec3, width: f32) {
        for pair in points.windows(2) {
            let delta = pair[1] - pair[0];
            if delta.length_squared() < 1e-8 {
                continue;
            }
            let side = Vec2::new(-delta.y, delta.x).normalize() * width;
            let tone = color * self.rng.range(0.72, 1.);
            let g = &mut self.g[PAINT];
            let first = g.positions.len() as u32;
            for p in [
                pair[0] - side,
                pair[1] - side,
                pair[1] + side,
                pair[0] + side,
            ] {
                g.vertex(origin + right * p.x + Vec3::Y * p.y, p, tone);
            }
            g.triangle(first, first + 1, first + 2);
            g.triangle(first, first + 2, first + 3);
        }
    }
    pub(super) fn drawing(&mut self, origin: Vec3, right: Vec3, size: f32, motif: usize) {
        let colors = [
            Vec3::new(0.83, 0.72, 0.49),
            Vec3::new(0.58, 0.18, 0.065),
            Vec3::new(0.12, 0.095, 0.065),
            Vec3::new(0.69, 0.63, 0.43),
        ];
        let color = colors[(self.rng.unit() * 4.).min(3.) as usize];
        let tilt = self.rng.range(-0.12, 0.12);
        let stretch = Vec2::new(self.rng.range(0.82, 1.12), self.rng.range(0.88, 1.12));
        let mirror = if self.rng.unit() < 0.3 { -1. } else { 1. };
        let draw = |w: &mut Self, coords: &[(f32, f32)]| {
            let points: Vec<_> = coords
                .iter()
                .map(|&(x, y)| {
                    let v = Vec2::new(x * mirror, y) * stretch * size;
                    Vec2::new(
                        v.x * tilt.cos() - v.y * tilt.sin(),
                        v.x * tilt.sin() + v.y * tilt.cos(),
                    ) + Vec2::new(w.rng.range(-0.006, 0.006), w.rng.range(-0.006, 0.006))
                })
                .collect();
            w.stroke(origin, right, &points, color, 0.009 * size.max(0.6));
        };
        if motif >= 5 {
            for line in pictographs::generate(motif, &mut self.rng) {
                let coords: Vec<_> = line.iter().map(|p| (p.x, p.y)).collect();
                draw(self, &coords);
            }
            return;
        }
        match motif {
            0 => {
                // Fang-filled grin, staring eyes and enormous pointed ears.
                draw(
                    self,
                    &[
                        (-0.28, 0.05),
                        (-0.46, 0.35),
                        (-0.19, 0.23),
                        (0., 0.30),
                        (0.19, 0.23),
                        (0.46, 0.35),
                        (0.28, 0.05),
                        (0.15, -0.23),
                        (-0.15, -0.23),
                        (-0.28, 0.05),
                    ],
                );
                for x in [-0.12, 0.12] {
                    draw(
                        self,
                        &[
                            (x - 0.05, 0.12),
                            (x, 0.16),
                            (x + 0.05, 0.10),
                            (x, 0.07),
                            (x - 0.05, 0.12),
                        ],
                    );
                }
                draw(
                    self,
                    &[
                        (-0.20, -0.05),
                        (-0.13, -0.14),
                        (-0.07, -0.04),
                        (0., -0.15),
                        (0.08, -0.04),
                        (0.14, -0.13),
                        (0.21, -0.04),
                    ],
                );
            }
            1 => {
                // Prey story: four-legged beast, antlers, arrows.
                draw(
                    self,
                    &[
                        (-0.35, 0.05),
                        (0.18, 0.10),
                        (0.3, 0.28),
                        (0.45, 0.23),
                        (0.34, 0.12),
                        (0.22, -0.07),
                        (-0.32, -0.07),
                        (-0.35, 0.05),
                        (-0.45, 0.17),
                    ],
                );
                for x in [-0.26, -0.13, 0.1, 0.22] {
                    draw(self, &[(x, -0.05), (x - 0.035, -0.27), (x + 0.025, -0.29)]);
                }
                draw(self, &[(0.3, 0.28), (0.25, 0.46), (0.13, 0.49)]);
                draw(self, &[(0.28, 0.39), (0.4, 0.47)]);
                draw(self, &[(-0.25, 0.43), (0.05, 0.20), (-0.06, 0.22)]);
            }
            2 => {
                // Spiralling tunnel map, branches and a crossed-out dead end.
                let points: Vec<_> = (0..40)
                    .map(|i| {
                        let a = i as f32 * 0.25;
                        let r = 0.03 + i as f32 * 0.007;
                        (a.cos() * r, a.sin() * r)
                    })
                    .collect();
                draw(self, &points);
                draw(self, &[(0.1, -0.2), (0.3, -0.34), (0.46, -0.28)]);
                draw(self, &[(0.38, -0.21), (0.50, -0.36)]);
                draw(self, &[(0.39, -0.36), (0.51, -0.21)]);
                draw(self, &[(-0.2, 0.1), (-0.4, 0.2), (-0.49, 0.38)]);
            }
            3 => {
                // Hoard counts, with crude five-stroke grouping.
                for group in 0..2 {
                    for i in 0..4 {
                        let x = -0.4 + group as f32 * 0.42 + i as f32 * 0.08;
                        draw(self, &[(x, -0.18), (x + 0.015, 0.20)]);
                    }
                    let x = -0.44 + group as f32 * 0.42;
                    draw(self, &[(x, -0.1), (x + 0.34, 0.13)]);
                }
            }
            _ => {
                // Palm print with crooked, unequal fingers.
                draw(
                    self,
                    &[
                        (-0.12, -0.13),
                        (-0.19, 0.02),
                        (-0.13, 0.15),
                        (0.1, 0.13),
                        (0.16, -0.04),
                        (0.06, -0.16),
                        (-0.12, -0.13),
                    ],
                );
                for (x, top) in [(-0.13, 0.27), (-0.055, 0.38), (0.02, 0.41), (0.10, 0.32)] {
                    draw(self, &[(x, 0.10), (x - 0.015, top)]);
                }
                draw(self, &[(-0.16, 0.01), (-0.29, 0.16)]);
            }
        }
    }
    fn wall_stories(&mut self, h: &House) {
        let q = Quat::from_rotation_y(h.yaw);
        let count = 35 + (self.rng.unit() * 46.).floor().min(45.) as usize;
        for i in 0..count {
            let wall = if i < 3 {
                0
            } else {
                (self.rng.unit() * 3.).floor().min(2.) as usize
            };
            let (p, right, size) = if wall == 0 {
                let x = self.rng.range(-h.width / 2. + 0.62, h.width / 2. - 0.62);
                let top = ((h.roof_height(x, -h.depth / 2.) - 0.46) * h.rise - 0.48).max(0.85);
                (
                    Vec3::new(x, self.rng.range(0.45, top), -h.depth / 2. + 0.055),
                    Vec3::X,
                    self.rng.range(0.25, 0.76),
                )
            } else {
                let side = if wall == 1 { -1. } else { 1. };
                let top = h.window.x * h.rise;
                let y = self.rng.range(0.22, top - 0.20);
                let size = self
                    .rng
                    .range(0.20, 0.52)
                    .min((y.min(top - y) - 0.02) / 0.7);
                (
                    Vec3::new(
                        side * (h.width / 2. - 0.054),
                        y,
                        self.rng.range(-h.depth / 2. + 0.5, h.depth / 2. - 0.5),
                    ),
                    Vec3::Z * -side,
                    size,
                )
            };
            let motif = match i {
                0 => 5 + (self.rng.unit() * 5.).floor().min(4.) as usize,
                1 => 10 + (self.rng.unit() * 6.).floor().min(5.) as usize,
                2 => 16 + (self.rng.unit() * 6.).floor().min(5.) as usize,
                _ => (self.rng.unit() * pictographs::COUNT as f32)
                    .floor()
                    .min((pictographs::COUNT - 1) as f32) as usize,
            };
            let origin = h.at + q * p;
            let right = q * right;
            self.drawing(origin, right, size, motif);
            // Some residents add a second symbol, making overlapping little stories.
            if self.rng.unit() < 0.28 {
                let companion = (self.rng.unit() * pictographs::COUNT as f32)
                    .floor()
                    .min((pictographs::COUNT - 1) as f32) as usize;
                self.drawing(
                    origin + right * size * 0.22 - Vec3::Y * size * 0.12,
                    right,
                    size * 0.40,
                    companion,
                );
            }
        }
    }
    pub(super) fn dress_house(&mut self, h: &House) {
        let q = Quat::from_rotation_y(h.yaw);
        let at = |v| h.at + q * v;
        // Shelves occupy sampled wall positions and heights, rather than rows.
        let mut shelves: Vec<Vec3> = Vec::new();
        let shelf_count = 4 + (self.rng.unit() * 6.).floor().min(5.) as usize;
        for _ in 0..shelf_count {
            for _ in 0..40 {
                let wall = (self.rng.unit() * 3.).floor().min(2.) as usize;
                let y = self.rng.range(0.32, 1.55);
                let (p, turn) = match wall {
                    0 => (
                        Vec3::new(
                            self.rng.range(-h.width / 2. + 0.70, h.width / 2. - 0.70),
                            y,
                            -h.depth / 2. + 0.23,
                        ),
                        0.,
                    ),
                    1 => (
                        Vec3::new(
                            -h.width / 2. + 0.23,
                            y,
                            self.rng.range(-h.depth / 2. + 0.65, h.depth / 2. - 0.65),
                        ),
                        std::f32::consts::FRAC_PI_2,
                    ),
                    _ => (
                        Vec3::new(
                            h.width / 2. - 0.23,
                            y,
                            self.rng.range(-h.depth / 2. + 0.65, h.depth / 2. - 0.65),
                        ),
                        -std::f32::consts::FRAC_PI_2,
                    ),
                };
                if shelves
                    .iter()
                    .any(|v| v.xz().distance(p.xz()) < 0.95 && (v.y - p.y).abs() < 0.45)
                {
                    continue;
                }
                shelves.push(p);
                let rot = Quat::from_rotation_y(turn);
                let transform = |v| at(p + rot * v);
                let length = self.rng.range(0.75, 1.25);
                self.block(
                    WOOD,
                    transform(Vec3::ZERO),
                    Vec3::new(length, 0.045, 0.33),
                    q * rot,
                    false,
                );
                for x in [-length * 0.38, length * 0.38] {
                    let root = p + rot * Vec3::new(x, 0., -0.10);
                    let root = root.with_y(h.roof_height(root.x, root.z) * h.rise - 0.05);
                    self.beam(ROPE, transform(Vec3::new(x, 0., 0.12)), at(root), 0.008);
                }
                let count = 4 + (self.rng.unit() * 5.).floor().min(4.) as usize;
                for i in 0..count {
                    let offset = Vec3::new(
                        ((i as f32 + 0.5) / count as f32 - 0.5) * length,
                        0.03,
                        self.rng.range(-0.07, 0.07),
                    );
                    self.trinket(transform(offset), q * rot);
                }
                break;
            }
        }
        for side in [-1., 1.] {
            // Open slatted salvage bins with contents above their rims.
            let bin = Vec3::new(side * (h.width / 2. - 0.35), 0., h.depth / 2. - 0.7);
            for i in 0..4 {
                let y = 0.04 + i as f32 * 0.06;
                for z in [-0.2, 0.2] {
                    self.block(
                        WOOD,
                        at(bin + Vec3::new(0., y, z)),
                        Vec3::new(0.44, 0.035, 0.025),
                        q,
                        false,
                    );
                }
                for x in [-0.21, 0.21] {
                    self.block(
                        WOOD,
                        at(bin + Vec3::new(x, y, 0.)),
                        Vec3::new(0.025, 0.035, 0.40),
                        q,
                        false,
                    );
                }
            }
            for _ in 0..(5 + (self.rng.unit() * 9.).floor().min(8.) as usize) {
                let offset = Vec3::new(
                    self.rng.range(-0.15, 0.15),
                    self.rng.range(0.12, 0.25),
                    self.rng.range(-0.13, 0.13),
                );
                self.trinket(at(bin + offset), q);
            }
            // Knotted garland of bones, teeth, rusty rings and dried fungus.
            for k in 0..9 {
                let z = -h.depth / 2. + 0.3 + k as f32 * (h.depth - 0.6) / 8.;
                let p = Vec3::new(side * (h.width / 2. - 0.16), 1.80, z);
                let drop = self.rng.range(0.15, 0.45);
                self.beam(ROPE, at(p), at(p - Vec3::Y * drop), 0.006);
                self.bone(at(p - Vec3::Y * drop), q * Quat::from_rotation_x(1.2), 0.11);
                self.ring(
                    IRON,
                    at(p - Vec3::Y * (drop * 0.6)),
                    0.035,
                    q * Quat::from_rotation_x(1.57),
                    0.006,
                );
            }
        }
        self.wall_stories(h);
        // Mess at the edges, leaving the four spawn positions and doorway open.
        for side in [-1., 1.] {
            for i in 0..7 {
                let z = -h.depth / 2. + 0.5 + i as f32 * (h.depth - 1.) / 6.;
                let x = side * (h.width / 2. - self.rng.range(0.15, 0.28));
                self.trinket(at(Vec3::new(x, 0.015, z)), q);
            }
        }
    }
}

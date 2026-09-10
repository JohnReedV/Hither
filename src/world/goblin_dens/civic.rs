//! Individually assembled communal buildings, connected to the settlement graph.
use super::*;
impl Workshop {
    pub(super) fn relic(&mut self, at: Vec3, radius: f32, entry: usize) {
        self.relics.push(lore::Relic { at, radius, entry });
    }
    fn candle(&mut self, p: Vec3, height: f32) {
        self.beam(BONE, p, p + Vec3::Y * height, 0.025);
        self.shape(
            GLOW,
            Sphere::new(1.).mesh().uv(8, 6),
            p + Vec3::Y * (height + 0.025),
            Vec3::new(0.011, 0.036, 0.011),
            Quat::IDENTITY,
            Vec3::ONE,
            false,
        );
        for _ in 0..3 {
            let a = self.rng.range(0., TAU);
            let y = self.rng.range(0.02, height);
            self.beam(
                BONE,
                p + Vec3::new(a.cos() * 0.025, y, a.sin() * 0.025),
                p + Vec3::new(a.cos() * 0.026, (y - 0.035).max(0.), a.sin() * 0.026),
                0.004,
            );
        }
    }
    fn mushroom(&mut self, p: Vec3, size: f32) {
        self.beam(BONE, p, p + Vec3::Y * size * 0.72, size * 0.09);
        self.shape(
            CLAY,
            Sphere::new(1.).mesh().uv(24, 12),
            p + Vec3::Y * size * 0.76,
            Vec3::new(size * 0.48, size * 0.16, size * 0.45),
            Quat::IDENTITY,
            Vec3::new(0.72, 0.58, 0.42),
            false,
        );
        for i in 0..24 {
            let a = i as f32 * TAU / 24.;
            self.beam(
                BONE,
                p + Vec3::Y * size * 0.65,
                p + Vec3::new(a.cos() * size * 0.43, size * 0.69, a.sin() * size * 0.40),
                size * 0.006,
            );
        }
    }
    pub(super) fn shrine(&mut self, h: &House, den: &Den) {
        self.structure(h, den, h.rise);
        self.dress_house(h);
        let q = Quat::from_rotation_y(h.yaw);
        let at = |v| h.at + q * v;
        let mut stations = [
            Vec3::new(
                -h.width * 0.30,
                0.,
                -h.depth / 2. + self.rng.range(0.88, 1.50),
            ),
            Vec3::new(
                self.rng.range(-0.07, 0.07),
                0.,
                -h.depth / 2. + self.rng.range(0.88, 1.50),
            ),
            Vec3::new(
                h.width * 0.30,
                0.,
                -h.depth / 2. + self.rng.range(0.88, 1.50),
            ),
        ];
        for i in (1..3).rev() {
            let j = (self.rng.unit() * (i + 1) as f32).floor().min(i as f32) as usize;
            stations.swap(i, j);
        }
        let back = stations[0].z;
        let altar_x = stations[0].x;
        // Layered stone altar with mortised timber, iron corners and offerings.
        for layer in 0..3 {
            self.block(
                STONE,
                at(Vec3::new(altar_x, 0.13 + layer as f32 * 0.21, back)),
                Vec3::new(2.15 - layer as f32 * 0.18, 0.22, 0.88 - layer as f32 * 0.07),
                q,
                true,
            );
        }
        let idol = stations[0] + Vec3::Y * 0.68;
        let arms = 2 + (self.rng.unit() * 3.).floor().min(2.) as usize;
        self.shape(
            WOOD,
            Sphere::new(1.).mesh().uv(20, 16),
            at(idol + Vec3::Y * 0.52),
            Vec3::new(0.26, 0.58, 0.20),
            q,
            Vec3::splat(0.8),
            false,
        );
        self.shape(
            WOOD,
            Sphere::new(1.).mesh().uv(24, 16),
            at(idol + Vec3::Y * 1.26),
            Vec3::new(0.34, 0.35, 0.22),
            q,
            Vec3::splat(0.85),
            false,
        );
        for side in [-1., 1.] {
            self.shape(
                WOOD,
                Cone::new(0.14, 0.52).mesh().resolution(9).build(),
                at(idol + Vec3::new(side * 0.32, 1.45, 0.)),
                Vec3::ONE,
                q * Quat::from_rotation_z(-side * 0.9),
                Vec3::ONE,
                false,
            );
            self.shape(
                BONE,
                Sphere::new(0.052).mesh().uv(12, 8),
                at(idol + Vec3::new(side * 0.13, 1.31, 0.20)),
                Vec3::new(1., 0.7, 0.45),
                q,
                Vec3::ONE,
                false,
            );
            for arm in 0..arms {
                let y = 0.22 + arm as f32 * 0.72 / (arms - 1) as f32;
                let elbow = Vec3::new(side * (0.45 + arm as f32 * 0.06), y + 0.12, 0.02);
                let palm = Vec3::new(side * (0.71 + arm as f32 * 0.04), y + 0.31, 0.14);
                self.beam(
                    WOOD,
                    at(idol + Vec3::new(side * 0.16, y, 0.)),
                    at(idol + elbow),
                    0.055,
                );
                self.beam(WOOD, at(idol + elbow), at(idol + palm), 0.043);
                for finger in 0..4 {
                    let offset = Vec3::new((finger as f32 - 1.5) * 0.025, 0., 0.);
                    self.beam(
                        WOOD,
                        at(idol + palm + offset),
                        at(idol + palm + offset + Vec3::new(side * 0.02, 0.13, 0.055)),
                        0.010,
                    );
                }
                self.ring(
                    IRON,
                    at(idol + palm + Vec3::Y * 0.03),
                    0.07,
                    q * Quat::from_rotation_x(1.57),
                    0.009,
                );
            }
        }
        for i in 0..7 {
            self.shape(
                BONE,
                Cone::new(0.022, 0.10).mesh().resolution(8).build(),
                at(idol + Vec3::new(-0.14 + i as f32 * 0.047, 1.13, 0.21)),
                Vec3::ONE,
                q * Quat::from_rotation_z(std::f32::consts::PI),
                Vec3::ONE,
                false,
            );
        }
        self.relic(at(idol + Vec3::Y * 1.1), 0.75, 0);
        // The Deep Maw: a tooth-lined rock aperture around a dark offering bowl.
        let maw = stations[1] + Vec3::Y * self.rng.range(0.90, 1.10);
        for i in 0..16 {
            let a = i as f32 * TAU / 16.;
            let b = (i + 1) as f32 * TAU / 16.;
            self.beam(
                STONE,
                at(maw + Vec3::new(a.cos() * 0.48, a.sin() * 0.54, 0.)),
                at(maw + Vec3::new(b.cos() * 0.48, b.sin() * 0.54, 0.)),
                0.11,
            );
            if i < 8 {
                self.shape(
                    BONE,
                    Cone::new(0.038, 0.20).mesh().resolution(9).build(),
                    at(maw + Vec3::new(a.cos() * 0.37, a.sin() * 0.40, 0.06)),
                    Vec3::ONE,
                    q * Quat::from_rotation_z(a + std::f32::consts::FRAC_PI_2),
                    Vec3::ONE,
                    false,
                );
            }
        }
        self.vessel(
            IRON,
            at(stations[1] + Vec3::new(0., 0.12, 0.25)),
            0.35,
            0.27,
        );
        self.relic(at(maw), 0.57, 1);
        // Spore Mother grows from a root-wrapped shrine of stolen bowls.
        let mother = stations[2] + Vec3::Y * 0.20;
        self.mushroom(at(mother), 1.28);
        for i in 0..7 {
            let a = i as f32 * TAU / 7.;
            let p = mother + Vec3::new(a.cos() * 0.39, 0., a.sin() * 0.30);
            let size = self.rng.range(0.18, 0.35);
            self.mushroom(at(p), size);
            self.beam(ROPE, at(mother), at(p), 0.016);
        }
        self.relic(at(mother + Vec3::Y * 0.7), 0.60, 2);
        // Kneeling hides flank a clear processional aisle; no human pews.
        for side in [-1., 1.] {
            for row in 0..3 {
                let p = Vec3::new(side * 1.25, 0.02, -0.35 + row as f32 * 0.72);
                self.hide(at(p), Vec2::new(0.26, 0.23), q);
                self.vessel(CLAY, at(p + Vec3::new(side * 0.37, 0., 0.)), 0.06, 0.045);
            }
            let x = side * (h.width / 2. - 0.37);
            // Bone genealogy ladders, each rung bearing a tiny ancestor mask.
            for dx in [-0.19, 0.19] {
                self.beam(
                    WOOD,
                    at(Vec3::new(x + dx, 0.08, -0.3)),
                    at(Vec3::new(x + dx, 1.62, -0.3)),
                    0.025,
                );
            }
            for rung in 0..6 {
                let y = 0.2 + rung as f32 * 0.23;
                self.bone(
                    at(Vec3::new(x - 0.19, y, -0.3)),
                    q * Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
                    0.38,
                );
                self.drawing(at(Vec3::new(x, y + 0.09, -0.27)), q * Vec3::X, 0.12, 0);
            }
            self.relic(
                at(Vec3::new(x, 0.85, -0.3)),
                0.42,
                if side < 0. { 3 } else { 6 },
            );
        }
        for i in 0..10 {
            let x = altar_x - 1. + i as f32 * 0.22;
            let height = self.rng.range(0.09, 0.23);
            self.candle(at(Vec3::new(x, 0.70, back + 0.35)), height);
        }
        self.vessel(
            CLAY,
            at(Vec3::new(altar_x + -0.70, 0.70, back + 0.17)),
            0.15,
            0.09,
        );
        self.relic(at(Vec3::new(altar_x + -0.70, 0.82, back + 0.17)), 0.23, 4);
        self.ring(
            IRON,
            at(Vec3::new(altar_x + 0.70, 0.76, back + 0.20)),
            0.11,
            q,
            0.018,
        );
        self.relic(at(Vec3::new(altar_x + 0.70, 0.80, back + 0.20)), 0.23, 5);
        for (station, motif) in [(stations[0], 16), (stations[1], 17), (stations[2], 18)] {
            let x = station.x;
            let size = self.rng.range(0.85, 1.2);
            let y = ((h.roof_height(x, -h.depth / 2.) - 0.4) * h.rise - size * 0.7).min(2.6);
            self.drawing(
                at(Vec3::new(x, y, -h.depth / 2. + 0.08)),
                q * Vec3::X,
                size,
                motif,
            );
        }
        // Entrance ward and a suspended reliquary of keys, bones and thread.
        self.drawing(
            at(Vec3::new(-1.3, 1.15, h.depth / 2. + 0.07)),
            q * -Vec3::X,
            0.75,
            21,
        );
        for i in 0..11 {
            let x = -1.4 + i as f32 * 0.28;
            let y = 2.70 - 0.3 * (1. - (x / 1.4).powi(2));
            self.beam(
                ROPE,
                at(Vec3::new(x, 3.5, 0.)),
                at(Vec3::new(x, y, 0.)),
                0.007,
            );
            self.ring(
                IRON,
                at(Vec3::new(x, y, 0.)),
                0.085,
                q * Quat::from_rotation_x(1.57),
                0.009,
            );
            self.bone(
                at(Vec3::new(x, y - 0.14, 0.)),
                q * Quat::from_rotation_x(1.57),
                0.12,
            );
        }
        self.lamp(at(Vec3::new(-1.8, 2.2, 0.6)));
        self.lamp(at(Vec3::new(1.8, 2.2, 0.6)));
        self.lamp(at(Vec3::new(0., 2.45, h.depth / 2. + 0.3)));
    }
    fn barrel(&mut self, p: Vec3, q: Quat) {
        for i in 0..18 {
            let a = i as f32 * TAU / 18.;
            self.block(
                WOOD,
                p + q * Vec3::new(a.cos() * 0.23, 0.27, a.sin() * 0.23),
                Vec3::new(0.074, 0.52, 0.04),
                q * Quat::from_rotation_y(-a + std::f32::consts::FRAC_PI_2),
                false,
            );
        }
        for y in [0.08, 0.44] {
            self.ring(IRON, p + Vec3::Y * y, 0.25, q, 0.015);
        }
    }
    pub(super) fn food_hall(&mut self, h: &House, den: &Den) {
        self.structure(h, den, h.rise);
        self.dress_house(h);
        let q = Quat::from_rotation_y(h.yaw);
        let at = |v| h.at + q * v;
        // Pack differently sized tables into the two walkable side bands.
        let desired = 2 + (self.rng.unit() * 3.).floor().min(2.) as usize;
        let mut tables: Vec<(Vec3, f32, f32)> = Vec::new();
        for index in 0..desired {
            for attempt in 0..100 {
                let side = if self.rng.unit() < 0.5 { -1. } else { 1. };
                let length = self.rng.range(1.25, 2.4);
                let width = self.rng.range(0.72, 0.90);
                let p = Vec3::new(
                    side * self.rng.range(1.50, (h.width / 2. - 1.25).max(1.51)),
                    self.rng.range(0.27, 0.36),
                    self.rng.range(
                        -h.depth / 2. + 1.75 + length / 2.,
                        h.depth / 2. - 0.6 - length / 2.,
                    ),
                );
                if tables.iter().any(|(other, _, len)| {
                    other.x.signum() == side && (other.z - p.z).abs() < (*len + length) * 0.5 + 0.32
                }) {
                    if attempt == 99 && index < 2 {
                        continue;
                    }
                    continue;
                }
                tables.push((p, width, length));
                break;
            }
        }
        // The first table on each side is always possible; fill only if sampling
        // exhausted the available room after choosing a long first table.
        if tables.len() < 2 {
            let side = -tables.first().map_or(1., |v| v.0.x.signum());
            tables.push((
                Vec3::new(
                    side * self.rng.range(1.55, 1.75),
                    self.rng.range(0.28, 0.34),
                    self.rng.range(0.10, 0.70),
                ),
                self.rng.range(0.72, 0.84),
                self.rng.range(1.25, 1.50),
            ));
        }
        for (index, (table, width, length)) in tables.into_iter().enumerate() {
            let twist = Quat::from_rotation_y(self.rng.range(-0.08, 0.08));
            let place = |v| at(table + twist * v);
            let tq = q * twist;
            self.deck(place(Vec3::ZERO), width, length, tq);
            for z in [-length * 0.38, length * 0.38] {
                for dx in [-width * 0.30, width * 0.30] {
                    self.beam(
                        WOOD,
                        place(Vec3::new(dx, -table.y, z)),
                        place(Vec3::new(dx * 0.8, -0.08, z)),
                        0.045,
                    );
                }
            }
            for dx in [-width * 0.5 - 0.23, width * 0.5 + 0.23] {
                self.block(
                    WOOD,
                    place(Vec3::new(dx, -0.16, 0.)),
                    Vec3::new(0.25, 0.07, length - 0.15),
                    tq,
                    true,
                );
                for z in [-length * 0.37, length * 0.37] {
                    self.beam(
                        WOOD,
                        place(Vec3::new(dx, -table.y, z)),
                        place(Vec3::new(dx, -0.16, z)),
                        0.04,
                    );
                }
            }
            let diners = (length / 0.62).floor().max(2.) as usize;
            for row in 0..diners {
                for sign in [-1., 1.] {
                    let z = -length / 2. + (row as f32 + 0.5) * length / diners as f32;
                    let plate = Vec3::new(sign * width * 0.28, 0.012, z);
                    self.vessel(CLAY, place(plate), 0.10, 0.03);
                    let food_size = self.rng.range(0.065, 0.11);
                    self.mushroom(place(plate + Vec3::Y * 0.035), food_size);
                    self.pot(place(plate + Vec3::new(0.07, 0., 0.14)), 0.04, 0.08);
                    self.beam(
                        IRON,
                        place(plate + Vec3::new(-0.13, 0.01, -0.05)),
                        place(plate + Vec3::new(-0.13, 0.01, 0.09)),
                        0.006,
                    );
                    if self.rng.unit() < 0.7 {
                        self.bone(place(plate + Vec3::new(0., 0.038, -0.03)), tq, 0.07);
                    }
                }
            }
            if index < 2 {
                self.relic(place(Vec3::new(0., 0.08, 0.)), 0.38, 9 + index);
            }
        }
        for side in [-1., 1.] {
            for i in 0..(2 + (self.rng.unit() * 3.).floor().min(2.) as usize) {
                let z = -h.depth / 2. + 0.55 + i as f32 * 0.61;
                let barrel = Vec3::new(side * (h.width / 2. - 0.43), 0., z);
                self.barrel(at(barrel), q);
                if side < 0. && i == 0 {
                    self.relic(at(barrel + Vec3::Y * 0.3), 0.4, 11);
                }
            }
            // Drying lines, hanging roots, mushrooms and patched provision sacks.
            let x = side * (h.width / 2. - 0.24);
            self.beam(
                ROPE,
                at(Vec3::new(x, 1.7, -1.8)),
                at(Vec3::new(x, 1.7, 1.7)),
                0.014,
            );
            for i in 0..12 {
                let z = -1.65 + i as f32 * 0.28;
                let drop = self.rng.range(0.16, 0.45);
                self.beam(
                    ROPE,
                    at(Vec3::new(x, 1.7, z)),
                    at(Vec3::new(x, 1.7 - drop, z)),
                    0.006,
                );
                for root in 0..4 {
                    let dx = (root as f32 - 1.5) * 0.018;
                    self.beam(
                        CLAY,
                        at(Vec3::new(x + dx, 1.7 - drop, z)),
                        at(Vec3::new(x + dx * 2., 1.49 - drop, z + 0.05)),
                        0.013,
                    );
                }
            }
        }
        // Masonry hearth and iron cauldron, with chains, rim handles and stirring pole.
        let hearth = Vec3::new(
            self.rng.range(-0.40, 0.40),
            0.,
            -h.depth / 2. + self.rng.range(0.75, 1.05),
        );
        for i in 0..13 {
            let a = i as f32 * TAU / 13.;
            self.block(
                STONE,
                at(hearth + Vec3::new(a.cos() * 0.56, 0.10, a.sin() * 0.43)),
                Vec3::new(0.26, 0.20, 0.23),
                q * Quat::from_rotation_y(-a),
                true,
            );
        }
        self.vessel(IRON, at(hearth + Vec3::Y * 0.22), 0.46, 0.48);
        self.shape(
            CLOTH,
            Cylinder::new(0.24, 0.012).mesh().resolution(24).build(),
            at(hearth + Vec3::Y * 0.63),
            Vec3::ONE,
            q,
            Vec3::new(0.6, 0.7, 0.4),
            false,
        );
        self.beam(
            WOOD,
            at(hearth + Vec3::new(-0.12, 0.54, 0.)),
            at(hearth + Vec3::new(0.35, 1.3, 0.1)),
            0.023,
        );
        for side in [-1., 1.] {
            self.ring(
                IRON,
                at(hearth + Vec3::new(side * 0.37, 0.58, 0.)),
                0.09,
                q * Quat::from_rotation_z(1.57),
                0.013,
            );
            self.beam(
                IRON,
                at(hearth + Vec3::new(side * 0.38, 0.6, 0.)),
                at(hearth + Vec3::new(side * 0.63, 1.7, 0.)),
                0.018,
            );
        }
        self.beam(
            WOOD,
            at(hearth + Vec3::new(-0.8, 1.7, 0.)),
            at(hearth + Vec3::new(0.8, 1.7, 0.)),
            0.065,
        );
        for side in [-1., 1.] {
            self.beam(
                WOOD,
                at(hearth + Vec3::new(side * 0.8, 0., 0.)),
                at(hearth + Vec3::new(side * 0.8, 1.7, 0.)),
                0.06,
            );
        }
        for i in 0..6 {
            let a = i as f32 * TAU / 6.;
            self.shape(
                GLOW,
                Sphere::new(1.).mesh().uv(10, 8),
                at(hearth + Vec3::new(a.cos() * 0.21, 0.16, a.sin() * 0.17)),
                Vec3::new(0.065, 0.022, 0.05),
                q,
                Vec3::ONE,
                false,
            );
        }
        self.relic(at(hearth + Vec3::Y * 0.6), 0.53, 7);
        let ledger = Vec3::new(1.10, 0.75, -h.depth / 2. + 0.19);
        self.block(WOOD, at(ledger), Vec3::new(0.6, 0.68, 0.035), q, false);
        self.drawing(at(ledger + Vec3::Z * 0.022), q * Vec3::X, 0.52, 3);
        self.relic(at(ledger), 0.40, 8);

        for motif in [33, 22, 18, 24, 26] {
            let x = self.rng.range(-h.width / 2. + 0.6, h.width / 2. - 0.6);
            let size = self.rng.range(0.50, 0.85);
            let y = ((h.roof_height(x, -h.depth / 2.) - 0.45) * h.rise - size * 0.7).min(2.5);
            self.drawing(
                at(Vec3::new(x, y, -h.depth / 2. + 0.075)),
                q * Vec3::X,
                size,
                motif,
            );
        }
        for x in [-1.6, 1.6] {
            self.lamp(at(Vec3::new(x, 2.2, 0.4)));
        }
        self.lamp(at(Vec3::new(0., 2.3, h.depth / 2. + 0.3)));
    }
}

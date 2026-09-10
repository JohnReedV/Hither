//! Excavated pilgrim/miner routes: worn stone, hewn shoring and clan history.
//! The center aisle stays clear; dressing is sampled along distance, not prefab slots.
use super::*;
impl Workshop {
    fn crag(&mut self, p: Vec3, scale: Vec3, tone: Vec3, solid: bool) {
        let mut g = primitive(Sphere::new(1.).mesh().ico(0).unwrap());
        let salt = self.rng.range(0., 1000.);
        for i in 0..g.positions.len() {
            let v = Vec3::from(g.positions[i]);
            let chip = 0.76 + geology::noise(v * 5. + Vec3::splat(salt)) * 0.38;
            let v = v * scale * chip;
            g.positions[i] = (p + v).to_array();
            g.uvs[i] = (Vec2::new(v.x + v.z * 0.3, v.y) * 1.8).to_array();
            g.colors[i] = (tone * (0.85 + chip * 0.15)).extend(1.).to_array();
        }
        let mut facets = Geometry::default();
        for face in g.indices.chunks_exact(3) {
            let k = facets.positions.len() as u32;
            for &i in face {
                let i = i as usize;
                facets.vertex(
                    Vec3::from(g.positions[i]),
                    Vec2::from(g.uvs[i]),
                    Vec4::from(g.colors[i]).truncate(),
                );
            }
            facets.triangle(k, k + 1, k + 2);
        }
        g = facets;
        g.finish_normals();
        append(&mut self.g[STONE], &g, Transform::default());
        if solid {
            append(&mut self.solid, &g, Transform::default());
        }
    }
    fn hewn(&mut self, a: Vec3, b: Vec3, radius: f32) {
        let length = a.distance(b);
        let q = Quat::from_rotation_arc(Vec3::Y, (b - a).normalize());
        let salt = self.rng.range(0., 1000.);
        let mut g = Geometry::default();
        // Uneven adze-cut facets, taper, bowed axis and dark longitudinal splits.
        for row in 0..=8 {
            let t = row as f32 / 8.;
            for i in 0..=12 {
                let angle = (i % 12) as f32 * TAU / 12.;
                let grain = geology::noise(Vec3::new(angle.cos() * 3., t * 2., salt));
                let r = radius * (0.76 + grain * 0.30) * (1. - t * 0.16);
                let bow = (t * std::f32::consts::PI).sin() * radius * 0.30;
                let pos = a + q * Vec3::new(angle.cos() * r + bow, t * length, angle.sin() * r);
                let color = Vec3::new(0.86, 0.90, 0.86) * (0.62 + grain * 0.46);
                g.vertex(pos, Vec2::new(i as f32 * 0.11, t * length * 0.55), color);
                if row > 0 && i > 0 {
                    let k = ((row - 1) * 13 + i - 1) as u32;
                    g.triangle(k, k + 13, k + 1);
                    g.triangle(k + 1, k + 13, k + 14);
                }
            }
        }
        for (row, center, reverse) in [(0, a, true), (8, b, false)] {
            let c = g.vertex(center, Vec2::splat(0.5), Vec3::splat(0.62));
            for i in 0..12 {
                let k = (row * 13 + i) as u32;
                if reverse {
                    g.triangle(c, k, k + 1);
                } else {
                    g.triangle(c, k + 1, k);
                }
            }
        }
        g.finish_normals();
        append(&mut self.g[WOOD], &g, Transform::default());
        append(&mut self.solid, &g, Transform::default());
        for _ in 0..4 {
            let angle = self.rng.range(0., TAU);
            let start = self.rng.range(0.05, 0.55);
            let end = (start + self.rng.range(0.2, 0.40)).min(0.96);
            let mut points = Vec::new();
            for i in 0..5 {
                let t = start + (end - start) * i as f32 / 4.;
                let grain = geology::noise(Vec3::new(angle.cos() * 3., t * 2., salt));
                let r = radius * (0.76 + grain * 0.30) * (1. - t * 0.16) + 0.002;
                points.push(
                    a + q * Vec3::new(
                        angle.cos() * r + (t * std::f32::consts::PI).sin() * radius * 0.30,
                        t * length,
                        angle.sin() * r,
                    ),
                );
            }
            for p in points.windows(2) {
                self.beam(IRON, p[0], p[1], 0.0035);
            }
        }
    }
    fn tunnel_landing(&mut self, path: &[Vec3]) {
        let center = path[1] - Vec3::Y * 1.45;
        let outgoing = (path[2] - path[1]).with_y(0.).normalize();
        // The bridge and burrow meet at different angles. Fill their entire
        // shared corner, rather than merely joining their centerlines.
        let height = |p: Vec3| {
            let sample = |a: Vec3, b: Vec3| {
                let delta = b.xz() - a.xz();
                let t = ((p.xz() - a.xz()).dot(delta) / delta.length_squared()).clamp(0., 1.);
                let q = a.lerp(b, t);
                (q.y - 1.45, q.xz().distance_squared(p.xz()))
            };
            let (a, da) = sample(path[0], path[1]);
            let (b, db) = sample(path[1], path[2]);
            if da + db < 0.00001 {
                center.y + 0.015
            } else {
                (a * db + b * da) / (da + db) + 0.015
            }
        };
        let mut top = Geometry::default();
        let segments = 32_u32;
        let rings = 8_u32;
        top.vertex(
            center + Vec3::Y * 0.015,
            center.xz() * 0.65,
            Vec3::new(0.65, 0.59, 0.48),
        );
        for ring in 1..=rings {
            for i in 0..segments {
                let angle = i as f32 * TAU / segments as f32;
                let ray = Vec3::new(angle.cos(), 0., angle.sin());
                let radius = 2.05 + geology::noise(center * 0.3 + ray * 2.1) * 0.35;
                let mut p = center + ray * radius * ring as f32 / rings as f32;
                p.y = height(p);
                top.vertex(p, p.xz() * 0.65, Vec3::new(0.65, 0.59, 0.48));
            }
        }
        for i in 0..segments {
            let next = (i + 1) % segments;
            top.triangle(0, 1 + next, 1 + i);
            for ring in 1..rings {
                let a = 1 + (ring - 1) * segments + i;
                let b = 1 + (ring - 1) * segments + next;
                let c = a + segments;
                let d = b + segments;
                top.triangle(a, b, d);
                top.triangle(a, d, c);
            }
        }
        top.finish_normals();
        append(&mut self.g[EARTH], &top, Transform::default());
        append(&mut self.solid, &top, Transform::default());
        // A closed rock buttress supports the turn and buries the dirt skirts.
        // Its lower ring retreats into the mountain instead of hanging as a slab.
        let mut bank = Geometry::default();
        for i in 0..segments {
            let p = Vec3::from(top.positions[(1 + (rings - 1) * segments + i) as usize]);
            let lower = center + outgoing * 1.2 + (p - center) * 0.48 - Vec3::Y * 2.0;
            for v in [p, lower] {
                bank.vertex(v, Vec2::new(v.x + v.z * 0.3, v.y) * 0.65, geology::tint(v));
            }
        }
        let bottom = bank.vertex(
            center + outgoing * 1.2 - Vec3::Y * 2.0,
            Vec2::ZERO,
            Vec3::splat(0.65),
        );
        for i in 0..segments {
            let a = i * 2;
            let b = ((i + 1) % segments) * 2;
            bank.triangle(a, b, a + 1);
            bank.triangle(b, b + 1, a + 1);
            bank.triangle(bottom, a + 1, b + 1);
        }
        bank.finish_normals();
        append(&mut self.g[STONE], &bank, Transform::default());
        append(&mut self.solid, &bank, Transform::default());
    }

    fn earth_run(&mut self, a: Vec3, b: Vec3, tangent_a: Vec3, tangent_b: Vec3) {
        let dir = (b - a).with_y(0.).normalize();
        let side = Vec3::Y.cross(dir);
        // Shared cross-sections make one continuous earth surface. A narrow
        // center band keeps tight turns walkable while the banks twist outward.
        let mut g = Geometry::default();
        let width =
            |p: Vec3, sign: f32| 1.02 + geology::noise(p * 0.21 + Vec3::splat(sign * 17.)) * 0.43;
        let mut top_normals = Vec::new();
        for (p, tangent) in [(a, tangent_a), (b, tangent_b)] {
            let across = Vec3::Y.cross(tangent.with_y(0.).normalize());
            let normal = tangent.cross(across).normalize();
            for x in [-width(p, -1.), -0.4, 0.4, width(p, 1.)] {
                let v = p + across * x;
                g.vertex(v, v.xz() * 0.65, Vec3::new(0.58, 0.53, 0.45));
                top_normals.push(normal.to_array());
            }
        }
        for i in 0..3 {
            g.triangle(i, i + 5, i + 1);
            g.triangle(i, i + 4, i + 5);
        }
        for (a_index, b_index, sign) in [(4, 0, -1.), (3, 7, 1.)] {
            let k = g.positions.len() as u32;
            for index in [a_index, b_index] {
                let p = Vec3::from(g.positions[index as usize]);
                let v = p - Vec3::Y * 1.15 + side * sign * 0.7;
                g.vertex(
                    v,
                    Vec2::new(v.x, v.z + v.y) * 0.65,
                    Vec3::new(0.51, 0.47, 0.40),
                );
            }
            g.triangle(a_index, b_index, k);
            g.triangle(b_index, k + 1, k);
        }
        g.finish_normals();
        g.normals[..8].copy_from_slice(&top_normals);

        append(&mut self.g[EARTH], &g, Transform::default());
        append(&mut self.solid, &g, Transform::default());
        let count = (a.distance(b) / 0.62).ceil() as usize;
        for i in 0..count {
            let t = (i as f32 + 0.5) / count as f32;
            let p = a.lerp(b, t);
            for sign in [-1., 1.] {
                if self.rng.unit() < 0.65 {
                    continue;
                }
                let x = sign * self.rng.range(1.08, 1.44);
                let width = self.rng.range(0.08, 0.25);
                self.crag(
                    p + side * x - Vec3::Y * 0.025,
                    Vec3::new(width, width * 0.55, width * 1.4),
                    Vec3::new(0.62, 0.58, 0.48),
                    false,
                );
            }
            // Scattered spoil, dirt and wet smears; no paving or fitted edging.
            for _ in 0..(1 + (self.rng.unit() * 4.) as usize) {
                let x = self.rng.range(-1.0, 1.0);
                let z = self.rng.range(-0.25, 0.25);
                let width = self.rng.range(0.015, 0.07);
                self.crag(
                    p + side * x + dir * z,
                    Vec3::new(width, width * 0.45, width * 0.8),
                    Vec3::new(0.68, 0.59, 0.44),
                    false,
                );
            }

            if self.rng.unit() < 0.18 {
                let x = self.rng.range(-1.1, 1.1);
                self.crag(
                    p + side * x,
                    Vec3::new(0.22, 0.008, 0.31),
                    Vec3::new(0.23, 0.18, 0.11),
                    false,
                );
            }
            if self.rng.unit() < 0.42 {
                let p = p + side * self.rng.range(-0.8, 0.8) + Vec3::Y * 0.014;
                let length = self.rng.range(0.18, 0.55);
                let drift = self.rng.range(-0.12, 0.12);
                let grade = (b.y - a.y) / a.xz().distance(b.xz());
                let end = p + dir * length + side * drift + Vec3::Y * (grade * length);
                let g = &mut self.g[EARTH];
                let k = g.vertex(p - side * 0.017, p.xz() * 0.65, Vec3::new(0.19, 0.13, 0.07));
                g.vertex(end, end.xz() * 0.65, Vec3::new(0.32, 0.24, 0.14));
                g.vertex(p + side * 0.018, p.xz() * 0.65, Vec3::new(0.23, 0.16, 0.09));
                g.triangle(k, k + 1, k + 2);
                let normal = Vec3::new(-dir.x * grade, 1., -dir.z * grade).normalize();
                for n in &mut g.normals[k as usize..] {
                    *n = normal.to_array();
                }
            }
        }
    }
    fn shore(&mut self, floor: Vec3, dir: Vec3) {
        // A single crooked prop under suspect rock, not an engineered frame.
        let side = Vec3::Y.cross(dir) * if self.rng.unit() < 0.5 { -1. } else { 1. };
        let foot = floor + side * self.rng.range(1.05, 1.30);
        let under = floor
            + side * self.rng.range(1.30, 1.55)
            + dir * self.rng.range(-0.5, 0.5)
            + Vec3::Y * 0.1;
        let Some(height) = self.wall.as_ref().and_then(|w| w.hit(under, Vec3::Y, 4.)) else {
            return;
        };
        let top = under + Vec3::Y * height;
        let radius = self.rng.range(0.065, 0.105);
        self.hewn(foot, top, radius);
        self.crag(
            top - Vec3::Y * 0.06,
            Vec3::new(0.20, 0.07, 0.16),
            Vec3::splat(0.64),
            false,
        );
        for k in 0..3 {
            self.ring(
                ROPE,
                foot.lerp(top, 0.72) + Vec3::Y * k as f32 * 0.025,
                radius * 0.98,
                Quat::IDENTITY,
                0.008,
            );
        }
    }
    fn grease_lamp(&mut self, p: Vec3) {
        for _ in 0..5 {
            let offset = Vec3::new(
                self.rng.range(-0.11, 0.11),
                0.003,
                self.rng.range(-0.11, 0.11),
            );
            self.crag(
                p + offset,
                Vec3::new(0.07, 0.008, 0.09),
                Vec3::new(0.14, 0.10, 0.08),
                false,
            );
        }
        self.vessel(CLAY, p, 0.10, 0.065);
        self.beam(IRON, p + Vec3::Y * 0.04, p + Vec3::Y * 0.12, 0.006);
        self.shape(
            GLOW,
            Sphere::new(1.).mesh().uv(8, 6),
            p + Vec3::Y * 0.15,
            Vec3::new(0.015, 0.045, 0.012),
            Quat::IDENTITY,
            Vec3::ONE,
            false,
        );
        self.lamps.push(p + Vec3::Y * 0.16);
    }
    fn wall_story(
        &mut self,
        _den: &Den,
        center: Vec3,
        dir: Vec3,
        sign: f32,
        motif: usize,
        size: f32,
    ) {
        let side = Vec3::Y.cross(dir) * sign;
        let first = self.g[PAINT].positions.len();
        let first_index = self.g[PAINT].indices.len();
        self.drawing(center + side * 1.8, dir, size, motif);
        // Project directly onto rendered triangles, also avoiding expensive
        // repeated excavation queries while a den is built in the background.
        let mut valid = Vec::new();
        for index in first..self.g[PAINT].positions.len() {
            let p = Vec3::from(self.g[PAINT].positions[index]) - side * 1.8;
            let hit = self.wall.as_ref().and_then(|wall| wall.hit(p, side, 3.0));
            valid.push(hit.is_some());
            if let Some(distance) = hit {
                self.g[PAINT].positions[index] = (p + side * (distance - 0.012)).to_array();
            }
        }
        let indices: Vec<_> = self.g[PAINT].indices.drain(first_index..).collect();
        for face in indices.chunks_exact(3) {
            if face.iter().all(|&i| valid[i as usize - first]) {
                self.g[PAINT].indices.extend_from_slice(face);
            }
        }
    }

    fn pick_scars(&mut self, center: Vec3, dir: Vec3) {
        let side = Vec3::Y.cross(dir) * if self.rng.unit() < 0.5 { -1. } else { 1. };
        for _ in 0..(5 + (self.rng.unit() * 11.) as usize) {
            let p = center + dir * self.rng.range(-0.7, 0.7) + Vec3::Y * self.rng.range(-0.9, 0.55);
            let length = self.rng.range(0.12, 0.38);
            let drift = self.rng.range(-0.12, 0.12);
            let points = [
                p - dir * 0.014,
                p + dir * drift + Vec3::Y * length,
                p + dir * 0.026,
            ];
            let mut hit_points = Vec::new();
            for v in points {
                if let Some(t) = self.wall.as_ref().and_then(|w| w.hit(v, side, 3.5)) {
                    hit_points.push(v + side * (t - 0.009));
                }
            }
            if hit_points.len() == 3 {
                let g = &mut self.g[PAINT];
                let k = g.positions.len() as u32;
                for p in hit_points {
                    g.vertex(p, p.xz(), Vec3::new(0.23, 0.20, 0.15));
                }
                g.triangle(k, k + 1, k + 2);
            }
        }
    }
    fn waystation(&mut self, den: &Den, center: Vec3, dir: Vec3, role: usize, record: bool) {
        let side = Vec3::Y.cross(dir);
        let sign = if self.rng.unit() < 0.5 { -1. } else { 1. };
        let floor = center - Vec3::Y * 1.45;
        let at = |x: f32, y: f32, z: f32| floor + side * x * sign + Vec3::Y * y + dir * z;
        let q = Quat::from_rotation_y(dir.x.atan2(dir.z));
        let object = at(1.47, 0.30, 0.);
        match role {
            0 => {
                // Stolen scraps snagged on a spike: one tangled alarm bundle.
                self.hewn(at(1.5, 0., 0.), at(1.7, 1.25, -0.12), 0.055);
                for _ in 0..(2 + (self.rng.unit() * 3.) as usize) {
                    let z = self.rng.range(-0.2, 0.2);
                    let y = self.rng.range(0.4, 0.9);
                    self.beam(ROPE, at(1.7, 1.2, -0.12), at(1.5, y, z), 0.007);
                    self.vessel(IRON, at(1.5, y - 0.1, z), 0.05, 0.1);
                    self.bone(at(1.5, y - 0.15, z), q, 0.14);
                }
                self.pot(object, 0.18, 0.20);
            }
            1 => {
                // Abandoned shift: ore basket, pick, wedge bag, lamp and tally.
                for i in 0..8 {
                    self.ring(
                        ROPE,
                        at(1.45, 0.06 + i as f32 * 0.035, 0.),
                        0.21,
                        Quat::IDENTITY,
                        0.015,
                    );
                }
                for i in 0..9 {
                    let a = i as f32 * TAU / 9.;
                    self.beam(
                        WOOD,
                        object + Vec3::new(a.cos() * 0.20, -0.26, a.sin() * 0.20),
                        object + Vec3::new(a.cos() * 0.20, 0.01, a.sin() * 0.20),
                        0.012,
                    );
                    let x = self.rng.range(1.30, 1.58);
                    let z = self.rng.range(-0.12, 0.12);
                    self.crag(
                        at(x, 0.26, z),
                        Vec3::splat(0.09),
                        Vec3::new(1.0, 0.58, 0.27),
                        false,
                    );
                }
                self.hewn(at(1.30, 0.04, 0.38), at(1.66, 0.93, 0.42), 0.025);
                self.beam(IRON, at(1.48, 0.83, 0.42), at(1.83, 1.02, 0.42), 0.03);
                for i in 0..4 {
                    self.trinket(at(1.40, 0.03, -0.45 - i as f32 * 0.11), q);
                }
            }
            2 => {
                // Memorial cairn, offerings and tiny masks of lost miners.
                for i in 0..7 {
                    let z = self.rng.range(-0.13, 0.13);
                    let x = self.rng.range(1.36, 1.54);
                    self.crag(
                        at(x, 0.07 + i as f32 * 0.085, z),
                        Vec3::new(0.28 - i as f32 * 0.019, 0.09, 0.22),
                        Vec3::splat(0.84),
                        false,
                    );
                }
                self.beam(ROPE, at(1.55, 0.76, -0.3), at(1.55, 0.90, 0.3), 0.009);
                for i in 0..4 {
                    self.bone(at(1.55, 0.75, -0.24 + i as f32 * 0.15), q, 0.10);
                }
                self.pot(at(1.26, 0., 0.38), 0.10, 0.12);
                self.trinket(at(1.28, 0., -0.35), q);
            }
            _ => {
                // Homecoming ledger, hoisted bundles and shared first meal.
                self.hide(at(1.45, 0.04, 0.), Vec2::new(0.25, 0.34), q);
                self.pot(at(1.40, 0.07, 0.), 0.20, 0.25);
                for i in 0..5 {
                    self.trinket(at(1.35, 0.03, -0.50 + i as f32 * 0.21), q);
                }
                self.hewn(at(1.7, 0., -0.3), at(1.7, 1.8, -0.3), 0.07);
                self.beam(ROPE, at(1.7, 1.7, -0.3), at(1.40, 0.36, 0.), 0.009);
            }
        }
        let motif = [31, 27, 32, 9][role];
        self.wall_story(den, center - Vec3::Y * 0.15, dir, sign, motif, 0.95);
        self.wall_story(
            den,
            center - dir * 0.95 - Vec3::Y * 0.45,
            dir,
            -sign,
            3,
            0.50,
        );
        if record {
            self.relic(object + Vec3::Y * 0.22, 0.40, 12 + role);
        }
        if role == 1 || self.rng.unit() < 0.35 {
            let z = self.rng.range(-0.5, 0.5);
            self.grease_lamp(at(1.20, 0.08, z));
        }
    }
    fn entrance(&mut self, den: &Den, path: &[Vec3]) {
        let n = path.len();
        let center = path[n - 3];
        let dir = (path[n - 1] - path[n - 4]).with_y(0.).normalize();
        let side = Vec3::Y.cross(dir);
        let floor = center - Vec3::Y * 1.45;
        let inner = path[n - 1] - Vec3::Y * 1.45;
        let outer = inner + dir * 0.65 - Vec3::Y * 0.55;
        self.earth_run(inner, outer, path[n - 1] - path[n - 2], outer - inner);
        // Spoil thrown out of the hole, not fitted stones surrounding a doorway.
        let heavy_side = if self.rng.unit() < 0.5 { -1. } else { 1. };
        for sign in [-1., 1.] {
            let count = if sign == heavy_side { 37 } else { 16 };
            for _ in 0..count {
                let x = self.rng.range(1.55, 3.3) * sign;
                let z = self.rng.range(-0.3, 5.0);
                let mut p = floor + side * x + dir * z;
                p.y = terrain::height(den.center.xz() + p.xz()) - den.center.y;
                if p.y > floor.y + 0.40 {
                    continue;
                }
                let e = 0.25;
                let world = den.center.xz() + p.xz();
                let normal = Vec3::new(
                    terrain::height(world - Vec2::X * e) - terrain::height(world + Vec2::X * e),
                    2. * e,
                    terrain::height(world - Vec2::Y * e) - terrain::height(world + Vec2::Y * e),
                )
                .normalize();
                let rotation = Quat::from_rotation_arc(Vec3::Y, normal);
                let first = self.g[STONE].positions.len();
                let size = self.rng.range(0.06, 0.32);
                self.crag(
                    p + Vec3::Y * size * 0.3,
                    Vec3::new(size, size * 0.55, size * 1.4),
                    Vec3::new(0.79, 0.63, 0.43),
                    false,
                );
                for i in first..self.g[STONE].positions.len() {
                    let v = Vec3::from(self.g[STONE].positions[i]);
                    self.g[STONE].positions[i] =
                        (p + rotation * (v - p) - normal * size * 0.25).to_array();
                    self.g[STONE].normals[i] =
                        (rotation * Vec3::from(self.g[STONE].normals[i])).to_array();
                }
            }
        }
        // Raw roots and split rock hang from the lip. Their attachment points
        // come from the excavated surface, not a manufactured arch silhouette.
        for _ in 0..13 {
            let sign = if self.rng.unit() < 0.5 { -1. } else { 1. };
            let ray = (Vec3::Y + side * sign * self.rng.range(0.45, 1.4)).normalize();
            let start = center - dir * self.rng.range(0., 2.0);
            if let Some(t) = self.wall.as_ref().and_then(|w| w.hit(start, ray, 4.)) {
                let root = start + ray * t;
                let end =
                    root - Vec3::Y * self.rng.range(0.25, 0.85) + dir * self.rng.range(-0.15, 0.15);
                self.beam(WOOD, root, end, 0.014);
                self.beam(WOOD, end.lerp(root, 0.3), end + side * sign * 0.15, 0.008);
            }
        }
        let sign = heavy_side;
        let stake = floor + side * sign * 1.45 + dir * 0.25;
        self.hewn(stake, stake + Vec3::Y * 1.40 + side * sign * 0.15, 0.065);
        let q = Quat::from_rotation_arc(Vec3::Y, dir);
        self.hide(stake + Vec3::Y * 1.06, Vec2::new(0.26, 0.37), q);
        self.bone(stake + Vec3::Y * 1.47, q, 0.28);
        self.ring(IRON, stake + Vec3::Y * 0.70 + dir * 0.06, 0.10, q, 0.012);
        self.grease_lamp(floor - side * sign * 1.18 - dir * 0.6 + Vec3::Y * 0.05);
        self.wall_story(den, center - dir * 1.5, dir, sign, 0, 1.4);
        self.wall_story(
            den,
            center - dir * 2.5 - Vec3::Y * 0.35,
            dir,
            -sign,
            34,
            0.7,
        );
    }
    pub(super) fn passages(&mut self, den: &Den) {
        for path in &den.tunnels {
            self.tunnel_landing(path);
            let mut roles = [0, 1, 2, 3];
            for i in (1..4).rev() {
                let j = (self.rng.unit() * (i + 1) as f32).floor().min(i as f32) as usize;
                roles.swap(i, j);
            }
            let mut traveled = 0.;
            let mut next_shore = self.rng.range(16., 35.);
            let mut next_story = self.rng.range(5., 14.);
            let mut stories = 0;
            for (i, seg) in path.windows(2).enumerate() {
                let a = seg[0] - Vec3::Y * 1.45;
                let b = seg[1] - Vec3::Y * 1.45;
                if i == 0 {
                    self.bridge(
                        Span {
                            a,
                            b,
                            width: 2.4,
                            sag: 0.,
                        },
                        false,
                    );
                    continue;
                }
                let tangent_at =
                    |j: usize| path[(j + 1).min(path.len() - 1)] - path[j.saturating_sub(1).max(1)];
                self.earth_run(a, b, tangent_at(i), tangent_at(i + 1));
                let dir = (b - a).with_y(0.).normalize();
                traveled += a.distance(b);
                if self.rng.unit() < 0.11 {
                    self.pick_scars(seg[0], dir);
                }
                if traveled >= next_shore && i < path.len() - 5 {
                    self.shore(a, dir);
                    next_shore = traveled + self.rng.range(25., 48.);
                }
                if traveled >= next_story && i < path.len() - 6 {
                    let role = if stories < 4 {
                        roles[stories]
                    } else {
                        (self.rng.unit() * 4.).floor().min(3.) as usize
                    };
                    self.waystation(den, seg[0], dir, role, stories < 4);
                    stories += 1;
                    next_story = traveled + self.rng.range(12., 26.);
                }
            }
            self.entrance(den, path);
        }
    }
}

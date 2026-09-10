//! Scavenged joinery, hide-and-straw nests, thrown pottery and hammered
//! metal. World-scale grain/pores and high roughness avoid glossy toy surfaces.
use super::*;
use crate::rendering::geometry::{append, primitive};
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
#[path = "civic.rs"]
mod civic;
#[path = "clutter.rs"]
mod clutter;
#[path = "passages.rs"]
mod passages;
#[path = "pictographs.rs"]
mod pictographs;
const STONE: usize = 0;
const WOOD: usize = 1;
const ROPE: usize = 2;
const IRON: usize = 3;
const CLAY: usize = 4;
const CLOTH: usize = 5;
pub(super) const GLOW: usize = 6;
const PAINT: usize = 7;
const BONE: usize = 8;
const EARTH: usize = 9;
pub(super) struct Workshop {
    g: Vec<Geometry>,
    solid: Geometry,
    lamps: Vec<Vec3>,
    relics: Vec<lore::Relic>,
    rng: Rng,
    junctions: Vec<Span>,
    wall: Option<RayMesh>,
}
impl Workshop {
    #[expect(
        clippy::too_many_arguments,
        reason = "Geometry authoring specifies material, pose, tint, and collision independently."
    )]
    fn shape(
        &mut self,
        mat: usize,
        mesh: Mesh,
        at: Vec3,
        scale: Vec3,
        rot: Quat,
        color: Vec3,
        solid: bool,
    ) {
        let mut g = primitive(mesh);
        g.colors.fill(color.extend(1.).to_array());
        let t = Transform::from_translation(at)
            .with_scale(scale)
            .with_rotation(rot);
        append(&mut self.g[mat], &g, t);
        if solid {
            append(&mut self.solid, &g, t);
        }
    }
    fn block(&mut self, mat: usize, at: Vec3, size: Vec3, rot: Quat, solid: bool) {
        let tone = self.rng.range(0.74, 1.15);
        let mut g = primitive(Cuboid::new(size.x, size.y, size.z).into());
        if mat == WOOD {
            // Physical grain scale follows the long axis of each face. A long
            // board no longer stretches one shiny texture over its whole length.
            let offset = Vec2::new(self.rng.range(0., 8.), self.rng.range(0., 8.));
            for i in 0..g.positions.len() {
                let normal = Vec3::from(g.normals[i]).abs();
                let mut axes = [0, 1, 2];
                axes.sort_by(|&a, &b| normal[a].total_cmp(&normal[b]));
                if size[axes[0]] > size[axes[1]] {
                    axes.swap(0, 1);
                }
                let p = Vec3::from(g.positions[i]);
                g.uvs[i] = (Vec2::new(p[axes[0]] * 2.5, p[axes[1]] * 0.55) + offset).to_array();
            }
        }
        g.colors.fill(Vec3::splat(tone).extend(1.).to_array());
        let transform = Transform::from_translation(at).with_rotation(rot);
        append(&mut self.g[mat], &g, transform);
        if solid {
            append(&mut self.solid, &g, transform);
        }
    }

    fn beam(&mut self, mat: usize, a: Vec3, b: Vec3, r: f32) {
        self.shape(
            mat,
            Cylinder::new(r, a.distance(b))
                .mesh()
                .resolution(10)
                .build(),
            (a + b) * 0.5,
            Vec3::ONE,
            Quat::from_rotation_arc(Vec3::Y, (b - a).normalize()),
            Vec3::ONE,
            r >= 0.025,
        );
    }
    fn pot(&mut self, p: Vec3, r: f32, h: f32) {
        self.vessel(CLAY, p, r, h);
    }
    fn vessel(&mut self, mat: usize, p: Vec3, r: f32, h: f32) {
        // Closed foot, thick rolled lip and visible hollow interior; no solid
        // sphere pretending to be a vessel. Rings also carry throwing ridges.
        let profile = [
            (0., 0.06),
            (0.04, 0.60),
            (0.20, 0.96),
            (0.55, 1.),
            (0.80, 0.73),
            (0.95, 0.56),
            (1., 0.62),
            (1.025, 0.59),
            (1., 0.49),
            (0.92, 0.47),
            (0.75, 0.62),
            (0.50, 0.88),
            (0.22, 0.84),
            (0.12, 0.5),
            (0.12, 0.),
        ];
        let g = &mut self.g[mat];
        let first = g.positions.len() as u32;
        for (row, (y, radius)) in profile.iter().enumerate() {
            for i in 0..=32 {
                let a = i as f32 / 32. * TAU;
                let rr = r * radius * (1. + 0.017 * (a * 5. + row as f32).sin());
                g.vertex(
                    p + Vec3::new(a.cos() * rr, y * h, a.sin() * rr),
                    Vec2::new(i as f32 / 32., *y),
                    Vec3::new(0.9, 0.85, 0.78),
                );
                if row > 0 && i > 0 {
                    let b = first + ((row - 1) * 33 + i - 1) as u32;
                    g.triangle(b, b + 33, b + 1);
                    g.triangle(b + 1, b + 33, b + 34);
                }
            }
        }
    }
    fn lamp(&mut self, p: Vec3) {
        self.lamps.push(p);
        self.block(
            IRON,
            p - Vec3::Y * 0.20,
            Vec3::new(0.27, 0.06, 0.27),
            Quat::IDENTITY,
            false,
        );
        self.block(
            IRON,
            p + Vec3::Y * 0.22,
            Vec3::new(0.30, 0.055, 0.30),
            Quat::IDENTITY,
            false,
        );
        for x in [-0.105, 0.105] {
            for z in [-0.105, 0.105] {
                self.beam(
                    IRON,
                    p + Vec3::new(x, -0.2, z),
                    p + Vec3::new(x, 0.22, z),
                    0.012,
                );
            }
        }
        self.shape(
            GLOW,
            Sphere::new(0.08).mesh().uv(16, 12),
            p,
            Vec3::new(0.7, 1.9, 0.7),
            Quat::IDENTITY,
            Vec3::ONE,
            false,
        );
        self.beam(IRON, p + Vec3::Y * 0.23, p + Vec3::Y * 0.6, 0.012);
    }
    fn deck(&mut self, p: Vec3, w: f32, d: f32, q: Quat) {
        let n = (w / 0.18).ceil() as usize;
        for i in 0..n {
            self.block(
                WOOD,
                p + q * Vec3::new(-w / 2. + (i as f32 + 0.5) * w / n as f32, -0.075, 0.),
                Vec3::new(w / n as f32 - 0.006, 0.15, d),
                q,
                false,
            );
        }
        // Continuous collision matches the 6mm joints; a foot cannot enter one.
        let g = primitive(Cuboid::new(w, 0.15, d).into());
        append(
            &mut self.solid,
            &g,
            Transform::from_translation(p - Vec3::Y * 0.075).with_rotation(q),
        );
        for x in [-w * 0.4, w * 0.4] {
            self.beam(
                WOOD,
                p + q * Vec3::new(x, -0.22, -d * 0.5),
                p + q * Vec3::new(x, -0.22, d * 0.5),
                0.10,
            );
        }
    }
    fn bridge(&mut self, span: Span, rails: bool) {
        let inset = |at: Vec3, to: Vec3| {
            let direction = (to - at).with_y(0.).normalize();
            self.junctions
                .iter()
                .filter_map(|other| {
                    let end = if other.a.distance(at) < 0.03 {
                        other.b
                    } else if other.b.distance(at) < 0.03 {
                        other.a
                    } else {
                        return None;
                    };
                    if end.distance(to) < 0.03 {
                        return None;
                    }
                    let cosine = direction.dot((end - at).with_y(0.).normalize());
                    if cosine <= 0. {
                        return None;
                    }
                    let sine = (1. - cosine * cosine).sqrt().max(0.12);
                    Some((span.width.max(other.width) * 0.5 + 0.36) / sine + 0.30)
                })
                .fold(1.30_f32, f32::max)
        };
        let start_inset = inset(span.a, span.b);
        let end_inset = inset(span.b, span.a);
        let length = span.a.distance(span.b);
        let n = (span.a.distance(span.b) / 0.20).ceil().max(1.) as usize;
        let direction = (span.b - span.a).with_y(0.).normalize();
        let side = Vec3::Y.cross(direction);
        let yaw = Quat::from_rotation_y(direction.x.atan2(direction.z));
        for i in 0..n {
            let a = span.point(i as f32 / n as f32);
            let b = span.point((i + 1) as f32 / n as f32);
            let mid = (a + b) * 0.5;
            let tilt = Quat::from_rotation_x(-(b.y - a.y).atan2(b.xz().distance(a.xz())));
            self.block(
                WOOD,
                mid - Vec3::Y * 0.055,
                Vec3::new(span.width, 0.11, a.distance(b) + 0.002),
                yaw * tilt,
                true,
            );
            if rails
                && i % 5 == 0
                && (i as f32 + 0.5) / n as f32 * length > start_inset
                && (1. - (i as f32 + 0.5) / n as f32) * length > end_inset
            {
                for sign in [-1., 1.] {
                    let p = mid + side * span.width * 0.49 * sign;
                    self.beam(ROPE, p, p + Vec3::Y * 0.95, 0.016);
                    // Tightly wrapped lashings at each upright/deck connection.
                    for j in 0..3 {
                        self.beam(
                            ROPE,
                            p + direction * 0.05 + Vec3::Y * (j as f32 * 0.018),
                            p - direction * 0.05 + Vec3::Y * (j as f32 * 0.018),
                            0.009,
                        );
                    }
                }
            }
        }
        if rails {
            for sign in [-1., 1.] {
                for height in [0.12, 0.95] {
                    for i in 0..n {
                        let distance = span.a.distance(span.b);
                        let along = (i as f32 + 0.5) / n as f32 * distance;
                        if along < start_inset || distance - along < end_inset {
                            continue;
                        }
                        let offset = side * (span.width * 0.49 * sign) + Vec3::Y * height;
                        self.beam(
                            ROPE,
                            span.point(i as f32 / n as f32) + offset,
                            span.point((i + 1) as f32 / n as f32) + offset,
                            0.028,
                        );
                    }
                }
            }
        }
    }
    // An irregular, creased animal hide: radial rings preserve the ragged edge
    // and actual folds instead of representing bedding with a smooth ellipsoid.
    fn hide(&mut self, p: Vec3, size: Vec2, q: Quat) {
        let phase = self.rng.range(0., TAU);
        let g = &mut self.g[CLOTH];
        let first = g.positions.len() as u32;
        for ring in 0..=8 {
            let r = ring as f32 / 8.;
            for i in 0..=48 {
                let a = i as f32 / 48. * TAU;
                let edge = 1. + 0.13 * (a * 7. + phase).sin() + 0.08 * (a * 11.).cos();
                let x = a.cos() * r * size.x * edge;
                let z = a.sin() * r * size.y * edge;
                let y = 0.014 + 0.018 * (a * 6. + r * 12. + phase).sin() * r + 0.024 * r.powi(3);
                g.vertex(
                    p + q * Vec3::new(x, y, z),
                    Vec2::new(x * 5., z * 5.),
                    Vec3::new(0.67, 0.49, 0.31) * (0.9 + 0.1 * (a * 9.).sin()),
                );
                if ring > 0 && i > 0 {
                    let b = first + ((ring - 1) * 49 + i - 1) as u32;
                    g.triangle(b, b + 1, b + 49);
                    g.triangle(b + 1, b + 50, b + 49);
                }
            }
        }
    }
    fn structure(&mut self, h: &House, den: &Den, rise: f32) {
        let q = Quat::from_rotation_y(h.yaw);
        let at = |v: Vec3| h.at + q * (v * Vec3::new(1., rise, 1.));
        // Unequal overhangs make a fresh platform outline, with solid support
        // under the occupied room and individually cut outer planks.
        let width = h.width + 1.2;
        let n = (width / 0.18).ceil() as usize;
        for i in 0..n {
            let t = (i as f32 + 0.5) / n as f32;
            let front = h.depth * 0.5 + h.edges.x.lerp(h.edges.y, t);
            let back = -h.depth * 0.5 - h.edges.z.lerp(h.edges.w, t);
            self.block(
                WOOD,
                h.at + q * Vec3::new((t - 0.5) * width, -0.075, (front + back) * 0.5),
                Vec3::new(width / n as f32 - 0.005, 0.15, front - back),
                q,
                true,
            );
        }
        let slab = primitive(Cuboid::new(h.width + 0.9, 0.15, h.depth + 0.9).into());
        append(
            &mut self.solid,
            &slab,
            Transform::from_translation(h.at - Vec3::Y * 0.075).with_rotation(q),
        );
        for x in [-h.width / 2., h.width / 2.] {
            for z in [-h.depth / 2., h.depth / 2.] {
                let top = h.roof_height(x, z);
                self.beam(
                    WOOD,
                    at(Vec3::new(x, 0., z)),
                    at(Vec3::new(x, top, z)),
                    0.12,
                );
                self.beam(
                    WOOD,
                    at(Vec3::new(x, -0.18, z)),
                    at(Vec3::new(x * 0.7, -1.5, z * 0.7)),
                    0.10,
                );
                let start = at(Vec3::new(x, -0.3, z));
                let mut anchor = at(Vec3::new(x * 1.1, 5., z * 1.1));
                while den.air(anchor) < 0. {
                    anchor.y += 0.25;
                }
                self.beam(ROPE, start, anchor, 0.043);
                for j in 0..6 {
                    let y = top - 0.20 + j as f32 * 0.025;
                    self.beam(
                        ROPE,
                        at(Vec3::new(x - 0.13, y, z)),
                        at(Vec3::new(x + 0.13, y, z)),
                        0.012,
                    );
                }
            }
        }
        for side in [-1., 1.] {
            let boards = (h.depth / 0.23).ceil() as usize;
            for i in 0..boards {
                let z = -h.depth / 2. + (i as f32 + 0.5) * h.depth / boards as f32;
                let roof = h.roof_height(side * h.width / 2., z);
                for (bottom, top) in [(0., h.window.x), (h.window.y, roof - 0.04)] {
                    if top <= bottom {
                        continue;
                    }
                    self.block(
                        WOOD,
                        at(Vec3::new(side * h.width / 2., (top + bottom) * 0.5, z)),
                        Vec3::new(0.09, (top - bottom) * rise, h.depth / boards as f32 - 0.012),
                        q,
                        true,
                    );
                }
            }
            self.beam(
                WOOD,
                at(Vec3::new(side * h.width / 2., h.window.x, -h.depth / 2.)),
                at(Vec3::new(side * h.width / 2., h.window.x, h.depth / 2.)),
                0.075,
            );
        }
        let boards = (h.width / 0.22).ceil() as usize;
        for i in 0..boards {
            let x = -h.width / 2. + (i as f32 + 0.5) * h.width / boards as f32;
            for side in [-1., 1.] {
                if side > 0. && x.abs() < 0.88 {
                    continue;
                }
                let z = side * h.depth / 2.;
                let length = h.roof_height(x, z) - self.rng.range(0.04, 0.38);
                let lean = self.rng.range(-0.018, 0.018);
                self.block(
                    WOOD,
                    at(Vec3::new(x, length * 0.5, z)),
                    Vec3::new(h.width / boards as f32 - 0.012, length * rise, 0.09),
                    q * Quat::from_rotation_z(lean),
                    true,
                );
            }
        }
        // Continuous, independently shifted/sloping ridges, never roof variants.
        let nx = ((h.width + 1.) / 0.38).ceil() as usize;
        let nz = ((h.depth + 0.8) / 0.30).ceil() as usize;
        for ix in 0..nx {
            for iz in 0..nz {
                let x = -h.width / 2. - 0.35 + ix as f32 * (h.width + 0.7) / (nx - 1) as f32;
                let z = -h.depth / 2. - 0.3 + iz as f32 * (h.depth + 0.6) / (nz - 1) as f32;
                let slope = (h.roof_height(x + 0.01, z) - h.roof_height(x - 0.01, z)) / 0.02;
                let warp = self.rng.range(-0.018, 0.018);
                let mat = if self.rng.unit() < 0.055 { CLOTH } else { WOOD };
                self.block(
                    mat,
                    at(Vec3::new(x, h.roof_height(x, z) + warp, z)),
                    Vec3::new(0.51, 0.045, 0.41),
                    q * Quat::from_rotation_z((slope * rise).atan())
                        * Quat::from_rotation_x(-(h.roof.w * rise).atan()),
                    true,
                );
            }
        }
        for z in [-h.depth / 2. - 0.2, h.depth / 2. + 0.2] {
            for side in [-1., 1.] {
                let x = side * (h.width / 2. + 0.2);
                self.beam(
                    WOOD,
                    at(Vec3::new(h.roof.z, h.roof_height(h.roof.z, z) - 0.07, z)),
                    at(Vec3::new(x, h.roof_height(x, z) - 0.07, z)),
                    0.075,
                );
            }
        }
    }
    fn house(&mut self, h: &House, den: &Den) {
        self.structure(h, den, h.rise);
        let q = Quat::from_rotation_y(h.yaw);
        let at = |v| h.at + q * v;
        // Four low sleeping nests, sized from the same stature as the residents.
        // The biggest dimension is only 1.35 goblin heights; no human bed frames.
        let stature = super::super::goblins::HEIGHT;
        let mut nests: Vec<Vec3> = Vec::new();
        for _ in 0..4 {
            let mut chosen = None;
            for _ in 0..100 {
                let side = if self.rng.unit() < 0.5 { -1. } else { 1. };
                let p = Vec3::new(
                    side * (h.width / 2. - 0.45),
                    0.,
                    self.rng.range(-h.depth / 2. + 0.45, h.depth / 2. - 0.60),
                );
                if nests.iter().all(|other| other.distance(p) > 0.78) {
                    chosen = Some(p);
                    break;
                }
            }
            let p =
                chosen.unwrap_or_else(|| Vec3::new(h.width / 2. - 0.45, 0., h.depth / 2. - 0.6));
            nests.push(p);
            let side = p.x.signum();
            let twist = q * Quat::from_rotation_y(self.rng.range(-0.45, 0.45));
            for k in 0..100 {
                let a = self.rng.range(0., TAU);
                let ring = self.rng.range(0.30, 1.);
                let c = Vec3::new(
                    a.cos() * stature * 0.47 * ring,
                    0.018 + ring * ring * 0.065,
                    a.sin() * stature * 0.675 * ring,
                );
                let tangent = Vec3::new(-a.sin(), self.rng.range(-0.12, 0.12), a.cos())
                    * self.rng.range(0.04, 0.115);
                self.beam(
                    if k % 5 == 0 { WOOD } else { ROPE },
                    at(p) + twist * (c - tangent),
                    at(p) + twist * (c + tangent),
                    if k % 5 == 0 { 0.008 } else { 0.003 },
                );
            }
            self.hide(
                at(p + Vec3::Y * 0.025),
                Vec2::new(stature * 0.34, stature * 0.50),
                twist,
            );
            // Mismatched, hand-sewn scraps pinned over one edge of the hide.
            for k in 0..9 {
                let z = -stature * 0.32 + k as f32 * stature * 0.08;
                self.beam(
                    ROPE,
                    at(p) + twist * Vec3::new(-stature * 0.23, 0.065, z),
                    at(p) + twist * Vec3::new(-stature * 0.15, 0.064, z + 0.014),
                    0.003,
                );
            }
            self.pot(at(p + Vec3::new(-side * 0.32, 0., -0.23)), 0.043, 0.075);
        }
        // A split slab on short root legs; cups and bowls sit at goblin waist height.
        let table = Vec3::new(
            self.rng.range(-0.35, 0.35),
            stature * self.rng.range(0.43, 0.55),
            -h.depth / 2. + self.rng.range(0.55, 0.85),
        );
        for i in 0..3 {
            let tilt = self.rng.range(-0.035, 0.035);
            self.block(
                WOOD,
                at(table + Vec3::new(0., tilt * 0.15, (i as f32 - 1.) * 0.13)),
                Vec3::new(0.78 + tilt, 0.055, 0.14),
                q * Quat::from_rotation_y(tilt),
                true,
            );
        }
        for x in [-0.27, 0.27] {
            self.beam(
                WOOD,
                at(table + Vec3::new(x * 1.16, -table.y, 0.)),
                at(table + Vec3::new(x, 0., 0.)),
                0.06,
            );
        }
        for _ in 0..5 {
            let offset = Vec3::new(
                self.rng.range(-0.29, 0.29),
                0.03,
                self.rng.range(-0.12, 0.12),
            );
            let radius = self.rng.range(0.025, 0.05);
            let height = self.rng.range(0.035, 0.075);
            self.pot(at(table + offset), radius, height);
        }
        // Unequal stump seats, scavenged sacks and a low sagging storage shelf.
        for x in [-0.42, 0.45] {
            self.beam(
                WOOD,
                at(Vec3::new(x, 0., -h.depth / 2. + 1.05)),
                at(Vec3::new(x + 0.018, stature * 0.27, -h.depth / 2. + 1.05)),
                0.095,
            );
        }
        let shelf = Vec3::new(-0.15, stature * 1.18, -h.depth / 2. + 0.20);
        self.block(
            WOOD,
            at(shelf),
            Vec3::new(1.20, 0.045, 0.27),
            q * Quat::from_rotation_z(-0.025),
            true,
        );
        for x in [-0.48, 0.48] {
            self.beam(
                ROPE,
                at(shelf + Vec3::new(x, 0., 0.12)),
                at(shelf + Vec3::new(x, 0.42, -0.17)),
                0.009,
            );
        }
        for k in 0..6 {
            let offset = self.rng.range(-0.03, 0.03);
            let height = self.rng.range(0.07, 0.15);
            self.pot(
                at(shelf + Vec3::new(-0.47 + k as f32 * 0.18, 0.03, offset)),
                0.055,
                height,
            );
        }
        for k in 0..3 {
            let p = Vec3::new(-1.05 - k as f32 * 0.19, 0., -h.depth / 2. + 0.35);
            let size = self.rng.range(0.10, 0.15);
            self.shape(
                CLOTH,
                Sphere::new(1.).mesh().uv(16, 12),
                at(p + Vec3::Y * size),
                Vec3::new(size, size * 1.15, size * 0.8),
                q,
                Vec3::new(0.63, 0.54, 0.36),
                false,
            );
            for turn in 0..3 {
                self.beam(
                    ROPE,
                    at(p + Vec3::new(-0.035, size * 1.9 + turn as f32 * 0.005, 0.)),
                    at(p + Vec3::new(0.035, size * 1.9 + turn as f32 * 0.005, 0.)),
                    0.005,
                );
            }
        }
        // Ochre claw marks directly on salvaged boards, with dangling tooth charms.
        for _ in 0..18 {
            let y = self.rng.range(0.18, 0.72);
            let z = self.rng.range(-1.0, 0.7);
            self.beam(
                CLAY,
                at(Vec3::new(h.width / 2. - 0.065, y, z)),
                at(Vec3::new(h.width / 2. - 0.065, y + 0.075, z + 0.035)),
                0.005,
            );
        }
        for k in 0..7 {
            let p = Vec3::new(h.width / 2. - 0.18, 1.65, -0.9 + k as f32 * 0.14);
            let drop = self.rng.range(0.12, 0.37);
            self.beam(ROPE, at(p), at(p - Vec3::Y * drop), 0.004);
            self.shape(
                CLAY,
                Cone::new(0.022, 0.09).mesh().resolution(9).build(),
                at(p - Vec3::Y * (drop + 0.035)),
                Vec3::ONE,
                q * Quat::from_rotation_z(3.0),
                Vec3::new(1.6, 1.5, 1.2),
                false,
            );
        }
        self.dress_house(h);
        self.lamp(at(Vec3::new(0., 2.35, 0.2)));
        self.lamp(at(Vec3::new(0., 2.05, h.depth * 0.5 + 0.35)));
        // Threshold nails and iron nail heads on every porch board.
        for i in 0..26 {
            for z in [-h.depth / 2. - 0.4, h.depth / 2. + 0.4] {
                self.shape(
                    IRON,
                    Sphere::new(0.015).mesh().uv(8, 6),
                    at(Vec3::new(
                        -h.width / 2. + i as f32 * h.width / 25.,
                        0.004,
                        z,
                    )),
                    Vec3::new(1., 0.3, 1.),
                    q,
                    Vec3::ONE,
                    false,
                );
            }
        }
    }
}
pub(super) fn build(den: &Den) -> Built {
    let mut w = Workshop {
        g: (0..10).map(|_| Geometry::default()).collect(),
        solid: Geometry::default(),
        lamps: Vec::new(),
        relics: Vec::new(),
        rng: Rng(den.seed),
        wall: None,
        junctions: den
            .spans
            .iter()
            .copied()
            .chain(den.tunnels.iter().map(|path| Span {
                a: path[0] - Vec3::Y * 1.45,
                b: path[1] - Vec3::Y * 1.45,
                width: 2.4,
                sag: 0.,
            }))
            .collect(),
    };
    let shell = excavation::shell(den);
    w.wall = Some(RayMesh::new(shell.triangles()));
    append(&mut w.solid, &shell, Transform::default());
    w.g[STONE] = shell;
    for _ in 0..36 {
        let angle = w.rng.range(0., TAU);
        let radius = w.rng.range(5., 19.);
        let mut root = Vec3::new(angle.cos() * radius, 0., angle.sin() * radius);
        while den.air(root) < 0. {
            root.y += 0.2;
        }
        let length = w.rng.range(0.8, 3.8);
        let width = w.rng.range(0.18, 0.55);
        w.shape(
            STONE,
            Cone::new(width, length).mesh().resolution(12).build(),
            root - Vec3::Y * (length * 0.5 - 0.15),
            Vec3::ONE,
            Quat::from_rotation_z(std::f32::consts::PI),
            Vec3::splat(0.85),
            true,
        );
    }
    for l in &den.landings {
        w.deck(l.at, l.size.x, l.size.y, Quat::from_rotation_y(l.yaw));
    }
    for span in &den.spans {
        w.bridge(*span, true);
    }
    for h in &den.houses {
        w.house(h, den);
    }
    w.shrine(&den.shrine, den);
    w.food_hall(&den.food_hall, den);
    w.passages(den);
    for l in den.landings.iter().skip(den.houses.len() + 2) {
        w.pot(l.at + Vec3::new(0.7, 0., 0.6), 0.16, 0.32);
        w.lamp(l.at + Vec3::Y * 1.8 + Vec3::X * 0.9);
    }
    for (kind, g) in w.g.iter_mut().enumerate().skip(1) {
        if kind != EARTH {
            g.finish_normals();
        }
    }
    let floors = RayMesh::new_floor(w.solid.triangles().filter(|t| {
        let [a, b, c] = t.0;
        (b - a).cross(c - a).y > 0.00001
    }));
    // Walkable tops are resolved by the support solver. Including them in
    // body spheres would falsely obstruct a capsule standing on a ramp.
    // Keep steep banks, walls, step risers and every underside/ceiling.
    let collision = RayMesh::new(w.solid.triangles().filter(|t| {
        let [a, b, c] = t.0;
        let n = (b - a).cross(c - a);
        n.y <= n.length() * std::f32::consts::FRAC_1_SQRT_2
    }));
    Built {
        meshes: w.g,
        collision,
        floors,
        lamps: w.lamps,
        relics: w.relics,
    }
}

pub(super) fn materials(
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
) -> Vec<Handle<StandardMaterial>> {
    let colors = [
        Vec3::new(0.48, 0.43, 0.36),
        Vec3::new(0.36, 0.30, 0.23),
        Vec3::new(0.40, 0.32, 0.18),
        Vec3::new(0.17, 0.15, 0.12),
        Vec3::new(0.40, 0.23, 0.13),
        Vec3::new(0.28, 0.30, 0.20),
        Vec3::new(1., 0.40, 0.08),
        Vec3::ONE,
        Vec3::new(0.72, 0.65, 0.49),
        Vec3::new(0.34, 0.25, 0.16),
    ];
    colors
        .iter()
        .enumerate()
        .map(|(kind, base)| {
            let size = 256;
            let mut data = Vec::new();
            let mut normals = Vec::new();
            let mut rough = Vec::new();
            let height = |x: f32, y: f32| {
                let noise = ((x * 127.1 + y * 311.7).sin() * 43_758.547).fract().abs();
                match kind {
                    WOOD => geology::wood_texture(Vec2::new(x, y)),
                    ROPE | CLOTH => (x * 2.).sin() * (y * 2.).sin() * 0.26 + noise * 0.1,
                    STONE | EARTH => geology::texture(Vec2::new(x, y)),
                    PAINT => 0.5,
                    CLAY => (y * 1.4).sin() * 0.09 + noise * 0.16,
                    _ => noise * 0.25,
                }
            };
            for y in 0..size {
                for x in 0..size {
                    let h = height(x as f32, y as f32);
                    let c = *base
                        * if kind == STONE || kind == EARTH {
                            0.48 + h * 0.95
                        } else if kind == PAINT {
                            1.
                        } else {
                            0.80 + h * 0.4
                        };
                    data.extend([c.x, c.y, c.z, 1.].map(|v| (v.clamp(0., 1.) * 255.) as u8));
                    let strength = if kind == WOOD {
                        2.4
                    } else if kind == STONE || kind == EARTH {
                        3.2
                    } else {
                        0.25
                    };
                    let n = Vec3::new(
                        (h - height(x as f32 + 1., y as f32)) * strength,
                        (h - height(x as f32, y as f32 + 1.)) * strength,
                        1.,
                    )
                    .normalize()
                        * 0.5
                        + Vec3::splat(0.5);
                    normals.extend([n.x, n.y, n.z, 1.].map(|v| (v * 255.) as u8));
                    rough.extend([
                        255,
                        (220. + h * 20.).clamp(0., 255.) as u8,
                        if kind == IRON { 190 } else { 0 },
                        255,
                    ]);
                }
            }
            let mut texture = |data: Vec<u8>, srgb: bool, normal_map: bool| {
                let mut image = Image::new(
                    Extent3d {
                        width: size,
                        height: size,
                        depth_or_array_layers: 1,
                    },
                    TextureDimension::D2,
                    data.clone(),
                    if srgb {
                        TextureFormat::Rgba8UnormSrgb
                    } else {
                        TextureFormat::Rgba8Unorm
                    },
                    RenderAssetUsages::default(),
                );
                // Complete mip pyramids suppress grain/normal shimmer on thin
                // planks. Average color in linear light and renormalize normals.
                let mut pyramid = data.clone();
                let mut level = data;
                let mut side = size as usize;
                let mut mip_count = 1;
                while side > 1 {
                    let next_side = side / 2;
                    let mut next = Vec::with_capacity(next_side * next_side * 4);
                    for y in 0..next_side {
                        for x in 0..next_side {
                            let mut channels = [0.0_f32; 4];
                            for dy in 0..2 {
                                for dx in 0..2 {
                                    let index = ((y * 2 + dy) * side + x * 2 + dx) * 4;
                                    for c in 0..4 {
                                        let v = level[index + c] as f32 / 255.;
                                        channels[c] += if srgb && c < 3 {
                                            v.powf(2.2) * 0.25
                                        } else {
                                            v * 0.25
                                        };
                                    }
                                }
                            }
                            if srgb {
                                for c in &mut channels[..3] {
                                    *c = c.powf(1. / 2.2);
                                }
                            }
                            if normal_map {
                                let n = (Vec3::new(channels[0], channels[1], channels[2]) * 2.
                                    - Vec3::ONE)
                                    .normalize_or(Vec3::Z)
                                    * 0.5
                                    + Vec3::splat(0.5);
                                channels[..3].copy_from_slice(&n.to_array());
                            }
                            next.extend(channels.map(|v| (v.clamp(0., 1.) * 255.).round() as u8));
                        }
                    }
                    pyramid.extend_from_slice(&next);
                    level = next;
                    side = next_side;
                    mip_count += 1;
                }
                image.data = Some(pyramid);
                image.texture_descriptor.mip_level_count = mip_count;
                image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                    address_mode_u: ImageAddressMode::Repeat,
                    address_mode_v: ImageAddressMode::Repeat,
                    anisotropy_clamp: 8,
                    ..ImageSamplerDescriptor::linear()
                });
                images.add(image)
            };
            materials.add(StandardMaterial {
                base_color_texture: Some(texture(data, true, false)),
                normal_map_texture: Some(texture(normals, false, true)),
                metallic_roughness_texture: Some(texture(rough, false, false)),
                metallic: if kind == IRON { 0.75 } else { 0. },
                perceptual_roughness: 1.,
                reflectance: if kind == WOOD || kind == EARTH {
                    0.06
                } else {
                    0.2
                },
                double_sided: kind == STONE || kind == PAINT,
                cull_mode: if kind == STONE || kind == PAINT {
                    None
                } else {
                    Some(bevy::render::render_resource::Face::Back)
                },
                emissive: if kind == GLOW {
                    LinearRgba::new(8., 2.4, 0.2, 1.)
                } else {
                    LinearRgba::BLACK
                },
                ..default()
            })
        })
        .collect()
}

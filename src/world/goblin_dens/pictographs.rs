//! Fictional den folklore, told through reusable strokes rather than image stamps.
use super::*;
pub(super) const COUNT: usize = 36;
type Line = Vec<Vec2>;
struct Sketch(Vec<Line>);
impl Sketch {
    fn line(&mut self, p: &[(f32, f32)]) {
        self.0
            .push(p.iter().map(|&(x, y)| Vec2::new(x, y)).collect());
    }
    fn arc(&mut self, x: f32, y: f32, rx: f32, ry: f32, start: f32, end: f32) {
        self.0.push(
            (0..=18)
                .map(|i| {
                    let a = start + (end - start) * i as f32 / 18.;
                    Vec2::new(x + rx * a.cos(), y + ry * a.sin())
                })
                .collect(),
        );
    }
    fn circle(&mut self, x: f32, y: f32, r: f32) {
        self.arc(x, y, r, r, 0., TAU);
    }
    fn head(&mut self, x: f32, y: f32, s: f32) {
        self.line(&[
            (x - s * 0.7, y),
            (x - s * 1.4, y + s * 1.3),
            (x - s * 0.35, y + s * 0.7),
            (x + s * 0.35, y + s * 0.7),
            (x + s * 1.4, y + s * 1.3),
            (x + s * 0.7, y),
            (x, y - s * 0.6),
            (x - s * 0.7, y),
        ]);
        for sign in [-1., 1.] {
            self.line(&[
                (x + sign * s * 0.3, y + s * 0.35),
                (x + sign * s * 0.25, y + s * 0.15),
            ]);
        }
    }
    fn goblin(&mut self, x: f32, y: f32, s: f32, raised: bool) {
        self.head(x, y + s * 0.6, s * 0.25);
        self.line(&[
            (x, y + s * 0.42),
            (x, y - s * 0.25),
            (x - s * 0.28, y - s * 0.55),
        ]);
        self.line(&[(x, y - s * 0.25), (x + s * 0.31, y - s * 0.5)]);
        let hands = if raised { 0.65 } else { -0.08 };
        self.line(&[
            (x - s * 0.4, y + s * hands),
            (x, y + s * 0.22),
            (x + s * 0.4, y + s * hands),
        ]);
    }
    fn bag(&mut self, x: f32, y: f32, s: f32) {
        self.line(&[
            (x - s * 0.35, y + s * 0.6),
            (x + s * 0.35, y + s * 0.6),
            (x + s * 0.2, y + s * 0.35),
            (x + s * 0.6, y - s * 0.2),
            (x + s * 0.4, y - s * 0.5),
            (x - s * 0.4, y - s * 0.5),
            (x - s * 0.6, y - s * 0.2),
            (x - s * 0.2, y + s * 0.35),
            (x - s * 0.35, y + s * 0.6),
        ]);
        self.line(&[(x - s * 0.35, y + s * 0.32), (x + s * 0.4, y + s * 0.32)]);
    }
    fn blade(&mut self, x: f32, y: f32, s: f32, lean: f32) {
        let before = self.0.len();
        self.line(&[
            (-0.04, -0.35),
            (0.04, -0.35),
            (0.04, 0.),
            (0.08, 0.30),
            (0., 0.65),
            (-0.08, 0.30),
            (-0.04, 0.),
            (-0.04, -0.35),
        ]);
        self.line(&[(-0.18, 0.), (0.18, 0.)]);
        for line in &mut self.0[before..] {
            for p in line {
                let a = *p * s;
                *p = Vec2::new(
                    x + a.x * lean.cos() - a.y * lean.sin(),
                    y + a.x * lean.sin() + a.y * lean.cos(),
                );
            }
        }
    }
    fn mushroom(&mut self, x: f32, y: f32, s: f32) {
        self.arc(x, y, s * 0.5, s * 0.35, 0., std::f32::consts::PI);
        self.line(&[(x - s * 0.5, y), (x + s * 0.5, y)]);
        self.line(&[
            (x - s * 0.07, y),
            (x - s * 0.1, y - s * 0.48),
            (x + s * 0.1, y - s * 0.48),
            (x + s * 0.07, y),
        ]);
        for dx in [-0.2, 0., 0.2] {
            self.circle(x + s * dx, y + s * 0.12, s * 0.025);
        }
    }
    fn eye(&mut self, x: f32, y: f32, s: f32) {
        self.line(&[
            (x - s, y),
            (x, y + s * 0.45),
            (x + s, y),
            (x, y - s * 0.45),
            (x - s, y),
        ]);
        self.circle(x, y, s * 0.2);
    }
    fn arrow(&mut self, a: Vec2, b: Vec2) {
        let dir = (b - a).normalize_or_zero();
        let side = Vec2::new(-dir.y, dir.x);
        self.0.push(vec![a, b]);
        self.0.push(vec![
            b - dir * 0.09 + side * 0.055,
            b,
            b - dir * 0.09 - side * 0.055,
        ]);
    }
    fn skull(&mut self, x: f32, y: f32, s: f32) {
        self.arc(x, y, s * 0.5, s * 0.45, 0., std::f32::consts::PI);
        self.line(&[
            (x - s * 0.5, y),
            (x - s * 0.2, y - s * 0.2),
            (x - s * 0.18, y - s * 0.4),
            (x + s * 0.18, y - s * 0.4),
            (x + s * 0.2, y - s * 0.2),
            (x + s * 0.5, y),
        ]);
        for sign in [-1., 1.] {
            self.circle(x + sign * s * 0.2, y - s * 0.01, s * 0.07);
        }
    }
}
pub(super) fn generate(motif: usize, rng: &mut Rng) -> Vec<Line> {
    let mut s = Sketch(Vec::new());
    let count = 3 + (rng.unit() * 4.) as usize;
    let lean = rng.range(-0.18, 0.18);
    match motif {
        5 => {
            // Theft: hooked fingers lifting a purse, falling coins.
            s.bag(0.14, -0.02, 0.44);
            s.line(&[
                (-0.47, 0.32),
                (-0.13, 0.32),
                (0.02, 0.22),
                (0.08, 0.26),
                (0.06, 0.36),
                (-0.08, 0.43),
                (-0.47, 0.43),
            ]);
            for i in 0..count {
                s.circle(
                    -0.03 + i as f32 * 0.07,
                    -0.32 - (i % 2) as f32 * 0.07,
                    0.025,
                );
            }
        }
        6 => {
            // Broken lock, snapped chain and triumphant stolen key.
            s.line(&[
                (-0.27, -0.25),
                (0.14, -0.25),
                (0.14, 0.1),
                (-0.27, 0.1),
                (-0.27, -0.25),
            ]);
            s.arc(-0.06, 0.11, 0.13, 0.20, 0.2, 2.7);
            s.line(&[(-0.16, 0.32), (-0.04, 0.38), (-0.12, 0.43)]);
            s.circle(-0.06, -0.02, 0.034);
            s.circle(0.31, 0.31, 0.075);
            s.line(&[(0.31, 0.24), (0.31, -0.14), (0.43, -0.14), (0.43, -0.07)]);
            for i in 0..count {
                s.arc(-0.4 + i as f32 * 0.15, -0.38, 0.065, 0.025, 0., 5.3);
            }
        }
        7 => {
            // A little thief climbing through a large folk's window.
            s.line(&[(-0.35, -0.35), (-0.35, 0.43), (0.35, 0.43), (0.35, -0.35)]);
            s.line(&[(-0.35, -0.12), (0.35, -0.12)]);
            s.goblin(0., 0.01, 0.43, false);
            s.bag(0.3, -0.26, 0.22);
        }
        8 => {
            // Fleeing thief, stolen sack and deliberately misleading footprints.
            s.goblin(-0.17, 0.06, 0.44, false);
            s.bag(0.16, 0.08, 0.37);
            for i in 0..count {
                let x = -0.4 + i as f32 * 0.16;
                s.line(&[(x, -0.33), (x + 0.065, -0.31)]);
            }
            s.arrow(Vec2::new(-0.35, 0.4), Vec2::new(0.4, 0.4));
        }
        9 => {
            // Goblin wearing a stolen, absurdly oversized crown.
            s.head(0., -0.02, 0.21);
            s.line(&[
                (-0.32, 0.2),
                (-0.4, 0.45),
                (-0.18, 0.32),
                (0., 0.5),
                (0.17, 0.32),
                (0.4, 0.45),
                (0.32, 0.2),
                (-0.32, 0.2),
            ]);
            s.line(&[(-0.12, -0.09), (0., -0.15), (0.12, -0.09)]);
        }
        10 => {
            s.blade(0., -0.02, 0.68, 0.7 + lean);
            s.blade(0., -0.02, 0.68, -0.7 + lean);
            s.head(0., 0.36, 0.07);
        }
        11 => {
            // Spear through a rival's shield.
            s.line(&[
                (-0.3, 0.26),
                (0.2, 0.26),
                (0.16, -0.12),
                (-0.04, -0.34),
                (-0.28, -0.12),
                (-0.3, 0.26),
            ]);
            s.line(&[
                (-0.42, -0.38),
                (0.35, 0.38),
                (0.27, 0.23),
                (0.2, 0.31),
                (0.35, 0.38),
            ]);
            s.line(&[(-0.08, 0.14), (0.03, 0.02), (-0.1, -0.11)]);
        }
        12 => {
            // Ambush: small hunters surround a large stick-figure traveler.
            s.goblin(-0.33, -0.02, 0.26, false);
            s.goblin(0.34, -0.02, 0.26, false);
            s.circle(0., 0.3, 0.065);
            s.line(&[(0., 0.23), (0., -0.2), (-0.15, -0.4)]);
            s.line(&[(0., -0.2), (0.15, -0.4)]);
            s.arrow(Vec2::new(-0.22, 0.1), Vec2::new(-0.04, 0.03));
            s.arrow(Vec2::new(0.23, 0.1), Vec2::new(0.04, 0.03));
        }
        13 => {
            // Clan duel: fighters, jagged blades, witnessed by clan marks.
            s.goblin(-0.28, 0., 0.34, false);
            s.goblin(0.28, 0., 0.34, false);
            s.blade(-0.10, 0.1, 0.30, -0.55);
            s.blade(0.10, 0.1, 0.30, 0.55);
            s.eye(-0.30, 0.39, 0.1);
            s.line(&[(0.22, 0.32), (0.3, 0.46), (0.38, 0.32)]);
        }
        14 => {
            s.skull(0., 0.08, 0.42);
            s.line(&[(0., -0.43), (0., 0.45)]);
            s.blade(-0.28, -0.03, 0.47, 0.25);
            s.blade(0.28, -0.03, 0.47, -0.25);
        }
        15 => {
            // Torn war banner with a painted bite-mark and raid tally.
            s.line(&[
                (-0.3, -0.45),
                (-0.3, 0.46),
                (0.32, 0.40),
                (0.16, 0.17),
                (0.32, 0.05),
                (-0.3, 0.09),
            ]);
            s.line(&[
                (-0.18, 0.3),
                (-0.1, 0.19),
                (-0.02, 0.29),
                (0.06, 0.18),
                (0.14, 0.28),
            ]);
            for i in 0..count {
                let x = -0.05 + i as f32 * 0.07;
                s.line(&[(x, -0.17), (x + 0.01, -0.34)]);
            }
        }
        16 => {
            // The Many-Handed Thief: patron of cunning and taking.
            s.head(0., 0.27, 0.13);
            s.line(&[(0., 0.15), (0., -0.35)]);
            for row in 0..3 {
                for sign in [-1., 1.] {
                    let y = 0.12 - row as f32 * 0.16;
                    let x = sign * (0.28 + row as f32 * 0.045);
                    s.line(&[(0., y), (x, y + 0.11), (x + sign * 0.06, y + 0.20)]);
                    for finger in 0..3 {
                        let dx = finger as f32 * 0.025;
                        s.line(&[(x, y + 0.11), (x + sign * (0.025 + dx), y + 0.21)]);
                    }
                }
            }
            s.circle(0., 0.30, 0.035);
        }
        17 => {
            // The Deep Maw: mountain-mouth receives stolen offerings.
            s.line(&[
                (-0.48, -0.25),
                (-0.19, 0.43),
                (0.05, 0.3),
                (0.26, 0.46),
                (0.49, -0.25),
                (-0.48, -0.25),
            ]);
            s.arc(0., -0.08, 0.26, 0.22, 0., std::f32::consts::PI);
            for i in 0..count {
                let x = -0.2 + i as f32 * 0.4 / (count - 1) as f32;
                s.line(&[(x, 0.05), (x + 0.03, -0.08), (x + 0.055, 0.05)]);
            }
            s.bag(0., -0.34, 0.17);
        }
        18 => {
            // Spore Mother: sacred fungus, branching roots and small lives.
            s.mushroom(0., 0.12, 0.74);
            for i in 0..count {
                let a = i as f32 * TAU / count as f32;
                let x = a.cos() * 0.43;
                let y = 0.06 + a.sin() * 0.38;
                s.circle(x, y, 0.025);
            }
            s.line(&[(-0.06, -0.23), (-0.23, -0.42), (-0.4, -0.35)]);
            s.line(&[(0.06, -0.23), (0.25, -0.40), (0.39, -0.32)]);
        }
        19 => {
            // Ancestor ladder: generations of masks descend into the stone.
            for x in [-0.26, 0.26] {
                s.line(&[(x, -0.46), (x, 0.45)]);
            }
            for i in 0..3 {
                let y = -0.31 + i as f32 * 0.29;
                s.line(&[(-0.26, y), (0.26, y)]);
                s.head(0., y + 0.10, 0.07);
            }
        }
        20 => {
            // Offerings beneath the hidden moon.
            s.arc(0., 0.25, 0.18, 0.18, 0.3, 5.8);
            s.arc(0.08, 0.25, 0.17, 0.15, 1.5, 4.7);
            s.line(&[(-0.35, -0.14), (0.35, -0.14), (-0.28, -0.2), (-0.28, -0.4)]);
            s.line(&[(0.28, -0.2), (0.28, -0.4)]);
            s.bag(-0.13, -0.06, 0.15);
            s.mushroom(0.17, -0.015, 0.19);
        }
        21 => {
            // Blind stone eye: crossed pupil ward to hide a den from outsiders.
            s.eye(0., 0., 0.40);
            s.line(&[(-0.14, -0.25), (0.14, 0.25)]);
            s.line(&[(-0.14, 0.25), (0.14, -0.25)]);
            for sign in [-1., 1.] {
                s.line(&[(sign * 0.35, 0.27), (sign * 0.42, 0.38)]);
            }
        }
        22 => {
            // Clan feast: communal pot, mushrooms and a crooked spoon.
            s.arc(0., 0., 0.3, 0.25, std::f32::consts::PI, TAU);
            s.line(&[(-0.3, 0.), (0.3, 0.)]);
            s.mushroom(-0.11, 0.16, 0.22);
            s.mushroom(0.14, 0.11, 0.2);
            s.line(&[(0.22, -0.10), (0.39, 0.39)]);
            s.circle(0.40, 0.40, 0.045);
            for i in 0..count {
                let x = -0.25 + i as f32 * 0.1;
                s.line(&[(x, -0.36), (x + 0.04, -0.28), (x + 0.08, -0.36)]);
            }
        }
        23 => {
            // Bridge-builder's boast, a goblin above a dangerous chasm.
            s.line(&[(-0.45, 0.12), (-0.2, -0.02), (0.2, -0.02), (0.45, 0.12)]);
            s.line(&[(-0.45, -0.06), (-0.2, -0.18), (0.2, -0.18), (0.45, -0.06)]);
            for i in 0..7 {
                let x = -0.3 + i as f32 * 0.1;
                s.line(&[(x, -0.01), (x, -0.17)]);
            }
            s.goblin(0., 0.18, 0.24, true);
            s.line(&[
                (-0.35, -0.28),
                (-0.15, -0.43),
                (0., -0.28),
                (0.2, -0.43),
                (0.37, -0.27),
            ]);
        }
        24 => {
            // Teeth-for-coins barter, rendered as an exchange rather than writing.
            s.line(&[(-0.42, 0.14), (-0.21, 0.14), (-0.30, -0.14), (-0.42, 0.14)]);
            s.circle(0.29, 0., 0.1);
            s.arrow(Vec2::new(-0.18, 0.25), Vec2::new(0.19, 0.25));
            s.arrow(Vec2::new(0.19, -0.25), Vec2::new(-0.18, -0.25));
        }
        25 => {
            // Beloved tunnel rat: exaggerated ears, whiskers, curling tail.
            s.arc(-0.03, -0.04, 0.24, 0.15, 0., TAU);
            s.line(&[(0.14, 0.04), (0.39, -0.03), (0.16, -0.12)]);
            s.circle(0.13, 0.14, 0.09);
            s.circle(0.25, 0.04, 0.015);
            s.arc(-0.34, -0.08, 0.12, 0.17, 0., 4.8);
            for i in 0..3 {
                s.line(&[(0.31, -0.03), (0.48, -0.12 + i as f32 * 0.09)]);
            }
        }
        26 => {
            // Brood-and-nest kinship mark.
            s.arc(0., -0.1, 0.42, 0.2, std::f32::consts::PI, TAU);
            for i in 0..count {
                let x = -0.3 + i as f32 * 0.6 / (count - 1) as f32;
                s.head(x, 0., 0.06);
            }
            s.head(0., 0.29, 0.12);
        }
        27 => {
            // Clan knot: three interlocked fangs and a central ownership eye.
            for i in 0..3 {
                let a = i as f32 * TAU / 3.;
                let before = s.0.len();
                s.line(&[(-0.12, 0.17), (0.12, 0.17), (0., 0.43), (-0.12, 0.17)]);
                for p in &mut s.0[before] {
                    let v = *p;
                    *p = Vec2::new(v.x * a.cos() - v.y * a.sin(), v.x * a.sin() + v.y * a.cos());
                }
            }
            s.eye(0., 0., 0.10);
        }
        28 => {
            // Stolen fire carried into the dark, guarded by grasping hands.
            s.line(&[
                (-0.2, -0.14),
                (-0.13, 0.07),
                (-0.06, 0.03),
                (0.04, 0.4),
                (0.16, 0.13),
                (0.23, -0.06),
                (0.10, -0.21),
                (-0.2, -0.14),
            ]);
            s.line(&[(-0.38, -0.33), (0.28, -0.2)]);
            s.line(&[(-0.28, -0.2), (0.38, -0.33)]);
        }
        29 => {
            // Falling-stone warning over a very unhappy goblin.
            s.goblin(0., -0.19, 0.25, true);
            for _ in 0..count {
                let x = rng.range(-0.35, 0.35);
                let y = rng.range(0.10, 0.42);
                s.line(&[
                    (x - 0.05, y),
                    (x, y + 0.08),
                    (x + 0.07, y + 0.015),
                    (x + 0.02, y - 0.045),
                    (x - 0.05, y),
                ]);
            }
        }
        30 => {
            // Shaman dancing with rattles, mushroom crown and ritual rays.
            s.goblin(0., -0.05, 0.43, true);
            s.mushroom(0., 0.35, 0.24);
            for sign in [-1., 1.] {
                s.circle(sign * 0.26, 0.27, 0.055);
                s.line(&[(sign * 0.20, 0.10), (sign * 0.26, 0.27)]);
            }
            for i in 0..count {
                let a = i as f32 * TAU / count as f32;
                s.line(&[
                    (a.cos() * 0.37, a.sin() * 0.38),
                    (a.cos() * 0.48, a.sin() * 0.48),
                ]);
            }
        }
        31 => {
            // Hoard map: cave, winding route, bones and a concealed sack.
            s.line(&[(-0.46, 0.12), (-0.38, 0.40), (-0.22, 0.13), (-0.46, 0.12)]);
            s.line(&[
                (-0.33, 0.10),
                (-0.22, -0.08),
                (-0.4, -0.25),
                (-0.12, -0.34),
                (0.16, -0.20),
                (0.22, 0.05),
            ]);
            s.bag(0.26, 0.20, 0.26);
            s.skull(0.32, -0.29, 0.16);
        }
        32 => {
            // Ancestor vigil: a mask above a bier, small ritual flames.
            s.skull(0., 0.25, 0.31);
            s.line(&[
                (-0.3, -0.03),
                (0.3, -0.03),
                (0.3, -0.18),
                (-0.3, -0.18),
                (-0.3, -0.03),
            ]);
            for sign in [-1., 1.] {
                s.line(&[(sign * 0.38, -0.3), (sign * 0.38, 0.06)]);
                s.circle(sign * 0.38, 0.11, 0.035);
            }
            s.line(&[(-0.24, -0.18), (-0.24, -0.36)]);
            s.line(&[(0.24, -0.18), (0.24, -0.36)]);
        }
        33 => {
            // Mushroom harvest and a shared gathering basket.
            for i in 0..3 {
                s.mushroom(-0.3 + i as f32 * 0.3, 0.15 + rng.range(-0.05, 0.06), 0.30);
            }
            s.bag(0., -0.28, 0.25);
            s.arrow(Vec2::new(-0.25, -0.04), Vec2::new(-0.09, -0.20));
        }
        34 => {
            // Raiders' boast: blades split the hostile surface sun.
            s.circle(0., 0.15, 0.15);
            s.line(&[(-0.08, 0.36), (0.03, 0.2), (-0.04, 0.11), (0.06, -0.04)]);
            for i in 0..8 {
                let a = i as f32 * TAU / 8.;
                s.line(&[
                    (a.cos() * 0.21, 0.15 + a.sin() * 0.21),
                    (a.cos() * 0.30, 0.15 + a.sin() * 0.30),
                ]);
            }
            s.blade(-0.25, -0.25, 0.32, -0.6);
            s.blade(0.25, -0.25, 0.32, 0.6);
        }
        _ => {
            // Trickster's false trail: forked footprints and a laughing mask.
            for i in 0..count {
                let x = -0.4 + i as f32 * 0.11;
                s.arc(x, -0.1 - (i % 2) as f32 * 0.10, 0.035, 0.017, 0., TAU);
            }
            s.arrow(Vec2::new(0.03, -0.06), Vec2::new(0.40, 0.02));
            s.arrow(Vec2::new(0.03, -0.06), Vec2::new(0.16, 0.22));
            s.head(-0.2, 0.26, 0.11);
        }
    }
    s.0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn goblin_cultural_symbols_are_distinct_and_fit_their_wall_space() {
        let mut previous = Vec::new();
        for motif in 5..COUNT {
            let lines = generate(motif, &mut Rng(721));
            assert!(!lines.is_empty());
            assert!(lines.iter().all(|line| line.len() >= 2));
            assert!(
                lines
                    .iter()
                    .flatten()
                    .all(|p| p.is_finite() && p.abs().max_element() <= 0.7)
            );
            assert!(
                !previous.contains(&lines),
                "duplicate cultural symbol {motif}"
            );
            assert_eq!(lines, generate(motif, &mut Rng(721)));
            previous.push(lines);
        }
    }
}

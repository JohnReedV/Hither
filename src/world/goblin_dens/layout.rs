//! Rejection-packed rooms and a visibility graph; no rings, slots or den variants.
use super::*;
pub(super) struct Plan {
    pub houses: Vec<House>,
    pub shrine: House,
    pub food_hall: House,
    pub landings: Vec<Landing>,
    pub spans: Vec<Span>,
}
fn rock(p: Vec3, radii: Vec3, seed: u64) -> f32 {
    (((p / radii).length() - 1.) * radii.min_element() + geology::relief(p, seed)).max(-17. - p.y)
}
fn outside(p: Vec3, h: &House, padding: f32) -> bool {
    let v = Quat::from_rotation_y(-h.yaw) * (p - h.at);
    v.x.abs() > h.width * 0.5 + padding || v.z.abs() > h.depth * 0.5 + padding
}
fn clear(a: Vec3, b: Vec3, rooms: &[House], radii: Vec3, seed: u64) -> bool {
    let count = (a.distance(b) / 0.35).ceil() as usize;
    (0..=count).all(|i| {
        let p = a.lerp(b, i as f32 / count.max(1) as f32);
        rooms.iter().all(|h| outside(p, h, 0.95))
            && rock(p, radii, seed) < -0.95
            && rock(p + Vec3::Y * 1.8, radii, seed) < -0.95
    })
}
fn crosses(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> bool {
    let ab = b.xz() - a.xz();
    let cd = d.xz() - c.xz();
    let denominator = ab.perp_dot(cd);
    if denominator.abs() < 0.001 {
        return false;
    }
    let offset = c.xz() - a.xz();
    let t = offset.perp_dot(cd) / denominator;
    let u = offset.perp_dot(ab) / denominator;
    (0.025..0.975).contains(&t) && (0.025..0.975).contains(&u)
}
fn root(parents: &mut [usize], i: usize) -> usize {
    if parents[i] != i {
        parents[i] = root(parents, parents[i]);
    }
    parents[i]
}
pub(super) fn plan(count: usize, radii: Vec3, seed: u64, rng: &mut Rng) -> Option<Plan> {
    for _ in 0..10 {
        let floor = rng.range(-7.0, -5.7);
        let mut rooms = Vec::new();
        // Large rooms are packed first, anywhere they fit; homes have no slots.
        for role in 0..count + 2 {
            let (width, depth, rise) = if role == 0 {
                (
                    rng.range(6.3, 7.8),
                    rng.range(5.5, 6.8),
                    rng.range(1.20, 1.48),
                )
            } else if role == 1 {
                (
                    rng.range(7.0, 8.3),
                    rng.range(5.6, 7.0),
                    rng.range(1.07, 1.30),
                )
            } else {
                (
                    rng.range(4.4, 5.9),
                    rng.range(3.9, 5.7),
                    rng.range(0.95, 1.12),
                )
            };
            let mut placed = None;
            for _ in 0..450 {
                let angle = rng.range(0., TAU);
                let radius = rng.unit().sqrt() * 0.69;
                let h = House {
                    at: Vec3::new(
                        angle.cos() * radii.x * radius,
                        floor + rng.range(-0.25, 0.25),
                        angle.sin() * radii.z * radius,
                    ),
                    yaw: rng.range(0., TAU),
                    width,
                    depth,
                    rise,
                    roof: Vec4::new(
                        rng.range(3.1, 4.0),
                        rng.range(1.90, 2.25),
                        rng.range(-width * 0.22, width * 0.22),
                        rng.range(-0.07, 0.07),
                    ),
                    edges: Vec4::new(
                        rng.range(0.6, 1.35),
                        rng.range(0.6, 1.35),
                        rng.range(0.6, 1.35),
                        rng.range(0.6, 1.35),
                    ),
                    window: Vec2::new(rng.range(0.87, 1.12), rng.range(1.80, 2.12)),
                };
                let q = Quat::from_rotation_y(h.yaw);
                let radius = Vec2::new(width, depth).length() * 0.5;
                if rooms.iter().any(|other: &House| {
                    other.at.xz().distance(h.at.xz())
                        < radius + Vec2::new(other.width, other.depth).length() * 0.5 + 0.65
                }) {
                    continue;
                }
                let mut fits = true;
                for x in [-width * 0.5 - 0.9, width * 0.5 + 0.9] {
                    for z in [-depth * 0.5 - 0.9, depth * 0.5 + 0.9] {
                        for y in [0., h.roof.x * rise] {
                            if rock(h.at + q * Vec3::new(x, y, z), radii, seed) > -0.55 {
                                fits = false;
                            }
                        }
                    }
                }
                let port = h.port();
                if !fits
                    || rock(port, radii, seed) > -1.7
                    || rock(port + Vec3::Y * 1.8, radii, seed) > -1.7
                {
                    continue;
                }
                if rooms
                    .iter()
                    .any(|other| !outside(port, other, 1.5) || !outside(other.port(), &h, 1.5))
                {
                    continue;
                }
                placed = Some(h);
                break;
            }
            let Some(h) = placed else {
                break;
            };
            rooms.push(h);
        }
        if rooms.len() != count + 2 {
            continue;
        }
        let mut landings: Vec<_> = rooms
            .iter()
            .map(|h| Landing {
                at: h.port(),
                size: Vec2::splat(2.6),
                yaw: h.yaw,
            })
            .collect();
        let extra = 2 + (rng.unit() * 4.).floor().min(3.) as usize;
        for _ in 0..extra {
            for _ in 0..150 {
                let p = Vec3::new(
                    rng.range(-radii.x * 0.65, radii.x * 0.65),
                    floor,
                    rng.range(-radii.z * 0.65, radii.z * 0.65),
                );
                if rooms.iter().any(|h| !outside(p, h, 2.2))
                    || landings.iter().any(|l| l.at.distance(p) < 3.8)
                {
                    continue;
                }
                if rock(p, radii, seed) > -2.2 || rock(p + Vec3::Y * 1.8, radii, seed) > -2.2 {
                    continue;
                }
                landings.push(Landing {
                    at: p,
                    size: Vec2::new(rng.range(2.3, 3.3), rng.range(2.3, 3.3)),
                    yaw: rng.range(0., TAU),
                });
                break;
            }
        }
        // Visibility waypoints around real room corners provide routes around
        // obstructed doors. Unneeded waypoints are pruned after graph creation.
        for h in &rooms {
            let q = Quat::from_rotation_y(h.yaw);
            for x in [-h.width / 2. - 1.45, h.width / 2. + 1.45] {
                for z in [-h.depth / 2. - 1.45, h.depth / 2. + 1.45] {
                    let p = h.at + q * Vec3::new(x, 0., z);
                    if rooms.iter().any(|r| !outside(p, r, 1.1))
                        || landings.iter().any(|l| l.at.distance(p) < 2.7)
                    {
                        continue;
                    }
                    if rock(p, radii, seed) > -1.85 || rock(p + Vec3::Y * 1.8, radii, seed) > -1.85
                    {
                        continue;
                    }
                    landings.push(Landing {
                        at: p,
                        size: Vec2::splat(2.5),
                        yaw: rng.range(0., TAU),
                    });
                }
            }
        }
        let mut edges = Vec::new();
        for i in 0..landings.len() {
            for j in i + 1..landings.len() {
                let a = landings[i].at;
                let b = landings[j].at;
                let ab = b.xz() - a.xz();
                let skirts_other_platforms = landings.iter().enumerate().all(|(k, l)| {
                    if k == i || k == j {
                        return true;
                    }
                    let t = ((l.at.xz() - a.xz()).dot(ab) / ab.length_squared()).clamp(0., 1.);
                    !(0.03..0.97).contains(&t)
                        || l.at.xz().distance(a.xz() + ab * t) > l.size.max_element() * 0.5 + 0.38
                });
                if skirts_other_platforms && clear(a, b, &rooms, radii, seed) {
                    edges.push((a.distance(b) + rng.range(0., 0.8), i, j));
                }
            }
        }
        edges.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut parents: Vec<_> = (0..landings.len()).collect();
        let mut links: Vec<(usize, usize)> = Vec::new();
        for &(_, i, j) in &edges {
            let a = root(&mut parents, i);
            let b = root(&mut parents, j);
            if a == b
                || links.iter().any(|&(u, v)| {
                    crosses(
                        landings[i].at,
                        landings[j].at,
                        landings[u].at,
                        landings[v].at,
                    )
                })
            {
                continue;
            }
            parents[a] = b;
            links.push((i, j));
        }
        let connected = root(&mut parents, 0);
        if (1..rooms.len()).any(|i| root(&mut parents, i) != connected) {
            continue;
        }
        links.retain(|&(i, _)| root(&mut parents, i) == connected);
        // Seeded extra loops, only when they do not cross another hanging route.
        for &(_, i, j) in &edges {
            if root(&mut parents, i) != connected
                || root(&mut parents, j) != connected
                || rng.unit() > 0.14
                || links.contains(&(i, j))
                || landings[i].at.distance(landings[j].at) > 13.
            {
                continue;
            }
            if links.iter().any(|&(u, v)| {
                crosses(
                    landings[i].at,
                    landings[j].at,
                    landings[u].at,
                    landings[v].at,
                )
            }) {
                continue;
            }
            links.push((i, j));
        }
        // Keep useful branching/bend platforms, removing random dead ends.
        loop {
            let mut degree = vec![0; landings.len()];
            for &(i, j) in &links {
                degree[i] += 1;
                degree[j] += 1;
            }
            let before = links.len();
            links.retain(|&(i, j)| {
                !(i >= rooms.len() && degree[i] == 1 || j >= rooms.len() && degree[j] == 1)
            });
            if links.len() == before {
                break;
            }
        }
        let mut used = vec![false; landings.len()];
        for &(i, j) in &links {
            used[i] = true;
            used[j] = true;
        }
        let mut spans: Vec<_> = links
            .into_iter()
            .map(|(i, j)| Span {
                a: landings[i].at,
                b: landings[j].at,
                width: rng.range(1.25, 1.65),
                sag: rng.range(0.12, 0.35),
            })
            .collect();
        for h in &rooms {
            spans.push(Span {
                a: h.porch(),
                b: h.port(),
                width: 1.55,
                sag: 0.,
            });
        }
        let landings = landings
            .into_iter()
            .enumerate()
            .filter_map(|(i, l)| used[i].then_some(l))
            .collect();
        let shrine = rooms.remove(0);
        let food_hall = rooms.remove(0);
        return Some(Plan {
            houses: rooms,
            shrine,
            food_hall,
            landings,
            spans,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn goblin_sites_generate_distinct_repeatable_connected_layouts() {
        let mut previous = Vec::new();
        for seed in 1..=4 {
            let count = 2 + (seed as usize % 4);
            let radii = Vec3::new(28., 22., 27.);
            let a = plan(count, radii, seed, &mut Rng(seed))
                .unwrap_or_else(|| panic!("packing failed for seed {seed}, {count} homes"));
            let b = plan(count, radii, seed, &mut Rng(seed)).unwrap();
            let fingerprint = |p: &Plan| {
                format!(
                    "{:?}{:?}{:?}{:?}{:?}",
                    p.houses, p.shrine, p.food_hall, p.landings, p.spans
                )
            };
            let signature = fingerprint(&a);
            assert_eq!(signature, fingerprint(&b));
            assert!(!previous.contains(&signature));
            previous.push(signature);
            assert_eq!(a.houses.len(), count);
            assert!(a.spans.len() >= a.landings.len() - 1 + count + 2);
            let rooms: Vec<_> = a
                .houses
                .iter()
                .chain([&a.shrine, &a.food_hall])
                .cloned()
                .collect();
            for span in a.spans.iter().take(a.spans.len() - rooms.len()) {
                assert!(clear(span.a, span.b, &rooms, radii, seed));
            }
        }
    }
}

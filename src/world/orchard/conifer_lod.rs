//! Silhouette-preserving branch rings and complete-shoot far needle levels.
use super::*;
pub(super) fn enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        !(std::env::var_os("HITHER_PROFILE_FOREST").is_some()
            && std::env::var_os("HITHER_PROFILE_NO_CONIFER_LOD").is_some())
    })
}
pub(super) fn extent(parts: [&Geometry; 3]) -> f32 {
    let mut low = Vec3::splat(f32::INFINITY);
    let mut high = Vec3::splat(f32::NEG_INFINITY);
    for p in parts.into_iter().flat_map(|g| &g.positions) {
        low = low.min(Vec3::from(*p));
        high = high.max(Vec3::from(*p));
    }
    // Use twice the actual half-diagonal: 96/24-pixel transitions under the
    // shared 192/48-pixel selector keep broadened needle bases subpixel.
    (high - low).length()
}
pub(super) fn branch(
    mesh: &mut Geometry,
    path: &[(Vec3, f32)],
    sides: usize,
    seed: f32,
    detail: usize,
) {
    if detail == 0 {
        return super::branch(mesh, path, sides, seed);
    }
    let mut source = Geometry::default();
    super::branch(&mut source, path, sides, seed);
    let ring_count = (source.positions.len() - 1) / (sides + 1);
    let rings: Vec<_> = (0..ring_count)
        .step_by(1 << detail)
        .chain(std::iter::once(ring_count - 1))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let count = (sides / (detail + 1)).max(3).min(sides);
    let columns: Vec<_> = (0..=count)
        .map(|i| (i * sides + count / 2) / count)
        .collect();
    let base = mesh.positions.len() as u32;
    for (row, &ring) in rings.iter().enumerate() {
        for (column, &side) in columns.iter().enumerate() {
            let index = ring * (sides + 1) + side;
            mesh.vertex(
                Vec3::from(source.positions[index]),
                Vec2::from(source.uvs[index]),
                Vec3::from_slice(&source.colors[index][..3]),
            );
            if row > 0 && column > 0 {
                let a = base + ((row - 1) * (count + 1) + column - 1) as u32;
                let b = a + (count + 1) as u32;
                mesh.triangle(a, a + 1, b);
                mesh.triangle(a + 1, b + 1, b);
            }
        }
    }
    let cap = mesh.vertex(path.last().unwrap().0, Vec2::ZERO, Vec3::splat(0.65));
    let last = base + ((rings.len() - 1) * (count + 1)) as u32;
    for side in 0..count as u32 {
        mesh.triangle(cap, last + side, last + side + 1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reduced_branches_retain_endpoints_and_finite_closed_tips() {
        let path = [
            (Vec3::ZERO, 0.3),
            (Vec3::new(0.3, 1., 0.2), 0.14),
            (Vec3::new(0.6, 2., 0.1), 0.01),
        ];
        let mut full = Geometry::default();
        branch(&mut full, &path, 18, 7., 0);
        for detail in [1, 2] {
            let mut low = Geometry::default();
            branch(&mut low, &path, 18, 7., detail);
            assert!(low.indices.len() < full.indices.len() / 2);
            assert_eq!(
                *low.positions.last().unwrap(),
                path.last().unwrap().0.to_array()
            );
            assert!(low.positions.iter().all(|p| Vec3::from(*p).is_finite()));
            assert!(
                low.indices
                    .iter()
                    .all(|i| (*i as usize) < low.positions.len())
            );
        }
    }
}

//! Seeded citrus stands with occasional apple patches and bounded streaming.
use super::*;
use crate::world::streaming::{Coordinator, Layer};
use std::collections::HashMap;
#[cfg(test)]
use std::collections::HashSet;

const SPACING: f32 = 3.0;
const CHUNK: i32 = 5;
const RADIUS: i32 = 3;
const VARIANTS: usize = 8;
const APPLE_VARIANTS: usize = 4;
const FRUIT_LAYOUTS: usize = 16;
const CHUNKS_PER_FRAME: usize = 4;

fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

use crate::world::biome::world_seed;

fn cell_hash(seed: u64, x: i32, z: i32) -> u64 {
    mix(seed ^ mix(x as i64 as u64) ^ mix((z as i64 as u64).wrapping_add(0x9e3779b97f4a7c15)))
}

// Sparse, jittered groves: species is selected by area, never by a per-tree roll.
// Centers stay inside their cells, so patches cannot form a continuous orchard.
fn apple_patch(seed: u64, point: Vec2) -> bool {
    const PATCH_CELL: f32 = 72.0;
    let cell = (point / PATCH_CELL).floor().as_ivec2();
    let hash = cell_hash(seed ^ 0x6170706c655f7061, cell.x, cell.y);
    if !hash.is_multiple_of(3) {
        return false;
    }
    let mut rng = Rng(hash);
    let center =
        (cell.as_vec2() + Vec2::new(rng.range(0.25, 0.75), rng.range(0.25, 0.75))) * PATCH_CELL;
    let radii = Vec2::new(rng.range(7.0, 11.0), rng.range(7.0, 11.0));
    ((point - center) / radii).length_squared() < 1.0
}

fn eligible(p: Vec2) -> bool {
    // Entire crowns must clear the castle, the courtyard apple and entrance.
    !(p.x.abs() <= 10.0 && p.y.abs() <= 10.0)
        && crate::world::biome::snow_amount(p) < 0.18
        && crate::world::biome::forest_amount(p) > 0.55
}

fn altitude_allowed(y: f32) -> bool {
    y.is_finite() && y <= 50.0
}

fn location(seed: u64, x: i32, z: i32) -> Option<Placement> {
    thread_local! {
        static CACHE: std::cell::RefCell<super::placement_cache::PlacementCache<Placement>> = Default::default();
    }
    CACHE.with_borrow_mut(|cache| {
        cache.get_or_insert(seed, IVec2::new(x, z), || location_uncached(seed, x, z))
    })
}

fn location_uncached(seed: u64, x: i32, z: i32) -> Option<Placement> {
    let site = super::forest::site(seed, IVec2::new(x, z), SPACING, 2.0)?;
    if !altitude_allowed(crate::world::terrain::height(site.position))
        || !eligible(site.position)
        || crate::world::orcs::entrance_overlap(
            Vec3::new(site.position.x, 0., site.position.y),
            2.0,
        )
    {
        return None;
    }
    let hash = site.hash;
    // Keep 80% of original sites, then 90% of those (72% overall), without
    // moving survivors. Independent hashes avoid biasing stand density and age.
    if mix(hash ^ 0x6369747275735f64).is_multiple_of(5)
        || mix(hash ^ 0x6369747275735f65).is_multiple_of(10)
    {
        return None;
    }
    let mut rng = Rng(mix(hash));
    let scale = super::forest::age_scale(hash) * 1.20 * 1.10;
    let ground = crate::world::terrain::vegetation_anchor(site.position, 0.45 * scale, 0.8)?;
    if !altitude_allowed(ground) {
        return None;
    }
    Some(Placement {
        position: Vec3::new(site.position.x, ground, site.position.y),
        yaw: rng.range(0.0, TAU),
        // Scale every part, including individual leaves, without adding geometry.
        scale,
        variant: if apple_patch(seed, site.position) {
            VARIANTS + (hash >> 32) as usize % APPLE_VARIANTS
        } else {
            (hash >> 32) as usize % VARIANTS
        },
    })
}

#[derive(Clone, Copy)]
struct Placement {
    position: Vec3,
    yaw: f32,
    scale: f32,
    variant: usize,
}
impl Placement {
    fn transform(self) -> Transform {
        Transform::from_translation(self.position)
            .with_rotation(Quat::from_rotation_y(self.yaw))
            .with_scale(Vec3::splat(self.scale))
    }
    fn local(self, p: Vec3) -> Vec3 {
        Quat::from_rotation_y(-self.yaw) * (p - self.position) / self.scale
    }
}

fn nearby(point: Vec3, radius: i32) -> impl Iterator<Item = Placement> {
    let x = (point.x / SPACING).floor() as i32;
    let z = (point.z / SPACING).floor() as i32;
    let radius = (radius as f32 / SPACING).ceil() as i32 + 1;
    let seed = world_seed();
    (-radius..=radius)
        .flat_map(move |dx| (-radius..=radius).filter_map(move |dz| location(seed, x + dx, z + dz)))
}

pub(crate) fn player_collides(point: Vec3, radius: f32) -> bool {
    nearby(point, 1)
        .any(|p| point.xz().distance_squared(p.position.xz()) < (radius + 0.30 * p.scale).powi(2))
}

pub(crate) fn camera_obstructed(point: Vec3, radius: f32) -> bool {
    nearby(point, 6).any(|p| {
        let local = p.local(point);
        let margin = radius / p.scale;
        (local.y < 1.6 && local.xz().length() < 0.30 + margin)
            || tree_variant(p.variant)
                .camera_bounds
                .iter()
                .any(|(c, r)| local.distance_squared(*c) < (r + margin).powi(2))
    })
}

fn variants() -> &'static Vec<Tree> {
    static TREES: OnceLock<Vec<Tree>> = OnceLock::new();
    TREES.get_or_init(|| {
        // A bounded world-specific morphology bank; parallel CPU generation
        // avoids rebuilding a full tree whenever the player crosses a cell.
        std::thread::scope(|scope| {
            let jobs: Vec<_> = (0..VARIANTS)
                .map(|i| {
                    scope.spawn(move || {
                        generate_orange_tree(mix(world_seed() ^ (i as u64 + 0x636974727573)))
                    })
                })
                .collect();
            jobs.into_iter()
                .map(|job| job.join().expect("citrus generation"))
                .collect()
        })
    })
}

fn apple_variants() -> &'static Vec<Tree> {
    static TREES: OnceLock<Vec<Tree>> = OnceLock::new();
    TREES.get_or_init(|| {
        (0..APPLE_VARIANTS)
            .map(|i| generate_tree(mix(world_seed() ^ (i as u64 + 0x6170706c65))))
            .collect()
    })
}

fn tree_variant(variant: usize) -> &'static Tree {
    if variant < VARIANTS {
        &variants()[variant]
    } else {
        &apple_variants()[variant - VARIANTS]
    }
}

struct Model {
    shadow: Handle<Mesh>,
    wood: Handle<Mesh>,
    distant_wood: Handle<Mesh>,
    leaves: Handle<Mesh>,
    distant_leaves: Handle<Mesh>,
    fruit: Vec<FruitAttachment>,
    fruit_meshes: Vec<[Handle<Mesh>; 2]>,
    details: Vec<Handle<Mesh>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FruitAttachment {
    center: Vec3,
    anchor: Vec3,
    radius: f32,
}

impl FruitAttachment {
    fn top(self) -> Vec3 {
        self.center + Vec3::Y * self.radius
    }

    fn stem_transform(self) -> Transform {
        let delta = self.anchor - self.top();
        Transform::from_translation(self.top().lerp(self.anchor, 0.5))
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, delta.normalize()))
            .with_scale(Vec3::new(0.0017, delta.length(), 0.0017))
    }
}

fn fruit_candidates(tree: &Tree, seed: u64) -> Vec<FruitAttachment> {
    fruit_candidates_with_radius(tree, seed, 0.040, 0.058)
}

fn fruit_candidates_with_radius(
    tree: &Tree,
    seed: u64,
    min: f32,
    max: f32,
) -> Vec<FruitAttachment> {
    let mut rng = Rng(seed);
    let mut fruit: Vec<FruitAttachment> = Vec::new();
    // All candidates are actual leafy terminal shoots. No ellipsoid-shell
    // test: that pushed large, identical fruit out onto the canopy outline.
    for _ in 0..6000 {
        if fruit.len() == 64 {
            break;
        }
        let [a, b] = tree.fruit_spurs
            [(rng.unit() * tree.fruit_spurs.len() as f32) as usize % tree.fruit_spurs.len()];
        let anchor = a.lerp(b, rng.range(0.12, 0.94));
        let radius = rng.range(min, max);
        let stalk = rng.range(0.018, 0.045);
        let center = anchor - Vec3::Y * (radius + stalk)
            + Vec3::new(rng.range(-0.008, 0.008), 0.0, rng.range(-0.008, 0.008));
        let candidate = FruitAttachment {
            center,
            anchor,
            radius,
        };
        if tree.rays.overlaps_sphere(center, radius + 0.003)
            || fruit
                .iter()
                .any(|f| f.center.distance(center) < f.radius + radius + 0.025)
        {
            continue;
        }
        // The last few millimeters intentionally enter the parent twig.
        // The exposed stalk must not cut through intervening leaf blades.
        let top = candidate.top();
        let delta = anchor - top;
        if tree
            .rays
            .hit(top, delta.normalize(), (delta.length() - 0.007).max(0.0))
            .is_some()
        {
            continue;
        }
        fruit.push(candidate);
    }
    fruit
}

fn fruit_indices(count: usize, seed: u64) -> Vec<usize> {
    let mut indices: Vec<_> = (0..count).collect();
    indices.sort_unstable_by_key(|i| mix(seed ^ mix(*i as u64)));
    let previous_count = (14 + mix(seed) as usize % 9).min(count);
    let previous_count = (previous_count * 33 + 50) / 100;
    // Halve the existing fruit count. Seeded rounding balances odd counts
    // without changing tree sites or moving the retained fruit.
    let round_up = (mix(seed ^ 0x68616c665f667275) & 1) as usize;
    let previous_count = previous_count / 2 + previous_count % 2 * round_up;
    // Retain 50% of the current oranges with an independent seeded rounding roll.
    let round_up = (mix(seed ^ 0x6f72616e67655f32) & 1) as usize;
    indices.truncate(previous_count / 2 + previous_count % 2 * round_up);
    indices
}
fn selected_fruit(count: usize, layout: usize, apple: bool) -> Vec<usize> {
    if !apple {
        return fruit_indices(count, layout as u64);
    }
    let mut indices: Vec<_> = (0..count).collect();
    indices.sort_unstable_by_key(|i| mix(layout as u64 ^ mix(*i as u64)));
    // Retain 60% of the previous five apples per tree.
    indices.truncate(3);
    indices
}

fn fruit_primitives() -> &'static [Geometry; 5] {
    static PRIMITIVES: OnceLock<[Geometry; 5]> = OnceLock::new();
    PRIMITIVES.get_or_init(|| {
        let mut apple_shape = Geometry::default();
        super::lod::append(
            &mut apple_shape,
            apple(),
            Transform::from_scale(Vec3::splat(1.0 / APPLE_RADIUS)),
        );
        [
            super::lod::primitive(Sphere::new(1.0).mesh().uv(20, 12)),
            super::lod::primitive(Sphere::new(1.0).mesh().uv(8, 6)),
            super::lod::primitive(Cylinder::new(1.0, 1.0).mesh().resolution(6).build()),
            super::lod::primitive(Sphere::new(1.0).mesh().uv(8, 4)),
            apple_shape,
        ]
    })
}

impl Streaming {
    // Keep shared meshes for untouched trees. Cache only encountered harvest masks;
    // cell identities survive chunk unloading, while render handles stay reusable.
    #[expect(
        clippy::too_many_arguments,
        reason = "Harvest mesh selection needs the model, cell, part, layout, and LOD independently."
    )]
    fn mesh(
        &mut self,
        model: &Model,
        variant: usize,
        cell: IVec2,
        part: usize,
        layout: usize,
        near: bool,
        meshes: &mut Assets<Mesh>,
    ) -> Option<Handle<Mesh>> {
        let mask = self.harvested.get(&cell).copied().unwrap_or(0);
        if part < 2 || mask == 0 {
            return Some(model.mesh(part, layout, near));
        }
        let apple = variant >= VARIANTS;
        let selected: Vec<_> = selected_fruit(model.fruit.len(), layout, apple)
            .into_iter()
            .enumerate()
            .filter(|(slot, _)| mask & (1 << slot) == 0)
            .map(|(_, index)| index)
            .collect();
        // Empty meshes trigger Bevy's slab allocator upload error. Represent a
        // fully harvested part by the absence of Mesh3d, never an empty asset.
        if selected.is_empty() {
            return None;
        }
        let key = (variant, layout, mask, part, near);
        Some(
            self.fruit_meshes
                .entry(key)
                .or_insert_with(|| {
                    let [sphere, distant, stem, calyx, apple_shape] = fruit_primitives();
                    let shape = if !near && part == 2 {
                        distant
                    } else if apple {
                        apple_shape
                    } else {
                        sphere
                    };
                    let (body, details) = fruit_geometry_selected(
                        &model.fruit,
                        &selected,
                        shape,
                        stem,
                        calyx,
                        if apple && (near || part == 3) {
                            0.77
                        } else {
                            1.0
                        },
                    );
                    meshes.add(if part == 2 {
                        body.mesh()
                    } else {
                        details.mesh()
                    })
                })
                .clone(),
        )
    }
}

pub(crate) fn target_fruit(
    origin: Vec3,
    direction: Vec3,
    trees: &Query<&WildTree>,
    assets: &WildAssets,
    state: &Streaming,
) -> Option<(IVec2, usize, Vec3, f32, bool)> {
    static ORANGE_RAYS: OnceLock<RayMesh> = OnceLock::new();
    let orange = ORANGE_RAYS.get_or_init(|| {
        RayMesh::new(super::lod::primitive(Sphere::new(1.0).mesh().uv(20, 12)).triangles())
    });
    trees
        .iter()
        .filter_map(|tree| {
            let p = tree.placement;
            // A conservative broad phase avoids triangle queries for distant crowns.
            if p.position.distance(origin) > 70.0 {
                return None;
            }
            let model = &assets.models[p.variant];
            let apple = p.variant >= VARIANTS;
            let mask = state.harvested.get(&tree.cell).copied().unwrap_or(0);
            selected_fruit(model.fruit.len(), tree.layout, apple)
                .into_iter()
                .enumerate()
                .filter(|(slot, _)| mask & (1 << slot) == 0)
                .filter_map(|(slot, index)| {
                    let f = model.fruit[index];
                    let scale = f.radius / if apple { APPLE_RADIUS } else { 1.0 };
                    let local_origin = (p.local(origin) - f.center) / scale;
                    let local_direction = Quat::from_rotation_y(-p.yaw) * direction;
                    let rays = if apple { apple_rays() } else { orange };
                    let travel =
                        rays.hit(local_origin, local_direction, 70.0 / (p.scale * scale))?
                            * p.scale
                            * scale;
                    Some((
                        tree.cell,
                        slot,
                        p.transform().transform_point(f.center),
                        travel,
                        apple,
                    ))
                })
                .min_by(|a, b| a.3.total_cmp(&b.3))
        })
        .min_by(|a, b| a.3.total_cmp(&b.3))
}

pub(crate) fn occludes(
    origin: Vec3,
    direction: Vec3,
    limit: f32,
    trees: &Query<&WildTree>,
) -> bool {
    trees.iter().any(|tree| {
        let p = tree.placement;
        if p.position.distance(origin) > limit + 12.0 {
            return false;
        }
        tree_variant(p.variant)
            .rays
            .hit(
                p.local(origin),
                Quat::from_rotation_y(-p.yaw) * direction,
                limit / p.scale,
            )
            .is_some()
    })
}

pub(crate) fn harvest(state: &mut Streaming, cell: IVec2, slot: usize, apple: bool) {
    *state.harvested.entry(cell).or_default() |= 1 << slot;
    state.harvest_dirty = true;
    if !apple {
        state.oranges_harvested += 1;
    }
}

#[derive(Resource)]
pub(crate) struct WildAssets {
    models: Vec<Model>,
    wood: Handle<StandardMaterial>,
    leaves: Handle<StandardMaterial>,
    orange: Handle<StandardMaterial>,
    calyx: Handle<StandardMaterial>,
    apple_materials: [Handle<StandardMaterial>; 3],
}

#[derive(Resource, Default)]
pub(crate) struct Streaming {
    chunks: HashMap<IVec2, Entity>,
    center: Option<IVec2>,
    radius: i32,
    discovery: crate::world::streaming::Discovery<Vec<(IVec2, Placement)>>,
    lod_position: Option<Vec3>,
    lod_chunks: HashMap<IVec2, LodChunk>,
    harvested: HashMap<IVec2, u8>,
    harvest_dirty: bool,
    fruit_meshes: HashMap<(usize, usize, u8, usize, bool), Handle<Mesh>>,
    pub(crate) oranges_harvested: u32,
}

#[derive(Default)]
struct LodChunk {
    trees: Vec<TreeLod>,
    max_scale: f32,
}
struct TreeLod {
    position: Vec3,
    scale: f32,
    near: bool,
    parts: Vec<Entity>,
}

#[derive(Component)]
pub(crate) struct WildTree {
    cell: IVec2,
    placement: Placement,
    layout: usize,
}

#[derive(Component)]
pub(super) struct WildFoliage {
    #[cfg(test)]
    position: Vec3,
    variant: usize,
    cell: IVec2,
    layout: usize,
    part: usize,
    near: bool,
}

#[derive(Component)]
#[cfg(test)]
struct OrangeFruit {
    variant: usize,
    layout: usize,
}

fn fruit_geometry(
    fruit: &[FruitAttachment],
    layout: usize,
    sphere: &Geometry,
    stem: &Geometry,
    calyx: &Geometry,
) -> (Geometry, Geometry) {
    fruit_geometry_selected(
        fruit,
        &fruit_indices(fruit.len(), layout as u64),
        sphere,
        stem,
        calyx,
        1.0,
    )
}

fn fruit_geometry_selected(
    fruit: &[FruitAttachment],
    indices: &[usize],
    sphere: &Geometry,
    stem: &Geometry,
    calyx: &Geometry,
    top_fraction: f32,
) -> (Geometry, Geometry) {
    let mut body = Geometry::default();
    let mut details = Geometry::default();
    for &index in indices {
        let fruit = fruit[index];
        super::lod::append(
            &mut body,
            sphere,
            Transform::from_translation(fruit.center).with_scale(Vec3::splat(fruit.radius)),
        );
        // The sculpted apple has a recessed top; attach its stalk at the skin.
        let fruit = FruitAttachment {
            radius: fruit.radius * top_fraction,
            ..fruit
        };
        super::lod::append(&mut details, stem, fruit.stem_transform());
        super::lod::append(
            &mut details,
            calyx,
            Transform::from_translation(fruit.top() - Vec3::Y * 0.002).with_scale(Vec3::new(
                fruit.radius * 0.20,
                0.004,
                fruit.radius * 0.20,
            )),
        );
    }
    (body, details)
}

impl Model {
    fn mesh(&self, part: usize, layout: usize, near: bool) -> Handle<Mesh> {
        match part {
            0 => {
                if near {
                    self.wood.clone()
                } else {
                    self.distant_wood.clone()
                }
            }
            1 => {
                if near {
                    self.leaves.clone()
                } else {
                    self.distant_leaves.clone()
                }
            }
            2 => self.fruit_meshes[layout][usize::from(!near)].clone(),
            _ => self.details[layout].clone(),
        }
    }
}

// Keep complete individual blades, not fragments of each blade.
fn sparse_leaves(source: &Geometry) -> Geometry {
    let mut g = Geometry::default();
    const VERTICES: usize = orange_tree::LEAF_VERTICES;
    for i in 0..source.positions.len() / VERTICES {
        let start = i * VERTICES;
        let base = g.positions.len() as u32;
        // Preserve the raised midrib, not a flat quad. The center vertex keeps
        // the two leaf halves' normal/tangent interpolation and curved lighting.
        for offset in [1, 6, 7, 8, 13] {
            let n = start + offset;
            g.positions.push(source.positions[n]);
            g.normals.push(source.normals[n]);
            g.uvs.push(source.uvs[n]);
            g.colors.push(source.colors[n]);
        }
        g.indices.extend([
            base,
            base + 1,
            base + 2,
            base,
            base + 2,
            base + 3,
            base + 1,
            base + 4,
            base + 2,
            base + 2,
            base + 4,
            base + 3,
        ]);
    }
    g
}

pub(super) fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let start = std::time::Instant::now();
    let trees = variants();
    info!("Orange geometry ready in {:?}", start.elapsed());
    let mut models = Vec::new();
    let [sphere, distant_sphere, stem, calyx_mesh, apple_shape] = fruit_primitives();
    for (i, tree) in trees.iter().chain(apple_variants()).enumerate() {
        let is_apple = i >= VARIANTS;
        let fruit = if is_apple {
            fruit_candidates_with_radius(tree, i as u64 + 7723, 0.085, 0.11)
        } else {
            fruit_candidates(tree, i as u64 + 7723)
        };
        let mut fruit_meshes = Vec::new();
        let mut details = Vec::new();
        for layout in 0..FRUIT_LAYOUTS {
            let (body, detail, far_body) = if is_apple {
                let indices = selected_fruit(fruit.len(), layout, true);
                let (body, detail) =
                    fruit_geometry_selected(&fruit, &indices, apple_shape, stem, calyx_mesh, 0.77);
                let (far, _) = fruit_geometry_selected(
                    &fruit,
                    &indices,
                    distant_sphere,
                    stem,
                    calyx_mesh,
                    1.0,
                );
                (body, detail, far)
            } else {
                let (body, detail) = fruit_geometry(&fruit, layout, sphere, stem, calyx_mesh);
                let (far, _) = fruit_geometry(&fruit, layout, distant_sphere, stem, calyx_mesh);
                (body, detail, far)
            };
            fruit_meshes.push([meshes.add(body.mesh()), meshes.add(far_body.mesh())]);
            details.push(meshes.add(detail.mesh()));
        }
        let distant_wood = tree.distant_wood.as_ref().unwrap_or(&tree.wood);
        let distant_leaves = if is_apple {
            simple_apple_leaves(&tree.leaves)
        } else {
            sparse_leaves(&tree.leaves)
        };
        let mut shadow = distant_wood.clone();
        let shadow_leaves = if is_apple {
            distant_leaves.clone()
        } else {
            super::lod::shadow_leaf_clusters(&distant_leaves)
        };
        super::lod::append(&mut shadow, &shadow_leaves, Transform::default());
        models.push(Model {
            shadow: meshes.add(shadow.mesh()),
            wood: meshes.add(tree.wood.mesh()),
            distant_wood: meshes.add(distant_wood.mesh()),
            leaves: meshes.add(tree.leaves.mesh()),
            distant_leaves: meshes.add(distant_leaves.mesh()),
            fruit,
            fruit_meshes,
            details,
        });
    }
    let orange = materials.add(material(SurfaceKind::Orange, &mut images));
    let wood = materials.add(material(SurfaceKind::CitrusBark, &mut images));
    // One identical PBR material at every distance. Never trade lighting,
    // roughness or normal maps for speed when a tree crosses an LOD boundary.
    let leaves = materials.add(material(SurfaceKind::CitrusLeaf, &mut images));
    let calyx = materials.add(StandardMaterial {
        base_color: Color::srgb(0.17, 0.27, 0.055),
        perceptual_roughness: 0.72,
        ..default()
    });
    let apple_materials = [SurfaceKind::Bark, SurfaceKind::Leaf, SurfaceKind::Apple]
        .map(|kind| materials.add(material(kind, &mut images)));
    commands.insert_resource(WildAssets {
        models,
        apple_materials,
        wood,
        leaves,
        orange,
        calyx,
    });
    commands.insert_resource(Streaming::default());
    info!(
        "Orange world seed: {} (temperate forest understory)",
        world_seed()
    );
    info!("Orange setup ready in {:?}", start.elapsed());
}

fn sync_part_mesh(
    commands: &mut Commands,
    entity: Entity,
    current: Option<Mut<Mesh3d>>,
    wanted: Option<Handle<Mesh>>,
) {
    match (current, wanted) {
        (Some(mut current), Some(wanted)) => {
            if current.0 != wanted {
                current.0 = wanted;
            }
        }
        (None, Some(wanted)) => {
            commands.entity(entity).insert(Mesh3d(wanted));
        }
        (Some(_), None) => {
            commands.entity(entity).remove::<Mesh3d>();
        }
        (None, None) => {}
    }
}

#[allow(clippy::too_many_arguments)] // Bevy system parameters.
pub(super) fn stream(
    mut coordinator: ResMut<Coordinator>,
    detail: Option<Res<crate::rendering::view_distance::Detail>>,
    range: Option<Res<crate::rendering::view_distance::Range>>,
    mut commands: Commands,
    rig: Res<crate::player::camera::CameraRig>,
    view: Option<Res<crate::player::camera::CameraView>>,
    assets: Res<WildAssets>,
    mut state: ResMut<Streaming>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut foliage: Query<(
        Entity,
        &mut WildFoliage,
        Option<&mut Mesh3d>,
        &mut Visibility,
    )>,
) {
    let camera_position = view.as_ref().map_or(rig.position, |v| v.position);
    let radius = range.as_ref().map_or(RADIUS, |r| {
        ((r.load + 8.0) / (CHUNK as f32 * SPACING)).ceil() as i32 + 1
    });
    let center = IVec2::new(
        (rig.position.x / (CHUNK as f32 * SPACING)).floor() as i32,
        (rig.position.z / (CHUNK as f32 * SPACING)).floor() as i32,
    );
    let detail_changed = detail.as_ref().is_some_and(|d| d.is_changed());
    if detail_changed
        || state
            .lod_position
            .is_none_or(|p| p.distance_squared(camera_position) > 0.25)
    {
        let previous = state
            .lod_position
            .replace(camera_position)
            .unwrap_or(camera_position);
        // Only chunks intersecting the old/new detail spheres can contain a
        // transition. The old sphere also covers trees left behind by teleports.
        let mut chunks = std::mem::take(&mut state.lod_chunks);
        for (cell, chunk) in &mut chunks {
            let radius = detail
                .as_deref()
                .map_or(11.0, |d| d.0 + 2.0 * chunk.max_scale + 1.0);
            let low = cell.as_vec2() * (CHUNK as f32 * SPACING) - Vec2::splat(SPACING);
            let high = low + Vec2::splat(CHUNK as f32 * SPACING + 2.0 * SPACING);
            let intersects =
                |p: Vec3| p.xz().distance_squared(p.xz().clamp(low, high)) <= radius * radius;
            if !detail_changed && !intersects(previous) && !intersects(camera_position) {
                continue;
            }
            for tree in &mut chunk.trees {
                let near = super::lod::near_detail(
                    tree.near,
                    tree.position.distance(camera_position),
                    tree.scale,
                    detail.as_deref(),
                );
                if near == tree.near {
                    continue;
                }
                tree.near = near;
                for &entity in &tree.parts {
                    let Ok((_, mut part, mesh, mut visibility)) = foliage.get_mut(entity) else {
                        continue;
                    };
                    part.near = near;
                    let handle = state.mesh(
                        &assets.models[part.variant],
                        part.variant,
                        part.cell,
                        part.part,
                        part.layout,
                        near,
                        &mut meshes,
                    );
                    sync_part_mesh(&mut commands, entity, mesh, handle);
                    if part.part == 3 {
                        *visibility = if near {
                            Visibility::Inherited
                        } else {
                            Visibility::Hidden
                        };
                    }
                }
            }
        }
        state.lod_chunks = chunks;
    }
    if state.harvest_dirty {
        for (entity, part, mesh, _) in &mut foliage {
            if part.part >= 2 && state.harvested.contains_key(&part.cell) {
                let handle = state.mesh(
                    &assets.models[part.variant],
                    part.variant,
                    part.cell,
                    part.part,
                    part.layout,
                    part.near,
                    &mut meshes,
                );
                sync_part_mesh(&mut commands, entity, mesh, handle);
            }
        }
        state.harvest_dirty = false;
    }
    if state.center != Some(center) || state.radius != radius {
        state.center = Some(center);
        state.radius = radius;
        state.chunks.retain(|cell, entity| {
            if (*cell - center).abs().max_element() <= radius {
                true
            } else {
                crate::world::streaming::retire(&mut commands, *entity);
                false
            }
        });
        state
            .lod_chunks
            .retain(|cell, _| (*cell - center).abs().max_element() <= radius);
        state.discovery.update(center, radius);
    }
    state
        .discovery
        .poll(&mut coordinator, Layer::Citrus, chunk_placements);
    for _ in 0..CHUNKS_PER_FRAME {
        let Some((_, sites)) = state.discovery.front() else {
            break;
        };
        let entities = 1 + sites.len() * 6;
        let Some(_installation) = coordinator.install_entities(Layer::Citrus, 4, 0, entities)
        else {
            break;
        };
        let (cell, sites) = state.discovery.pop().unwrap();
        // Only entering chunks need entities. Refresh LOD handles above, rather
        // than recreating thousands of fruit/stem entities at each boundary.
        if state.chunks.contains_key(&cell) {
            continue;
        }
        let root = commands
            .spawn((
                Name::new(format!("Orange grove chunk {},{}", cell.x, cell.y)),
                Transform::default(),
                Visibility::Inherited,
            ))
            .id();
        let mut lod_chunk = LodChunk::default();
        for (site, p) in sites {
            let x = site.x;
            let z = site.y;
            let model = &assets.models[p.variant];
            let is_apple = p.variant >= VARIANTS;
            let tree_materials = if is_apple {
                [
                    &assets.apple_materials[0],
                    &assets.apple_materials[1],
                    &assets.apple_materials[2],
                    &assets.calyx,
                ]
            } else {
                [&assets.wood, &assets.leaves, &assets.orange, &assets.calyx]
            };
            let position = p.position + Vec3::Y * 2.0 * p.scale;
            let near = super::lod::near_detail(
                false,
                position.distance(camera_position),
                p.scale,
                detail.as_deref(),
            );
            let layout = cell_hash(world_seed(), x, z) as usize % FRUIT_LAYOUTS;
            let tree = commands
                .spawn((
                    WildTree {
                        cell: IVec2::new(x, z),
                        placement: p,
                        layout,
                    },
                    Name::new(if is_apple {
                        "Wild apple tree"
                    } else {
                        "Wild orange tree"
                    }),
                    p.transform(),
                    Visibility::Inherited,
                ))
                .id();
            commands.entity(root).add_child(tree);
            let mut part_entities = Vec::new();
            commands.entity(tree).with_children(|parent| {
                parent.spawn((
                    Name::new("Citrus shadow canopy"),
                    Mesh3d(model.shadow.clone()),
                    MeshMaterial3d(tree_materials[1].clone()),
                    super::lod::shadow_layers(),
                    crate::rendering::plant_lod::ShadowProxy,
                    Transform::default(),
                ));
                for (part, material) in tree_materials.into_iter().enumerate() {
                    let mut entity = parent.spawn((
                        crate::rendering::plant_lod::Canopy(part == 1),
                        WildFoliage {
                            #[cfg(test)]
                            position,
                            variant: p.variant,
                            cell: IVec2::new(x, z),
                            layout,
                            part,
                            near,
                        },
                        bevy::light::NotShadowCaster,
                        MeshMaterial3d(material.clone()),
                        Transform::default(),
                        if part == 3 && !near {
                            Visibility::Hidden
                        } else {
                            Visibility::Inherited
                        },
                    ));
                    if let Some(mesh) = state.mesh(
                        model,
                        p.variant,
                        IVec2::new(x, z),
                        part,
                        layout,
                        near,
                        &mut meshes,
                    ) {
                        entity.insert(Mesh3d(mesh));
                    }
                    part_entities.push(entity.id());
                    #[cfg(test)]
                    if part == 2 && !is_apple {
                        let mut entity = entity;
                        entity.insert(OrangeFruit {
                            variant: p.variant,
                            layout,
                        });
                    }
                    #[cfg(not(test))]
                    let _ = entity;
                }
            });
            lod_chunk.max_scale = lod_chunk.max_scale.max(p.scale);
            lod_chunk.trees.push(TreeLod {
                position,
                scale: p.scale,
                near,
                parts: part_entities,
            });
        }
        state.lod_chunks.insert(cell, lod_chunk);
        state.chunks.insert(cell, root);
    }
}

fn chunk_placements(cell: IVec2) -> Vec<(IVec2, Placement)> {
    (cell.x * CHUNK..(cell.x + 1) * CHUNK)
        .flat_map(|x| {
            (cell.y * CHUNK..(cell.y + 1) * CHUNK)
                .filter_map(move |z| location(world_seed(), x, z).map(|p| (IVec2::new(x, z), p)))
        })
        .collect()
}

/// Existing accepted sites are the population used for fallen-log density.
pub(super) fn log_source(cell: IVec2) -> Option<(Vec3, super::logs::Species)> {
    let p = location(world_seed(), cell.x, cell.y)?;
    Some((
        p.position,
        if p.variant < VARIANTS {
            super::logs::Species::Orange
        } else {
            super::logs::Species::Apple
        },
    ))
}

/// Fallen timber stays inside its parent species' patch even at its tips.
pub(super) fn log_habitat(point: Vec2, species: super::logs::Species) -> bool {
    eligible(point) && apple_patch(world_seed(), point) == (species == super::logs::Species::Apple)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn harvest_targets_scaled_rotated_fruit_and_preserves_shared_meshes() {
        use bevy::ecs::system::SystemState;
        let sphere = super::super::lod::primitive(Sphere::new(1.0).mesh().uv(20, 12));
        let stem =
            super::super::lod::primitive(Cylinder::new(1.0, 1.0).mesh().resolution(6).build());
        let calyx = super::super::lod::primitive(Sphere::new(1.0).mesh().uv(8, 4));
        let mut apple_shape = Geometry::default();
        super::super::lod::append(
            &mut apple_shape,
            apple(),
            Transform::from_scale(Vec3::splat(1.0 / APPLE_RADIUS)),
        );
        let mut meshes = Assets::<Mesh>::default();
        // Supply enough candidates to retain fruit after the spawn reductions.
        let fruit: Vec<_> = (0..14)
            .map(|i| FruitAttachment {
                center: Vec3::new(3.0, 1.5, i as f32 * 0.5),
                anchor: Vec3::new(3.0, 1.7, i as f32 * 0.5),
                radius: 0.1,
            })
            .collect();
        let models = (0..=VARIANTS)
            .map(|variant| {
                let indices = selected_fruit(fruit.len(), 0, variant >= VARIANTS);
                let (body, detail) = fruit_geometry_selected(
                    &fruit,
                    &indices,
                    if variant >= VARIANTS {
                        &apple_shape
                    } else {
                        &sphere
                    },
                    &stem,
                    &calyx,
                    if variant >= VARIANTS { 0.77 } else { 1.0 },
                );
                let near = meshes.add(body.mesh());
                let (far_body, _) = fruit_geometry_selected(
                    &fruit,
                    &indices,
                    &fruit_primitives()[1],
                    &stem,
                    &calyx,
                    1.0,
                );
                let far = meshes.add(far_body.mesh());
                Model {
                    shadow: default(),
                    wood: default(),
                    distant_wood: default(),
                    leaves: default(),
                    distant_leaves: default(),
                    fruit: fruit.clone(),
                    fruit_meshes: vec![[near, far]],
                    details: vec![meshes.add(detail.mesh())],
                }
            })
            .collect();
        let assets = WildAssets {
            models,
            wood: default(),
            leaves: default(),
            orange: default(),
            calyx: default(),
            apple_materials: default(),
        };
        for variant in [0, VARIANTS] {
            let mut world = World::new();
            let cell = IVec2::new(-12, 9);
            let placement = Placement {
                position: Vec3::new(-36.0, 8.0, 27.0),
                yaw: 1.2,
                scale: 1.8,
                variant,
            };
            let entity = world
                .spawn(WildTree {
                    cell,
                    placement,
                    layout: 0,
                })
                .id();
            let mut query = SystemState::<Query<&WildTree>>::new(&mut world);
            let mut state = Streaming::default();
            let selected = selected_fruit(fruit.len(), 0, variant >= VARIANTS);
            let center = placement
                .transform()
                .transform_point(fruit[selected[0]].center);
            let direction = placement.transform().rotation * -Vec3::X;
            let origin = center - direction * 0.5;
            let hit = target_fruit(
                origin,
                direction,
                &query.get(&world).unwrap(),
                &assets,
                &state,
            )
            .unwrap();
            assert_eq!((hit.0, hit.1, hit.4), (cell, 0, variant >= VARIANTS));
            assert!((hit.2 - center).length() < 1e-5);
            assert!((0.30..0.34).contains(&hit.3));
            harvest(&mut state, cell, 0, hit.4);
            assert!(
                target_fruit(
                    origin,
                    direction,
                    &query.get(&world).unwrap(),
                    &assets,
                    &state
                )
                .is_none()
            );
            let model = &assets.models[variant];
            for (part, near) in [(2, true), (2, false), (3, true)] {
                let original = model.mesh(part, 0, near);
                let original_count = meshes.get(&original).unwrap().indices().unwrap().len();
                let picked = state.mesh(model, variant, cell, part, 0, near, &mut meshes);
                if selected.len() == 1 {
                    assert!(picked.is_none());
                    assert_eq!(
                        meshes.get(&original).unwrap().indices().unwrap().len(),
                        original_count
                    );
                    continue;
                }
                let picked = picked.unwrap();
                assert_ne!(picked, original);
                let [sphere, distant, stem, calyx, apple_shape] = fruit_primitives();
                let (expected_body, expected_detail) = fruit_geometry_selected(
                    &fruit,
                    &selected[1..],
                    if part == 2 && !near {
                        distant
                    } else if variant >= VARIANTS {
                        apple_shape
                    } else {
                        sphere
                    },
                    stem,
                    calyx,
                    if variant >= VARIANTS && (near || part == 3) {
                        0.77
                    } else {
                        1.0
                    },
                );
                let expected = if part == 2 {
                    expected_body.mesh()
                } else {
                    expected_detail.mesh()
                };
                assert_eq!(
                    meshes.get(&picked).unwrap().indices().unwrap().len(),
                    expected.indices().unwrap().len()
                );
                assert!(meshes.get(&picked).unwrap().indices().unwrap().len() < original_count);
                assert_eq!(
                    meshes.get(&original).unwrap().indices().unwrap().len(),
                    original_count
                );
                assert_eq!(
                    state.mesh(model, variant, cell, part, 0, near, &mut meshes),
                    Some(picked)
                );
                assert_eq!(
                    state.mesh(model, variant, cell + IVec2::X, part, 0, near, &mut meshes),
                    Some(original)
                );
            }
            world.despawn(entity);
            world.spawn(WildTree {
                cell,
                placement,
                layout: 0,
            });
            assert!(
                target_fruit(
                    origin,
                    direction,
                    &query.get(&world).unwrap(),
                    &assets,
                    &state
                )
                .is_none()
            );
            for slot in 1..selected.len() {
                harvest(&mut state, cell, slot, variant >= VARIANTS);
            }
            let mesh_count = meshes.len();
            let cached_count = state.fruit_meshes.len();
            for part in [2, 3] {
                for near in [true, false] {
                    assert!(
                        state
                            .mesh(model, variant, cell, part, 0, near, &mut meshes)
                            .is_none()
                    );
                }
            }
            assert_eq!(meshes.len(), mesh_count);
            assert_eq!(state.fruit_meshes.len(), cached_count);
        }
        // Exercise the actual E-key system for both species, including pause gating.
        let mut app = App::new();
        app.add_plugins(crate::world::streaming::StreamingPlugin);
        app.insert_resource(assets)
            .init_resource::<Streaming>()
            .init_resource::<crate::app::GameState>()
            .init_resource::<crate::ui::chat::ChatState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(OrchardState {
                apples: Vec::new(),
                harvested: 0,
                growth_timer: Timer::from_seconds(1.0, TimerMode::Repeating),
                random_state: 1,
            })
            .add_systems(Update, super::super::fruit::pick_fruit);
        for variant in [0, VARIANTS] {
            let cell = IVec2::new(variant as i32, 20);
            let placement = Placement {
                position: Vec3::new(variant as f32 * 10.0, 100.0, 60.0),
                yaw: 1.2,
                scale: 1.8,
                variant,
            };
            app.world_mut().spawn(WildTree {
                cell,
                placement,
                layout: 0,
            });
            let selected = selected_fruit(fruit.len(), 0, variant >= VARIANTS);
            let center = placement
                .transform()
                .transform_point(fruit[selected[0]].center);
            let direction = placement.transform().rotation * -Vec3::X;
            let origin = center - direction * 0.5;
            app.insert_resource(crate::player::camera::CameraRig {
                position: origin,
                ..default()
            });
            app.insert_resource(crate::player::camera::CameraView {
                position: origin,
                forward: direction,
                right: Vec3::Z,
                up: Vec3::Y,
            });
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyE);
            app.world_mut()
                .resource_mut::<crate::app::GameState>()
                .paused = true;
            app.update();
            assert!(
                !app.world()
                    .resource::<Streaming>()
                    .harvested
                    .contains_key(&cell)
            );
            app.world_mut()
                .resource_mut::<crate::app::GameState>()
                .paused = false;
            app.update();
            assert_eq!(
                app.world().resource::<Streaming>().harvested.get(&cell),
                Some(&1)
            );
            app.update(); // An already-picked slot cannot be harvested again.
            assert_eq!(app.world().resource::<Streaming>().oranges_harvested, 1);
            assert_eq!(
                app.world().resource::<OrchardState>().harvested,
                u32::from(variant >= VARIANTS)
            );
        }
    }

    #[test]
    fn apple_groves_are_sparse_coherent_and_contain_neighboring_trees() {
        let mut apples = 0;
        let mut oranges = 0;
        let mut grouped = 0;
        for x in -250..250 {
            for z in -250..250 {
                let Some(p) = location(world_seed(), x, z) else {
                    continue;
                };
                assert_eq!(
                    p.variant >= VARIANTS,
                    apple_patch(world_seed(), p.position.xz())
                );
                if p.variant < VARIANTS {
                    oranges += 1;
                    continue;
                }
                apples += 1;
                assert_eq!(p.variant, location(world_seed(), x, z).unwrap().variant);
                grouped += usize::from((-2..=2).any(|dx| {
                    (-2..=2).any(|dz| {
                        (dx != 0 || dz != 0)
                            && location(world_seed(), x + dx, z + dz)
                                .is_some_and(|q| q.variant >= VARIANTS)
                    })
                }));
                assert!(camera_obstructed(p.position + Vec3::Y, 0.1));
                assert!(player_collides(p.position + Vec3::Y, 0.28));
            }
        }
        assert!(apples > 20, "apple patches must occur in temperate forests");
        assert!(
            apples < oranges / 10,
            "apple patches must remain occasional"
        );
        assert!(grouped * 10 > apples * 9, "apples must grow together");
    }

    #[test]
    fn citrus_never_spawns_in_plains_or_snow_and_clears_castle() {
        let mut count = 0;
        for x in -250..250 {
            for z in -250..250 {
                if let Some(p) = location(world_seed(), x, z) {
                    count += 1;
                    assert!(crate::world::biome::forest_amount(p.position.xz()) > 0.55);
                    assert!(crate::world::biome::snow_amount(p.position.xz()) < 0.18);
                    assert!(p.position.x.abs() > 10.0 || p.position.z.abs() > 10.0);
                }
            }
        }
        assert!(count > 100);
    }
    #[test]
    fn oranges_attach_to_twigs_without_intersections_and_vary_by_tree() {
        for (variant, tree) in variants().iter().enumerate() {
            let fruit = fruit_candidates(tree, variant as u64 + 7723);
            assert_eq!(fruit.len(), 64);
            assert_eq!(fruit, fruit_candidates(tree, variant as u64 + 7723));
            for (i, f) in fruit.iter().enumerate() {
                assert!((0.040..=0.058).contains(&f.radius));
                assert!((0.018 - 1e-6..=0.045 + 1e-6).contains(&(f.anchor.y - f.top().y)));
                assert!(!tree.rays.overlaps_sphere(f.center, f.radius + 0.003));
                assert!(
                    fruit[..i]
                        .iter()
                        .all(|g| f.center.distance(g.center) >= f.radius + g.radius + 0.025)
                );
                assert!(tree.fruit_spurs.iter().any(|[a, b]| {
                    let d = *b - *a;
                    let t = ((f.anchor - *a).dot(d) / d.length_squared()).clamp(0.0, 1.0);
                    f.anchor.distance(a.lerp(*b, t)) < 1e-5
                }));
                let stem = f.stem_transform();
                assert!(stem.transform_point(Vec3::Y * 0.5).distance(f.anchor) < 1e-5);
                assert!(stem.transform_point(-Vec3::Y * 0.5).distance(f.top()) < 1e-5);
            }
            let layouts: Vec<_> = (0..20)
                .map(|seed| fruit_indices(fruit.len(), seed))
                .collect();
            assert!(layouts.iter().all(|v| (1..=2).contains(&v.len())));
            for seed in 0..20 {
                let previous_count = 14 + mix(seed) as usize % 9;
                assert_eq!(fruit_indices(64, seed).len(), {
                    let old = (previous_count * 33 + 50) / 100;
                    let old = old / 2 + old % 2 * (mix(seed ^ 0x68616c665f667275) & 1) as usize;
                    old / 2 + old % 2 * (mix(seed ^ 0x6f72616e67655f32) & 1) as usize
                });
            }
            assert!(layouts.windows(2).all(|p| p[0] != p[1]));
            assert_eq!(layouts[7], fruit_indices(fruit.len(), 7));
        }
    }
    #[test]
    fn streaming_builds_assets_stays_bounded_and_revisits_same_trees() {
        let start = (-200..200)
            .flat_map(|x| (-200..200).map(move |z| (x, z)))
            .find_map(|(x, z)| location(world_seed(), x, z).filter(|p| p.variant >= VARIANTS))
            .unwrap()
            .position
            + Vec3::Y * 1.25;
        let mut app = App::new();
        app.add_plugins(crate::world::streaming::StreamingPlugin);
        app.insert_resource(Assets::<Mesh>::default())
            .insert_resource(Assets::<Image>::default())
            .insert_resource(Assets::<StandardMaterial>::default())
            .insert_resource(crate::player::camera::CameraRig {
                position: start,
                ..default()
            })
            .add_systems(Startup, setup)
            .add_systems(Update, stream.in_set(Layer::Citrus));
        app.update();
        assert!(app.world().resource::<Streaming>().chunks.len() <= CHUNKS_PER_FRAME);
        for _ in 0..20 {
            app.update();
        }
        fn positions(world: &mut World) -> Vec<[u32; 3]> {
            let mut p: Vec<_> = world
                .query_filtered::<&Transform, With<WildTree>>()
                .iter(world)
                .map(|t| t.translation.to_array().map(f32::to_bits))
                .collect();
            p.sort();
            p
        }
        let original = positions(app.world_mut());
        let world = app.world_mut();
        assert!(
            world
                .query::<&Name>()
                .iter(world)
                .any(|name| name.as_str() == "Wild apple tree")
        );
        assert!(
            world.resource::<WildAssets>().models[VARIANTS..]
                .iter()
                .all(|m| m.fruit.len() >= 5)
        );
        fn fruit_positions(world: &mut World) -> Vec<[u32; 3]> {
            let mut positions: Vec<_> = world
                .query::<(&OrangeFruit, &ChildOf)>()
                .iter(world)
                .flat_map(|(fruit, parent)| {
                    let model = &world.resource::<WildAssets>().models[fruit.variant];
                    let transform = world.get::<Transform>(parent.parent()).unwrap();
                    fruit_indices(model.fruit.len(), fruit.layout as u64)
                        .into_iter()
                        .map(move |i| {
                            transform
                                .transform_point(model.fruit[i].center)
                                .to_array()
                                .map(f32::to_bits)
                        })
                })
                .collect();
            positions.sort();
            positions
        }
        let original_fruit = fruit_positions(app.world_mut());
        assert!(!original_fruit.is_empty());
        assert!(!original.is_empty());
        let old_chunks = app.world().resource::<Streaming>().chunks.clone();
        app.world_mut()
            .resource_mut::<crate::player::camera::CameraRig>()
            .position
            .x += CHUNK as f32 * SPACING;
        app.update();
        for _ in 0..20 {
            app.update();
        }
        let retained = app
            .world()
            .resource::<Streaming>()
            .chunks
            .iter()
            .filter(|(cell, entity)| old_chunks.get(cell) == Some(entity))
            .count();
        assert_eq!(
            retained, 42,
            "overlapping chunks must retain their entities"
        );
        assert_eq!(app.world().resource::<Streaming>().chunks.len(), 49);
        let mesh_count = app.world().resource::<Assets<Mesh>>().len();
        assert!(
            app.world().resource::<WildAssets>().models[..VARIANTS]
                .iter()
                .all(|m| m.fruit.len() == 64)
        );
        // Cross positive and negative chunk boundaries, then return.
        for point in [
            Vec3::new(96.0, 1.25, -65.0),
            Vec3::new(-96.0, 1.25, 65.0),
            start,
        ] {
            app.world_mut()
                .resource_mut::<crate::player::camera::CameraRig>()
                .position = point;
            app.update();
            for _ in 0..20 {
                app.update();
            }
            assert_eq!(app.world().resource::<Streaming>().chunks.len(), 49);
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), mesh_count);
            let roots = app
                .world()
                .resource::<Streaming>()
                .chunks
                .values()
                .copied()
                .collect::<Vec<_>>();
            assert!(roots.iter().all(|e| app.world().get_entity(*e).is_ok()));
        }
        assert_eq!(original, positions(app.world_mut()));
        assert_eq!(original_fruit, fruit_positions(app.world_mut()));
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<&WildFoliage, With<bevy::light::NotShadowCaster>>()
                .iter(world)
                .count(),
            original.len() * 4
        );
        assert_eq!(
            world
                .query::<(&Name, &bevy::camera::visibility::RenderLayers)>()
                .iter(world)
                .filter(|(name, layers)| name.as_str() == "Citrus shadow canopy"
                    && *layers == &super::super::lod::shadow_layers())
                .count(),
            original.len()
        );
        assert_eq!(
            app.world_mut()
                .query::<&WildFoliage>()
                .iter(app.world())
                .count(),
            original.len() * 4
        );
        // Flying vertically must refresh detail without crossing a chunk edge.
        // This also checks the material and tiny-fruit-detail visibility swaps.
        let tree_position = app
            .world_mut()
            .query::<&WildFoliage>()
            .iter(app.world())
            .next()
            .unwrap()
            .position;
        app.world_mut()
            .resource_mut::<crate::player::camera::CameraRig>()
            .position = tree_position + Vec3::Y * 50.0;
        app.update();
        let center = app.world().resource::<Streaming>().center;
        for expected_near in [true, false] {
            app.world_mut()
                .resource_mut::<crate::player::camera::CameraRig>()
                .position = tree_position + Vec3::Y * if expected_near { 0.0 } else { 50.0 };
            app.update();
            assert_eq!(app.world().resource::<Streaming>().center, center);
            let world = app.world_mut();
            let mut query = world.query::<(
                &WildFoliage,
                &Mesh3d,
                &MeshMaterial3d<StandardMaterial>,
                &Visibility,
            )>();
            let assets = world.resource::<WildAssets>();
            let mut count = 0;
            for (part, mesh, material, visibility) in query.iter(world) {
                if part.position != tree_position {
                    continue;
                }
                count += 1;
                assert_eq!(part.near, expected_near);
                assert_eq!(
                    mesh.0,
                    assets.models[part.variant].mesh(part.part, part.layout, expected_near)
                );
                if part.part == 1 {
                    assert_eq!(
                        material.0,
                        if part.variant >= VARIANTS {
                            assets.apple_materials[1].clone()
                        } else {
                            assets.leaves.clone()
                        }
                    );
                }
                if part.part == 3 {
                    assert_eq!(
                        *visibility,
                        if expected_near {
                            Visibility::Inherited
                        } else {
                            Visibility::Hidden
                        }
                    );
                }
            }
            assert_eq!(count, 4);
        }
        // Harvest both species completely, including a simultaneous LOD change.
        // Neither live updates nor chunk re-entry may upload empty fruit assets.
        let targets: Vec<_> = [false, true]
            .into_iter()
            .map(|apple| {
                let world = app.world_mut();
                let tree = world
                    .query::<&WildTree>()
                    .iter(world)
                    .find(|t| (t.placement.variant >= VARIANTS) == apple)
                    .unwrap();
                let model = &world.resource::<WildAssets>().models[tree.placement.variant];
                (
                    tree.cell,
                    selected_fruit(model.fruit.len(), tree.layout, apple).len(),
                    apple,
                )
            })
            .collect();
        let mesh_count = app.world().resource::<Assets<Mesh>>().len();
        for &(cell, count, apple) in &targets {
            for slot in 0..count {
                harvest(
                    &mut app.world_mut().resource_mut::<Streaming>(),
                    cell,
                    slot,
                    apple,
                );
            }
        }
        let check_harvested = |world: &mut World| {
            for &(cell, _, _) in &targets {
                let parts: Vec<_> = world
                    .query::<(&WildFoliage, Option<&Mesh3d>)>()
                    .iter(world)
                    .filter(|(part, _)| part.cell == cell)
                    .collect();
                assert_eq!(parts.len(), 4);
                for (part, mesh) in parts {
                    assert_eq!(mesh.is_some(), part.part < 2);
                }
            }
            assert_eq!(world.resource::<Assets<Mesh>>().len(), mesh_count);
            assert!(
                world
                    .resource::<Assets<Mesh>>()
                    .iter()
                    .all(|(_, mesh)| mesh.count_vertices() > 0
                        && mesh.indices().is_none_or(|i| !i.is_empty()))
            );
        };
        for height in [0.0, 80.0, 0.0] {
            app.world_mut()
                .resource_mut::<crate::player::camera::CameraRig>()
                .position = start + Vec3::Y * height;
            for _ in 0..21 {
                app.update();
            }
            check_harvested(app.world_mut());
        }
        app.world_mut()
            .resource_mut::<crate::player::camera::CameraRig>()
            .position = start + Vec3::X * 1000.0;
        for _ in 0..21 {
            app.update();
        }
        for &(cell, _, _) in &targets {
            let world = app.world_mut();
            assert!(
                !world
                    .query::<&WildTree>()
                    .iter(world)
                    .any(|tree| tree.cell == cell)
            );
        }
        app.world_mut()
            .resource_mut::<crate::player::camera::CameraRig>()
            .position = start;
        for _ in 0..21 {
            app.update();
        }
        check_harvested(app.world_mut());
        for p in nearby(Vec3::new(0.0, 0.0, 20.0), 20) {
            assert!(player_collides(p.position + Vec3::Y * 1.25, 0.28));
            assert!(camera_obstructed(p.position + Vec3::Y, 0.1));
        }
    }

    #[test]
    fn stand_sites_repeat_exactly_and_respect_growing_room() {
        assert_ne!(cell_hash(1, -10, 20), cell_hash(1, 10, -20));
        for x in -100..100 {
            for z in -100..100 {
                let a = location(721, x, z);
                let b = location(721, x, z);
                assert_eq!(a.map(|p| p.position), b.map(|p| p.position));
                if let Some(p) = a {
                    assert!(eligible(p.position.xz()));
                    assert!((p.transform().rotation * Vec3::Y - Vec3::Y).length() < 0.00001);
                    for i in 0..16 {
                        let a = i as f32 * TAU / 16.0;
                        let foot = p.position.xz() + Vec2::new(a.cos(), a.sin()) * 0.42 * p.scale;
                        assert!(p.position.y <= crate::world::terrain::height(foot) + 0.001);
                    }
                    for dx in -2..=2 {
                        for dz in -2..=2 {
                            if dx == 0 && dz == 0 {
                                continue;
                            }
                            if let Some(q) = location(721, x + dx, z + dz) {
                                assert!(p.position.distance(q.position) >= 2.0);
                            }
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn walking_past_world_trees_does_not_trap_the_player() {
        let mut point = Vec3::new(0.0, 1.25, 3.15);
        for _ in 0..600 {
            point = crate::player::movement::move_player_with_collisions(point, Vec3::Z * 0.038);
        }
        let before = point;
        // Seed 721 places a trunk on the left edge of this path. Walk right
        // around it, not left into it; collision must allow the escape route.
        for _ in 0..100 {
            point = crate::player::movement::move_player_with_collisions(point, Vec3::X * 0.038);
        }
        assert!(point.x > before.x + 3.0, "{before:?} -> {point:?}");
        for _ in 0..500 {
            point = crate::player::movement::move_player_with_collisions(point, Vec3::Z * 0.038);
        }
        assert!(point.z > 16.0, "failed to cross chunk boundary: {point:?}");
    }
    #[test]
    fn batched_fruit_keeps_anchors_and_close_detail_with_cheaper_distant_spheres() {
        let sphere = super::super::lod::primitive(Sphere::new(1.0).mesh().uv(20, 12));
        let far_sphere = super::super::lod::primitive(Sphere::new(1.0).mesh().uv(8, 6));
        let stem =
            super::super::lod::primitive(Cylinder::new(1.0, 1.0).mesh().resolution(6).build());
        let calyx = super::super::lod::primitive(Sphere::new(1.0).mesh().uv(8, 4));
        let fruit = fruit_candidates(&variants()[0], 7723);
        for layout in 0..FRUIT_LAYOUTS {
            let indices = fruit_indices(fruit.len(), layout as u64);
            let (near, details) = fruit_geometry(&fruit, layout, &sphere, &stem, &calyx);
            let (far, _) = fruit_geometry(&fruit, layout, &far_sphere, &stem, &calyx);
            assert_eq!(near.positions.len(), sphere.positions.len() * indices.len());
            assert_eq!(
                far.positions.len(),
                far_sphere.positions.len() * indices.len()
            );
            assert!(far.indices.len() < near.indices.len() / 4);
            for (slot, index) in indices.iter().enumerate() {
                let f = fruit[*index];
                for (geometry, vertices) in [
                    (&near, sphere.positions.len()),
                    (&far, far_sphere.positions.len()),
                ] {
                    for p in &geometry.positions[slot * vertices..(slot + 1) * vertices] {
                        assert!((Vec3::from(*p).distance(f.center) - f.radius).abs() < 1e-5);
                    }
                }
            }
            for g in [&near, &far, &details] {
                assert!(g.positions.iter().all(|p| Vec3::from(*p).is_finite()));
                assert!(
                    g.normals
                        .iter()
                        .all(|n| (Vec3::from(*n).length() - 1.0).abs() < 1e-4)
                );
                assert!(g.indices.iter().all(|i| (*i as usize) < g.positions.len()));
            }
        }
    }

    #[test]
    fn citrus_variants_have_dense_crowns_and_valid_low_detail_meshes() {
        assert_eq!(variants().len(), VARIANTS);
        let counts: HashSet<_> = variants().iter().map(|t| t.wood.positions.len()).collect();
        assert!(
            counts.len() >= 5,
            "branch topology should vary, not just pose"
        );
        for tree in variants() {
            let distant_wood = tree.distant_wood.as_ref().unwrap();
            assert!(distant_wood.indices.len() < tree.wood.indices.len() / 2);
            assert!(
                distant_wood
                    .positions
                    .iter()
                    .all(|p| Vec3::from(*p).is_finite())
            );
            assert!(
                tree.leaves
                    .positions
                    .iter()
                    .all(|p| Vec3::from(*p).is_finite())
            );
            let sparse = sparse_leaves(&tree.leaves);
            assert_eq!(sparse.indices.len(), tree.leaves.indices.len() / 4);
            assert_eq!(
                sparse.positions.len() / 5,
                tree.leaves.positions.len() / orange_tree::LEAF_VERTICES
            );
            // The distant mesh must preserve authored shading data, including
            // the raised midrib. LOD must not be a different-colored material.
            for leaf in 0..sparse.positions.len() / 5 {
                for (out, source) in [1, 6, 7, 8, 13].into_iter().enumerate() {
                    let i = leaf * 5 + out;
                    let j = leaf * orange_tree::LEAF_VERTICES + source;
                    assert_eq!(sparse.positions[i], tree.leaves.positions[j]);
                    assert_eq!(sparse.normals[i], tree.leaves.normals[j]);
                    assert_eq!(sparse.uvs[i], tree.leaves.uvs[j]);
                    assert_eq!(sparse.colors[i], tree.leaves.colors[j]);
                }
            }
            assert!(
                sparse
                    .indices
                    .iter()
                    .all(|i| (*i as usize) < sparse.positions.len())
            );
            assert!(tree.fruit_spurs.len() > 100);
            assert!(
                (135 * 25 * orange_tree::LEAF_VERTICES..190 * 25 * orange_tree::LEAF_VERTICES)
                    .contains(&tree.leaves.positions.len())
            );
            let min_y = tree
                .leaves
                .positions
                .iter()
                .map(|p| p[1])
                .fold(f32::INFINITY, f32::min);
            let max_y = tree
                .leaves
                .positions
                .iter()
                .map(|p| p[1])
                .fold(f32::NEG_INFINITY, f32::max);
            assert!(min_y < 1.4 && max_y > 2.8 && max_y - min_y > 1.8);
            for (n, t) in tree.leaves.normals.iter().zip(tree.leaves.tangents()) {
                assert!(Vec4::from(t).is_finite());
                assert!(Vec3::from(*n).dot(Vec3::from_slice(&t)).abs() < 1e-4);
            }
        }
    }
}

#[cfg(test)]
#[test]
fn citrus_altitude_boundary() {
    assert!(altitude_allowed(50.0));
    assert!(altitude_allowed(-20.0));
    assert!(!altitude_allowed(50.001));
    assert!(!altitude_allowed(f32::NAN));
}

//! Shared crown representations for authored vegetation. Physics never reads these meshes.
use crate::world::streaming::{BuildTask, Coordinator, Layer, mesh_bytes};
use bevy::{camera::primitives::MeshAabb, mesh::VertexAttributeValues, prelude::*};
use std::collections::HashMap;
fn legacy_selection() -> bool {
    static LEGACY: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *LEGACY.get_or_init(|| {
        std::env::var_os("HITHER_PROFILE_FOREST").is_some()
            && std::env::var_os("HITHER_PROFILE_LEGACY_PLANT_LOD").is_some()
    })
}
fn projected_level(mut level: usize, pixels: f32) -> usize {
    if legacy_selection() {
        let bias = if level == 0 { 0.9 } else { 1.1 };
        return if pixels < 48. * bias {
            2
        } else if pixels < 192. * bias {
            1
        } else {
            0
        };
    }
    let thresholds = [192., 48.];
    while level < 2 && pixels < thresholds[level] * 0.9 {
        level += 1;
    }
    while level > 0 && pixels > thresholds[level - 1] * 1.1 {
        level -= 1;
    }
    level
}
#[derive(Component)]
pub(crate) struct AuthoredLods {
    pub(crate) levels: [Handle<Mesh>; 2],
    pub(crate) extent: f32,
}
#[derive(Component)]
pub(crate) struct Canopy(pub bool);
#[derive(Component)]
pub(crate) struct ShadowProxy;
#[derive(Component)]
struct Selection {
    source: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    displayed: AssetId<Mesh>,
    ready: bool,
    level: usize,
}
#[derive(Default)]
struct Checks {
    position: Option<Vec3>,
    travel: f64,
    scheduled: std::collections::BinaryHeap<std::cmp::Reverse<(u64, Entity)>>,
    due: HashMap<Entity, u64>,
    waiting: std::collections::HashSet<Entity>,
    retry: std::collections::HashSet<Entity>,
}
impl Checks {
    fn schedule(&mut self, entity: Entity, clearance: f32) {
        let due = (self.travel * 1000.0) as u64 + (clearance.max(0.0) as f64 * 800.0) as u64;
        self.due.insert(entity, due);
        self.scheduled.push(std::cmp::Reverse((due, entity)));
    }
}
struct Crown {
    bounds: bevy::camera::primitives::Aabb,
    source: Handle<Mesh>,
    levels: Option<[Handle<Mesh>; 2]>,
    requested: bool,
    task: Option<BuildTask<[Mesh; 2]>>,
}
#[derive(Resource, Default)]
struct Crowns {
    meshes: HashMap<AssetId<Mesh>, Crown>,
    materials: HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>>,
}
pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<Crowns>().add_systems(
        PostUpdate,
        select
            .after(TransformSystems::Propagate)
            .before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
    );
}
// Aggregate leaves into bounded crown volumes, rather than retaining one object
// per leaf. Group colors before averaging so snow doesn't turn green crowns gray.
type ClusterBin = (Vec3, Vec3, Vec3, usize);
fn cluster(mesh: &Mesh, divisions: i32) -> Mesh {
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return mesh.clone();
    };
    let bounds = mesh.compute_aabb().unwrap();
    let min = Vec3::from(bounds.min());
    let size = Vec3::from(bounds.half_extents) * 2.0;
    let colors = match mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
        Some(VertexAttributeValues::Float32x4(c)) => Some(c),
        _ => None,
    };
    let mut bins: std::collections::BTreeMap<(i32, i32, i32, bool), ClusterBin> =
        Default::default();
    for (i, p) in positions.iter().enumerate() {
        let p = Vec3::from(*p);
        let c = colors.map_or(Vec3::ONE, |c| Vec4::from(c[i]).truncate());
        let cell = ((p - min) / size.max(Vec3::splat(0.001)) * divisions as f32)
            .floor()
            .as_ivec3()
            .clamp(IVec3::ZERO, IVec3::splat(divisions - 1));
        let bin = bins
            .entry((cell.x, cell.y, cell.z, c.min_element() > 0.5))
            .or_insert((p, p, Vec3::ZERO, 0));
        bin.0 = bin.0.min(p);
        bin.1 = bin.1.max(p);
        bin.2 += c;
        bin.3 += 1;
    }
    let mut out = super::geometry::Geometry::default();
    for (_, (lo, hi, color, n)) in bins {
        let center = (lo + hi) * 0.5;
        let extent = ((hi - lo) * 0.5).max(size * 0.005);
        let base = out.positions.len() as u32;
        for direction in [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z] {
            out.vertex(center + direction * extent, Vec2::ZERO, color / n as f32);
        }
        for [a, b, c] in [
            [2, 0, 4],
            [2, 4, 1],
            [2, 1, 5],
            [2, 5, 0],
            [3, 4, 0],
            [3, 1, 4],
            [3, 5, 1],
            [3, 0, 5],
        ] {
            // Outward winding is essential even for double-sided materials:
            // generated tangent attributes suppress Bevy's normal flip when
            // the simplified material has no normal map.
            out.triangle(base + a, base + c, base + b);
        }
    }
    if divisions > 1
        && out.indices.len() >= mesh.indices().map_or(mesh.count_vertices(), |i| i.len()) * 3 / 4
    {
        return cluster(mesh, divisions / 2);
    }
    out.finish_normals();
    out.mesh()
}
// Generated foliage textures carry a linear-light filtered 1x1 mip. Preserve
// that albedo when replacing textured leaves with untextured crown volumes.
fn average_color(image: &Image) -> Option<Vec4> {
    use bevy::render::render_resource::TextureFormat;
    if image.texture_descriptor.size.depth_or_array_layers != 1 {
        return None;
    }
    let srgb = match image.texture_descriptor.format {
        TextureFormat::Rgba8UnormSrgb => true,
        TextureFormat::Rgba8Unorm => false,
        _ => return None,
    };
    let data = image.data.as_ref()?;
    let pixels = if image.texture_descriptor.mip_level_count > 1 {
        data.get(data.len().checked_sub(4)?..)?
    } else {
        data.as_slice()
    };
    if pixels.is_empty() {
        return None;
    }
    let mut sum = Vec4::ZERO;
    let mut count = 0.;
    for pixel in pixels.chunks_exact(4) {
        sum += if srgb {
            let c = Color::srgba_u8(pixel[0], pixel[1], pixel[2], pixel[3]).to_linear();
            Vec4::new(c.red, c.green, c.blue, c.alpha)
        } else {
            Vec4::new(
                pixel[0] as f32,
                pixel[1] as f32,
                pixel[2] as f32,
                pixel[3] as f32,
            ) / 255.0
        };
        count += 1.;
    }
    Some(sum / count)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn select(
    mut commands: Commands,
    mut cache: ResMut<Crowns>,
    mut coordinator: ResMut<Coordinator>,
    view: Res<crate::player::camera::CameraView>,
    settings: Res<crate::app::settings::GraphicsSettings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    images: Res<Assets<Image>>,
    mut checks: Local<Checks>,
    mut removed: RemovedComponents<Selection>,
    mut queries: ParamSet<(
        Query<
            Entity,
            (
                Or<(With<Canopy>, With<ShadowProxy>)>,
                Or<(
                    Changed<Mesh3d>,
                    Changed<GlobalTransform>,
                    Changed<Canopy>,
                    Added<ShadowProxy>,
                )>,
            ),
        >,
        Query<
            (
                Entity,
                Ref<GlobalTransform>,
                &mut Mesh3d,
                &mut MeshMaterial3d<StandardMaterial>,
                Option<&Canopy>,
                Has<ShadowProxy>,
                Option<&mut Selection>,
                Option<&AuthoredLods>,
            ),
            Or<(With<Canopy>, With<ShadowProxy>)>,
        >,
    )>,
) {
    // Poll before selection; each shared source has at most one job and two
    // derived meshes, regardless of the number of instances in the world.
    if let Some(previous) = checks.position.replace(view.position) {
        checks.travel += previous.distance(view.position) as f64;
    }
    for entity in removed.read() {
        checks.due.remove(&entity);
        checks.waiting.remove(&entity);
        checks.retry.remove(&entity);
    }
    let refresh = settings.is_changed() || coordinator.projection_changed();
    let mut completed = false;
    for crown in cache.meshes.values_mut() {
        if crown.requested
            && crown.levels.is_none()
            && crown.task.is_none()
            && let Some(source) = meshes.get(&crown.source)
            && source.try_attribute(Mesh::ATTRIBUTE_POSITION).is_ok()
            && let Some(permit) = coordinator.worker(Layer::Conifers)
        {
            let source = source.clone();
            crown.task = Some(BuildTask::new(permit, move || {
                [cluster(&source, 8), cluster(&source, 3)]
            }));
        }
        if let Some(task) = &mut crown.task
            && task.is_ready()
        {
            let bytes = task.ready_ref().unwrap().iter().map(mesh_bytes).sum();
            if let Some(_install) = coordinator.install(Layer::Conifers, 2, bytes) {
                crown.levels = Some(task.take().map(|m| meshes.add(m)));
                crown.task = None;
                completed = true;
            }
        }
    }
    // A camera can change an object's distance by at most its path length.
    // Schedule the next check at a conservative fraction of the nearest LOD
    // boundary clearance. Far populations therefore need no movement sweep.
    let mut candidates: std::collections::HashSet<_> = queries.p0().iter().collect();
    let mut plants = queries.p1();
    candidates.extend(checks.retry.drain());
    if completed {
        candidates.extend(checks.waiting.drain());
    }
    if refresh {
        candidates.extend(plants.iter().map(|p| p.0));
        checks.scheduled.clear();
        checks.due.clear();
    }
    let now = (checks.travel * 1000.0) as u64;
    let limit = if legacy_selection() { usize::MAX } else { 512 };
    let mut popped = 0;
    while popped < limit && checks.scheduled.peek().is_some_and(|v| v.0.0 <= now) {
        popped += 1;
        let std::cmp::Reverse((due, entity)) = checks.scheduled.pop().unwrap();
        if checks.due.get(&entity) == Some(&due) {
            candidates.insert(entity);
        }
    }
    let mut candidates: Vec<_> = candidates
        .into_iter()
        .map(|e| {
            let distance = plants.get(e).map_or(f32::INFINITY, |p| {
                p.1.translation().distance_squared(view.position)
            });
            (e, distance)
        })
        .collect();
    if candidates.len() > limit {
        candidates.select_nth_unstable_by(limit, |a, b| {
            a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0))
        });
        checks.retry.extend(candidates.drain(limit..).map(|p| p.0));
    }
    for (entity, _) in candidates {
        let Ok((entity, transform, mut mesh, mut material, canopy, shadow, selection, authored)) =
            plants.get_mut(entity)
        else {
            continue;
        };
        if !shadow && !canopy.is_some_and(|c| c.0) {
            continue;
        }
        let mut selection = match selection {
            Some(s) => s,
            None => {
                checks.retry.insert(entity);
                commands.entity(entity).insert(Selection {
                    source: mesh.0.clone(),
                    material: material.0.clone(),
                    displayed: mesh.id(),
                    ready: false,
                    level: 0,
                });
                continue;
            }
        };
        // Authored near/far adapters can change the source at any time.
        if mesh.id() != selection.displayed {
            selection.ready = false;
            selection.source = mesh.0.clone();
            selection.level = 0;
        }
        let id = selection.source.id();
        if let std::collections::hash_map::Entry::Vacant(entry) = cache.meshes.entry(id) {
            let Some(source) = meshes.get(id) else {
                checks.retry.insert(entity);
                continue;
            };
            if source.try_attribute(Mesh::ATTRIBUTE_POSITION).is_err() {
                continue;
            }
            let Some(bounds) = source.compute_aabb() else {
                continue;
            };
            entry.insert(Crown {
                bounds,
                source: selection.source.clone(),
                levels: authored.map(|a| a.levels.clone()),
                requested: false,
                task: None,
            });
        }
        let crown = cache.meshes.get_mut(&id).unwrap();
        let (scale, _, origin) = transform.to_scale_rotation_translation();
        let distance = origin.distance(view.position).max(1.0);
        let extent = authored.map_or_else(
            || (scale.abs() * Vec3::from(crown.bounds.half_extents)).length(),
            |a| a.extent * scale.abs().max_element(),
        );
        let pixels = if shadow {
            let resolution = match settings.shadow_quality {
                super::graphics::Quality::Low => 512.,
                super::graphics::Quality::Medium => 1024.,
                super::graphics::Quality::High => 2048.,
            };
            extent * resolution
                / if distance < super::view_distance::shadow_distance(&settings) * 0.25 + extent {
                    super::view_distance::shadow_distance(&settings) * 0.25
                } else {
                    super::view_distance::shadow_distance(&settings)
                }
        } else {
            extent * 2.0 * coordinator.focal_pixels() / distance
        };
        let level = if !shadow && distance < settings.detail_distance.min(settings.render_distance)
        {
            0
        } else {
            projected_level(selection.level, pixels)
        };
        crown.requested |= level > 0;
        let selected = if level == 0 {
            crown.source.clone()
        } else if let Some(levels) = &crown.levels {
            levels[level - 1].clone()
        } else {
            crown.source.clone()
        };
        let waiting = crown.requested && crown.levels.is_none();
        let simplified = selected.id() != id && authored.is_none();
        if mesh.id() != selected.id() {
            mesh.0 = selected;
        }
        if simplified {
            let id = selection.material.id();
            if !cache.materials.contains_key(&id)
                && let Some(source) = materials.get(id)
            {
                let mut crown = source.clone();
                if let Some(texture) = &crown.base_color_texture
                    && let Some(image) = images.get(texture)
                    && let Some(average) = average_color(image)
                {
                    let base = crown.base_color.to_linear();
                    crown.base_color = Color::linear_rgba(
                        base.red * average.x,
                        base.green * average.y,
                        base.blue * average.z,
                        base.alpha,
                    );
                    crown.base_color_texture = None;
                }
                crown.normal_map_texture = None;
                crown.alpha_mode = AlphaMode::Opaque;
                crown.cull_mode = None;
                crown.double_sided = true;
                cache.materials.insert(id, materials.add(crown));
            }
            if let Some(crown) = cache.materials.get(&id)
                && material.0 != *crown
            {
                material.0 = crown.clone();
            }
        } else {
            if material.0 != selection.material {
                material.0 = selection.material.clone();
            }
        }
        selection.ready = true;
        selection.displayed = mesh.id();
        selection.level = level;
        if waiting {
            checks.waiting.insert(entity);
        }
        let bias = if level == 0 { 0.9 } else { 1.1 };
        let clearance = if shadow {
            (distance - (super::view_distance::shadow_distance(&settings) * 0.25 + extent)).abs()
        } else {
            let scale = extent * 2.0 * coordinator.focal_pixels();
            [
                settings.detail_distance.min(settings.render_distance),
                scale
                    / (48.0
                        * if legacy_selection() || level == 2 {
                            bias
                        } else {
                            0.9
                        }),
                scale / (192.0 * bias),
            ]
            .into_iter()
            .map(|boundary| (distance - boundary).abs())
            .fold(f32::INFINITY, f32::min)
        };
        checks.schedule(entity, clearance);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_projected_boundary_has_separate_enter_and_exit_thresholds() {
        for (near, pixels) in [(0, 192.), (1, 48.)] {
            assert_eq!(projected_level(near, pixels), near);
            assert_eq!(projected_level(near + 1, pixels), near + 1);
            assert_eq!(projected_level(near, pixels * 0.89), near + 1);
            assert_eq!(projected_level(near + 1, pixels * 1.11), near);
        }
        assert_eq!(projected_level(0, 10.), 2);
        assert_eq!(projected_level(2, 300.), 0);
    }

    #[test]
    fn stationary_selection_accepts_new_plants_and_updates_after_movement() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<crate::app::settings::GraphicsSettings>()
            .init_resource::<crate::rendering::view_distance::Range>()
            .insert_resource(crate::player::camera::CameraView {
                position: Vec3::ZERO,
                forward: -Vec3::Z,
                right: Vec3::X,
                up: Vec3::Y,
            })
            .add_plugins(crate::world::streaming::StreamingPlugin);
        plugin(&mut app);
        app.update();
        app.update();
        let source = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Sphere::new(3.0).mesh().uv(64, 32));
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let plant = app
            .world_mut()
            .spawn((
                Canopy(true),
                GlobalTransform::default(),
                Mesh3d(source.clone()),
                MeshMaterial3d(material),
            ))
            .id();
        for _ in 0..3 {
            app.update();
        }
        assert!(app.world().get::<Selection>(plant).unwrap().ready);
        assert_eq!(app.world().get::<Mesh3d>(plant).unwrap().0, source);
        app.world_mut()
            .resource_mut::<crate::player::camera::CameraView>()
            .position = Vec3::Z * 300.;
        for _ in 0..4 {
            app.update();
        }
        assert_ne!(app.world().get::<Mesh3d>(plant).unwrap().0, source);
        app.world_mut()
            .resource_mut::<crate::player::camera::CameraView>()
            .position = Vec3::ZERO;
        app.update();
        assert_eq!(app.world().get::<Mesh3d>(plant).unwrap().0, source);
    }
    #[test]
    fn authored_levels_keep_materials_and_return_to_the_current_source() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<crate::app::settings::GraphicsSettings>()
            .init_resource::<crate::rendering::view_distance::Range>()
            .insert_resource(crate::player::camera::CameraView {
                position: Vec3::Z * 300.,
                forward: -Vec3::Z,
                right: Vec3::X,
                up: Vec3::Y,
            })
            .add_plugins(crate::world::streaming::StreamingPlugin);
        plugin(&mut app);
        app.update();
        let source = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Sphere::new(3.).mesh().uv(32, 16));
        let levels = [
            app.world_mut()
                .resource_mut::<Assets<Mesh>>()
                .add(Sphere::new(3.).mesh().uv(16, 8)),
            app.world_mut()
                .resource_mut::<Assets<Mesh>>()
                .add(Sphere::new(3.).mesh().uv(8, 4)),
        ];
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                alpha_mode: AlphaMode::Mask(0.4),
                ..default()
            });
        let entity = app
            .world_mut()
            .spawn((
                Canopy(true),
                AuthoredLods {
                    levels: levels.clone(),
                    extent: 3.,
                },
                GlobalTransform::default(),
                Mesh3d(source.clone()),
                MeshMaterial3d(material.clone()),
            ))
            .id();
        for _ in 0..4 {
            app.update();
        }
        assert_eq!(app.world().get::<Mesh3d>(entity).unwrap().0, levels[1]);
        assert_eq!(
            app.world()
                .get::<MeshMaterial3d<StandardMaterial>>(entity)
                .unwrap()
                .0,
            material
        );
        assert!(
            app.world()
                .resource::<Crowns>()
                .meshes
                .values()
                .all(|c| c.task.is_none())
        );
        app.world_mut()
            .resource_mut::<crate::player::camera::CameraView>()
            .position = Vec3::ZERO;
        app.update();
        assert_eq!(app.world().get::<Mesh3d>(entity).unwrap().0, source);
        assert_eq!(app.world().resource::<Assets<StandardMaterial>>().len(), 1);
    }
    #[test]
    fn crown_faces_and_normals_point_outward() {
        // One cluster isolates the topology from placement and source colors.
        let source = Sphere::new(3.0).mesh().uv(64, 32);
        let crown = cluster(&source, 1);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            crown.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        let Some(VertexAttributeValues::Float32x3(normals)) =
            crown.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("normals");
        };
        let center = Vec3::from(crown.compute_aabb().unwrap().center);
        let indices: Vec<_> = crown.indices().unwrap().iter().collect();
        for face in indices.chunks_exact(3) {
            let [a, b, c] = [face[0], face[1], face[2]].map(|i| Vec3::from(positions[i]));
            assert!(
                (b - a).cross(c - a).dot((a + b + c) / 3.0 - center) > 0.0,
                "closed crown faces must face outwards"
            );
        }
        for (p, n) in positions.iter().zip(normals) {
            assert!(
                (Vec3::from(*p) - center).dot(Vec3::from(*n)) > 0.0,
                "foliage must be illuminated from outside the crown"
            );
        }
        assert_eq!(indices.len(), 24, "fix must not add geometry");
    }
    #[test]
    fn crown_albedo_uses_the_linear_light_texture_average() {
        use bevy::{
            asset::RenderAssetUsages,
            render::render_resource::{Extent3d, TextureDimension, TextureFormat},
        };
        let mut image = Image::new(
            Extent3d {
                width: 2,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            vec![255, 0, 0, 255, 0, 0, 255, 255],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        assert_eq!(average_color(&image), Some(Vec4::new(0.5, 0.0, 0.5, 1.0)));
        super::super::mipmaps::generate(&mut image, super::super::mipmaps::Filter::Color);
        assert!(
            average_color(&image)
                .unwrap()
                .distance(Vec4::new(0.5, 0.0, 0.5, 1.0))
                < 0.01
        );
    }
    #[test]
    fn distant_crowns_have_bounded_geometry_and_preserve_extent() {
        let source = Sphere::new(3.0).mesh().uv(64, 32);
        for divisions in [2, 4] {
            let result = cluster(&source, divisions);
            assert!(result.count_vertices() <= 6 * (divisions.pow(3) as usize) * 2);
            assert!(result.count_vertices() < source.count_vertices());
            let bounds = result.compute_aabb().unwrap();
            assert!((Vec3::from(bounds.half_extents) - Vec3::splat(3.)).length() < 0.1);
        }
    }
}

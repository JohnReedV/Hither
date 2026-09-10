//! Shared photometric lighting and item emission. Author power, not render entities.
use crate::{app::settings::GraphicsSettings, player::camera::CameraView};
use bevy::{camera::visibility::RenderLayers, prelude::*};
use serde::{Deserialize, Serialize};

pub(crate) struct LightingPlugin;
impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        super::lighting_environment::plugin(app);
        app.init_resource::<LightingStats>()
            .add_systems(Startup, setup)
            .add_systems(PostStartup, validation_scene)
            .add_systems(Update, (reconcile, sync_offsets, sync_glow).chain())
            .add_systems(
                PostUpdate,
                update
                    .after(bevy::transform::TransformSystems::Propagate)
                    .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
            );
    }
}

/// Attach to any item. Power is lumens; range and offset are world/local metres.
/// Emission is independent of the item's material and follows its hierarchy.
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
#[require(Transform, Visibility)]
#[serde(default)]
pub(crate) struct LightEmitter {
    pub lumens: f32,
    pub color: [f32; 3],
    /// Zero selects an automatically derived, bounded influence radius.
    pub range: f32,
    pub radius: f32,
    pub offset: [f32; 3],
    pub enabled: bool,
    pub shadows: EmissionShadows,
    /// Higher priorities retain lighting when a scene exceeds its budget.
    pub priority: f32,
    /// Surface glow on this entity's StandardMaterial; zero keeps original emission.
    /// Child flame/lantern materials can remain independently authored.
    pub glow: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EmissionShadows {
    /// Suppress contribution when a shadow slot is unavailable, never leak through walls.
    #[default]
    Required,
    /// Only for short-range accents whose influence stays inside an open space.
    None,
}
impl Default for LightEmitter {
    fn default() -> Self {
        Self {
            lumens: 800.0,
            color: [1.0, 0.76, 0.48],
            range: 0.0,
            radius: 0.08,
            offset: [0.0; 3],
            enabled: true,
            shadows: EmissionShadows::Required,
            priority: 1.0,
            glow: 1.0,
        }
    }
}
impl LightEmitter {
    pub fn new(lumens: f32) -> Self {
        Self {
            lumens,
            ..default()
        }
    }
    fn power(&self) -> f32 {
        if self.enabled && self.lumens.is_finite() {
            self.lumens.clamp(0.0, 1_000_000.0)
        } else {
            0.0
        }
    }
    fn reach(&self) -> f32 {
        if self.range.is_finite() && self.range > 0.0 {
            self.range.clamp(0.25, 64.0)
        } else {
            (self.power() / (4.0 * std::f32::consts::PI * 2.0))
                .sqrt()
                .clamp(0.25, 32.0)
        }
    }
}

#[derive(Component)]
struct EmittedLight(Entity);
#[derive(Component)]
struct EmitterChild(Entity);

#[derive(Component)]
struct EmissionMaterial {
    original: Handle<StandardMaterial>,
    derived: Handle<StandardMaterial>,
    cast_shadows: bool,
}

fn sync_offsets(
    sources: Query<(&LightEmitter, &EmitterChild), Changed<LightEmitter>>,
    mut transforms: Query<&mut Transform, With<EmittedLight>>,
) {
    for (source, child) in &sources {
        if let Ok(mut transform) = transforms.get_mut(child.0) {
            let offset = Vec3::from_array(source.offset);
            transform.set_if_neq(Transform::from_translation(if offset.is_finite() {
                offset
            } else {
                Vec3::ZERO
            }));
        }
    }
}

#[allow(clippy::type_complexity)] // Filtered ECS material ownership query.
fn sync_glow(
    mut commands: Commands,
    mut assets: ResMut<Assets<StandardMaterial>>,
    mut meshes: Query<
        (
            Entity,
            Option<Ref<LightEmitter>>,
            &mut MeshMaterial3d<StandardMaterial>,
            Option<&EmissionMaterial>,
            Option<&bevy::light::NotShadowCaster>,
        ),
        Or<(With<LightEmitter>, With<EmissionMaterial>)>,
    >,
) {
    for (entity, emitter, mut material, derived, not_caster) in &mut meshes {
        let Some(emitter) =
            emitter.filter(|e| e.glow.is_finite() && e.glow > 0.0 && e.power() > 0.0)
        else {
            if let Some(derived) = derived {
                if material.0 == derived.derived {
                    material.0 = derived.original.clone();
                }
                if derived.cast_shadows {
                    commands
                        .entity(entity)
                        .remove::<bevy::light::NotShadowCaster>();
                }
                commands.entity(entity).remove::<EmissionMaterial>();
            }
            continue;
        };
        if let Some(derived) = derived {
            if material.0 != derived.derived {
                // A caller replaced the material; adopt it on the next frame.
                if derived.cast_shadows {
                    commands
                        .entity(entity)
                        .remove::<bevy::light::NotShadowCaster>();
                }
                commands.entity(entity).remove::<EmissionMaterial>();
                continue;
            }
            if !emitter.is_changed() {
                continue;
            }
        }
        let original = derived.map_or(&material.0, |d| &d.original);
        let Some(mut next) = assets.get(original).cloned() else {
            continue;
        };
        let rgb = emitter.color.map(|v| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }
        });
        let c = Color::srgb(rgb[0], rgb[1], rgb[2]).to_linear();
        let strength = emitter.power().sqrt() * 0.35 * emitter.glow.clamp(0.0, 10.0);
        next.emissive =
            LinearRgba::new(c.red * strength, c.green * strength, c.blue * strength, 1.0);
        if let Some(derived) = derived {
            if let Some(mut asset) = assets.get_mut(&derived.derived) {
                *asset = next;
            }
        } else {
            let original = material.0.clone();
            material.0 = assets.add(next);
            commands.entity(entity).insert((
                bevy::light::NotShadowCaster,
                EmissionMaterial {
                    original,
                    derived: material.0.clone(),
                    cast_shadows: not_caster.is_none(),
                },
            ));
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct LightingStats {
    pub candidates: usize,
    pub admitted: usize,
    pub shadowed: usize,
    pub suppressed: usize,
}

#[derive(Component)]
pub(crate) struct Sun;

fn setup(mut commands: Commands) {
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.84, 0.89, 1.0),
        brightness: if validation_dark() { 0.0 } else { 120.0 },
        ..default()
    });
    commands.insert_resource(bevy::light::PointLightShadowMap { size: 1024 });
    commands.insert_resource(bevy::light::DirectionalLightShadowMap { size: 1024 });
    commands.spawn((
        Name::new("World sun"),
        Sun,
        DirectionalLight {
            color: Color::srgb(1.0, 0.94, 0.84),
            illuminance: if validation_dark() { 0.0 } else { 11_000.0 },
            shadow_maps_enabled: true,
            ..default()
        },
        bevy::light::CascadeShadowConfigBuilder {
            num_cascades: 2,
            first_cascade_far_bound: 12.0,
            maximum_distance: 48.0,
            ..default()
        }
        .build(),
        // The hand pass samples the world's lighting bind group after the
        // world pass, so it must not allocate or overwrite sun cascades.
        RenderLayers::from_layers(&[0, 1]),
        Transform::default().looking_to(Vec3::new(0.55, -0.78, -0.42), Vec3::Y),
    ));
}

fn reconcile(
    mut commands: Commands,
    sources: Query<(Entity, &LightEmitter), Without<EmitterChild>>,
    owners: Query<&EmitterChild, With<LightEmitter>>,
    children: Query<(Entity, &EmittedLight)>,
    mut removed: RemovedComponents<LightEmitter>,
) {
    for entity in removed.read() {
        if owners.get(entity).is_err()
            && let Ok(mut owner) = commands.get_entity(entity)
        {
            owner.remove::<EmitterChild>();
        }
    }
    for (entity, owner) in &children {
        if !owners.get(owner.0).is_ok_and(|child| child.0 == entity) {
            commands.entity(entity).despawn();
        }
    }
    for (entity, emitter) in &sources {
        let child = commands
            .spawn((
                Name::new("Item emission"),
                EmittedLight(entity),
                PointLight {
                    intensity: 0.0,
                    ..default()
                },
                Transform::from_translation(Vec3::from_array(emitter.offset)),
                // The hand pass samples the world's lighting bind group after the
                // world pass, so it must not allocate or overwrite sun cascades.
                RenderLayers::from_layers(&[0, 1]),
                ChildOf(entity),
            ))
            .id();
        commands.entity(entity).insert(EmitterChild(child));
    }
}

// Sort only the bounded streamed emitter population; clustering still belongs to Bevy.
// Existing shadow owners get a score advantage, avoiding churn at equal distances.
#[allow(clippy::too_many_arguments)] // Shared render policy uses independent ECS resources.
fn update(
    time: Res<Time>,
    view: Res<CameraView>,
    settings: Res<GraphicsSettings>,
    mut map: ResMut<bevy::light::PointLightShadowMap>,
    mut stats: ResMut<LightingStats>,
    sources: Query<(
        &LightEmitter,
        &GlobalTransform,
        Option<&InheritedVisibility>,
    )>,
    mut lights: Query<(Entity, &EmittedLight, &mut PointLight, &mut Visibility)>,
    mut candidates: Local<Vec<(Entity, f32, bool)>>,
    mut retiring: Local<Vec<Entity>>,
) {
    let shadows_enabled = settings.shadow_distance > 0.0;
    let (cap, shadow_cap, size) = match settings.shadow_quality {
        super::graphics::Quality::Low => (64, 1, 512),
        super::graphics::Quality::Medium => (128, 2, 1024),
        super::graphics::Quality::High => (256, 4, 1024),
    };
    if map.size != size {
        map.size = size;
    }
    candidates.clear();
    for (entity, owner, light, _) in &lights {
        let Ok((source, transform, visibility)) = sources.get(owner.0) else {
            continue;
        };
        if source.power() <= 0.0 || visibility.is_some_and(|v| !v.get()) {
            continue;
        }
        let position = transform.transform_point(Vec3::from_array(source.offset));
        let distance = position.distance(view.position);
        if distance > settings.render_distance + source.reach() {
            continue;
        }
        let score = source.power() / (1.0 + distance * distance)
            * if source.priority.is_finite() {
                source.priority.clamp(0.01, 100.0)
            } else {
                1.0
            }
            * if light.shadow_maps_enabled { 1.4 } else { 1.0 };
        candidates.push((
            entity,
            score,
            shadows_enabled && source.shadows == EmissionShadows::Required,
        ));
    }
    candidates.sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    stats.candidates = candidates.len();
    let mut shadows = 0;
    candidates.retain(|entry| {
        if entry.2 {
            shadows += 1;
            shadows <= shadow_cap
        } else {
            true
        }
    });
    candidates.truncate(cap);
    // An evicted source keeps its shadow while fading out. New shadow owners
    // wait for those slots, so transitions never exceed the global map budget.
    retiring.clear();
    for (entity, owner, light, _) in &lights {
        if shadows_enabled
            && light.shadow_maps_enabled
            && light.intensity > 1.0
            && !candidates.iter().any(|c| c.0 == entity)
            && sources
                .get(owner.0)
                .is_ok_and(|(s, _, v)| s.power() > 0.0 && !v.is_some_and(|v| !v.get()))
            && retiring.len() < shadow_cap
        {
            retiring.push(entity);
        }
    }
    let mut available = shadow_cap - retiring.len();
    candidates.retain(|entry| {
        if !entry.2 {
            return true;
        }
        if available == 0 {
            return false;
        }
        available -= 1;
        true
    });
    candidates.truncate(cap - retiring.len());
    stats.admitted = candidates.len() + retiring.len();
    stats.shadowed = candidates.iter().filter(|entry| entry.2).count() + retiring.len();
    stats.suppressed = stats.candidates.saturating_sub(stats.admitted);
    let blend = 1.0 - (-time.delta_secs().min(0.1) * 12.0).exp();
    for (entity, owner, mut light, mut visibility) in &mut lights {
        let Ok((source, _, _)) = sources.get(owner.0) else {
            continue;
        };
        let fading = retiring.contains(&entity);
        let selected = candidates.iter().any(|entry| entry.0 == entity);
        let admitted = selected || fading;
        let shadowed = shadows_enabled && admitted && source.shadows == EmissionShadows::Required;
        visibility.set_if_neq(if admitted {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        let power = if selected { source.power() } else { 0.0 };
        // Retiring lights continue sampling their shadow until their energy is negligible.
        let intensity = if !admitted || source.power() == 0.0 {
            0.0
        } else {
            light.intensity.lerp(power, blend)
        };
        let c = source.color.map(|v| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }
        });
        let next = PointLight {
            color: Color::srgb(c[0], c[1], c[2]),
            intensity,
            range: source.reach(),
            radius: if source.radius.is_finite() {
                source.radius.clamp(0.0, 2.0)
            } else {
                0.0
            },
            shadow_maps_enabled: shadowed,
            shadow_depth_bias: 0.02,
            shadow_normal_bias: 0.4,
            shadow_map_near_z: 0.03,
            ..default()
        };
        if light.intensity != next.intensity
            || light.color != next.color
            || light.range != next.range
            || light.radius != next.radius
            || light.shadow_maps_enabled != next.shadow_maps_enabled
        {
            *light = next;
        }
    }
}

pub(crate) fn validation_dark() -> bool {
    static DARK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *DARK.get_or_init(|| std::env::var("HITHER_LIGHTING_TEST").is_ok_and(|v| v == "dark"))
}

/// Reproducible authoring/shadow fixture; opt-in and absent from normal worlds.
fn validation_scene(
    mut commands: Commands,
    view: Res<CameraView>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(mode) = std::env::var("HITHER_LIGHTING_TEST") else {
        return;
    };
    let forward = view.forward.with_y(0.0).normalize_or(-Vec3::Z);
    let center = view.position + forward * 4.0;
    let ground = crate::world::terrain::height(center.xz());
    let origin = Vec3::new(center.x, ground, center.z);
    let white = materials.add(StandardMaterial {
        base_color: Color::srgb(0.65, 0.65, 0.65),
        perceptual_roughness: 0.7,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.25, 2.2, 1.7))),
        MeshMaterial3d(white.clone()),
        Transform::from_translation(origin + Vec3::Y * 1.1),
    ));
    let count = mode.parse::<usize>().unwrap_or(1).clamp(1, 512);
    for i in 0..count {
        commands.spawn((
            Name::new("Lighting validation emitter"),
            Mesh3d(meshes.add(Sphere::new(0.10).mesh().ico(2).unwrap())),
            MeshMaterial3d(white.clone()),
            LightEmitter {
                color: [1.0, 0.24, 0.045],
                range: 8.0,
                ..LightEmitter::new(12000.0)
            },
            Transform::from_translation(
                origin + Vec3::new(-1.1 + (i % 16) as f32 * 0.2, 0.9, (i / 16) as f32 * 0.2),
            ),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn policy_app() -> App {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(16));
        app.insert_resource(time)
            .insert_resource(crate::player::camera::CameraRig::default().view())
            .insert_resource(GraphicsSettings::default())
            .init_resource::<bevy::light::PointLightShadowMap>()
            .init_resource::<LightingStats>()
            .add_systems(Update, (reconcile, update).chain());
        app
    }
    #[test]
    fn zero_shadow_distance_keeps_lights_on_without_shadow_maps() {
        let mut app = policy_app();
        app.world_mut().spawn((
            LightEmitter::new(800.0),
            GlobalTransform::default(),
            InheritedVisibility::VISIBLE,
        ));
        for distance in [24.0, 0.0, 24.0] {
            app.world_mut()
                .resource_mut::<GraphicsSettings>()
                .shadow_distance = distance;
            app.update();
            app.update();
            let mut query = app.world_mut().query::<(&PointLight, &Visibility)>();
            let (light, visibility) = query.single(app.world()).unwrap();
            assert_eq!(light.shadow_maps_enabled, distance > 0.0);
            assert!(light.intensity > 0.0);
            assert_eq!(*visibility, Visibility::Inherited);
            assert_eq!(
                app.world().resource::<LightingStats>().shadowed,
                usize::from(distance > 0.0)
            );
        }
    }
    #[test]
    fn shadow_budget_is_global_and_suppressed_lights_leave_clusters() {
        let mut app = policy_app();
        for i in 0..12 {
            app.world_mut().spawn((
                LightEmitter::new(800.0),
                GlobalTransform::from_translation(Vec3::new(i as f32, 0.0, 0.0)),
                InheritedVisibility::VISIBLE,
            ));
        }
        for _ in 0..4 {
            app.update();
        }
        let mut query = app.world_mut().query::<(&PointLight, &Visibility)>();
        let lights: Vec<_> = query.iter(app.world()).collect();
        assert_eq!(
            lights.iter().filter(|(p, _)| p.shadow_maps_enabled).count(),
            2
        );
        assert!(lights.iter().all(
            |(p, v)| p.shadow_maps_enabled || (p.intensity == 0.0 && **v == Visibility::Hidden)
        ));
        assert_eq!(app.world().resource::<LightingStats>().suppressed, 10);
    }
    #[test]
    fn disabling_an_item_immediately_removes_illumination() {
        let mut app = policy_app();
        let item = app
            .world_mut()
            .spawn((LightEmitter::new(800.0), InheritedVisibility::VISIBLE))
            .id();
        app.update();
        app.update();
        let child = app.world().get::<EmitterChild>(item).unwrap().0;
        assert!(app.world().get::<PointLight>(child).unwrap().intensity > 0.0);
        app.world_mut()
            .get_mut::<LightEmitter>(item)
            .unwrap()
            .enabled = false;
        app.update();
        let light = app.world().get::<PointLight>(child).unwrap();
        assert_eq!(light.intensity, 0.0);
        assert!(!light.shadow_maps_enabled);
    }
    #[test]
    fn glow_does_not_modify_shared_material_and_removal_restores_it() {
        let mut app = App::new();
        app.init_resource::<Assets<StandardMaterial>>()
            .add_systems(Update, sync_glow);
        let original = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let item = app
            .world_mut()
            .spawn((LightEmitter::new(800.0), MeshMaterial3d(original.clone())))
            .id();
        app.update();
        let derived = app
            .world()
            .get::<MeshMaterial3d<StandardMaterial>>(item)
            .unwrap()
            .0
            .clone();
        assert_ne!(original, derived);
        assert!(
            app.world()
                .get::<bevy::light::NotShadowCaster>(item)
                .is_some()
        );
        let assets = app.world().resource::<Assets<StandardMaterial>>();
        assert_eq!(assets.get(&original).unwrap().emissive, LinearRgba::BLACK);
        assert!(assets.get(&derived).unwrap().emissive.red > 1.0);
        app.world_mut().entity_mut(item).remove::<LightEmitter>();
        app.update();
        assert!(
            app.world()
                .get::<bevy::light::NotShadowCaster>(item)
                .is_none()
        );
        assert_eq!(
            app.world()
                .get::<MeshMaterial3d<StandardMaterial>>(item)
                .unwrap()
                .0,
            original
        );
    }
    #[test]
    fn serialized_items_receive_compatible_defaults() {
        let emitter: LightEmitter = serde_json::from_str(r#"{"lumens":1600}"#).unwrap();
        assert_eq!(emitter.lumens, 1600.0);
        assert!(emitter.enabled);
        assert_eq!(emitter.shadows, EmissionShadows::Required);
    }
    #[test]
    fn photometric_values_are_finite_and_bounded() {
        for power in [f32::NAN, f32::INFINITY, -1.0, 0.0, 800.0, 1e30] {
            let emitter = LightEmitter::new(power);
            assert!(emitter.power().is_finite());
            assert!((0.25..=32.0).contains(&emitter.reach()));
        }
        assert!(LightEmitter::new(1600.0).reach() > LightEmitter::new(800.0).reach());
    }
    #[test]
    fn removal_cleans_up_owned_light_without_touching_other_items() {
        let mut app = App::new();
        app.add_systems(Update, reconcile);
        let a = app
            .world_mut()
            .spawn((LightEmitter::new(800.0), Transform::default()))
            .id();
        let b = app
            .world_mut()
            .spawn((LightEmitter::new(400.0), Transform::default()))
            .id();
        app.update();
        let light_a = app.world().get::<EmitterChild>(a).unwrap().0;
        let light_b = app.world().get::<EmitterChild>(b).unwrap().0;
        app.update();
        assert_eq!(app.world().get::<EmitterChild>(a).unwrap().0, light_a);
        app.world_mut().entity_mut(a).remove::<LightEmitter>();
        app.update();
        assert!(app.world().get_entity(light_a).is_err());
        assert!(app.world().get_entity(light_b).is_ok());
    }
}

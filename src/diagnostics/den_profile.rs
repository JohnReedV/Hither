//! Independent, opt-in den shadow attribution. Never active in normal play.
use bevy::{
    pbr::{LightEntity, Shadow},
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems, render_phase::ViewBinnedRenderPhases, view::ExtractedView,
    },
};

pub(crate) fn plugin(app: &mut App) {
    if std::env::var_os("HITHER_PROFILE_FOREST").is_none() {
        return;
    }
    app.add_systems(
        PostUpdate,
        count_casters.after(bevy::light::check_point_light_mesh_visibility),
    );
    if std::env::var_os("HITHER_PROFILE_FREEZE_POINT_MAPS").is_some()
        && let Some(render) = app.get_sub_app_mut(RenderApp)
    {
        render.add_systems(
            Render,
            freeze_maps
                .after(RenderSystems::PrepareBindGroups)
                .before(RenderSystems::Render),
        );
    }
    if std::env::var_os("HITHER_PROFILE_NO_PROCEDURAL_CASTERS").is_some() {
        app.add_systems(Update, omit_procedural_casters);
    }
}

fn count_casters(
    time: Res<Time<Real>>,
    mut done: Local<bool>,
    lights: Query<(
        &PointLight,
        &bevy::camera::visibility::CubemapVisibleEntities,
    )>,
    instances: Query<(
        &Mesh3d,
        Has<MeshMaterial3d<crate::rendering::sdf::SdfMaterial>>,
    )>,
    meshes: Res<Assets<Mesh>>,
) {
    if *done || time.elapsed_secs() < 55. {
        return;
    }
    *done = true;
    let mut faces = 0;
    let mut instances_count = 0;
    let mut triangles = [0; 2];
    let mut unavailable = 0;
    for (light, visible) in &lights {
        if !light.shadow_maps_enabled || light.intensity <= 0. {
            continue;
        }
        for face in visible.iter() {
            faces += 1;
            for entity in &face.entities {
                if let Ok((mesh, procedural)) = instances.get(*entity)
                    && let Some(mesh) = meshes.get(mesh)
                {
                    instances_count += 1;
                    if let Ok(indices) = mesh.try_indices() {
                        triangles[usize::from(procedural)] += indices.len() / 3;
                    } else if let Ok(vertices) = mesh.try_attribute(Mesh::ATTRIBUTE_POSITION) {
                        triangles[usize::from(procedural)] += vertices.len() / 3;
                    } else {
                        unavailable += 1;
                    }
                }
            }
        }
    }
    println!(
        "PROFILE point casters: faces={faces} instances={instances_count} native_triangles={} procedural_triangles={} uncounted_gpu_meshes={unavailable}",
        triangles[0], triangles[1]
    );
}

// Keep light admission, clustering and shadow sampling unchanged. Removing the
// phase skips its clear and draw, retaining the previous map for attribution.
// This deliberately freezes dynamic shadows and is NOT a shipping cache.
fn freeze_maps(
    mut start: Local<Option<std::time::Instant>>,
    mut reported: Local<bool>,
    views: Query<(&LightEntity, &ExtractedView)>,
    mut phases: ResMut<ViewBinnedRenderPhases<Shadow>>,
) {
    if start
        .get_or_insert_with(std::time::Instant::now)
        .elapsed()
        .as_secs_f32()
        < 45.
    {
        return;
    }
    let mut count = 0;
    for (light, view) in &views {
        if matches!(light, LightEntity::Point { .. }) {
            phases.remove(&view.retained_view_entity);
            count += 1;
        }
    }
    if !*reported {
        println!("PROFILE point map draws frozen: {count} faces; sampling unchanged");
        *reported = true;
    }
}

fn omit_procedural_casters(
    mut commands: Commands,
    time: Res<Time<Real>>,
    meshes: Query<
        Entity,
        (
            With<MeshMaterial3d<crate::rendering::sdf::SdfMaterial>>,
            Without<bevy::light::NotShadowCaster>,
        ),
    >,
) {
    if time.elapsed_secs() < 45. {
        return;
    }
    for entity in &meshes {
        commands.entity(entity).insert(bevy::light::NotShadowCaster);
    }
}

#[derive(Default)]
struct Operation {
    calls: usize,
    total: std::time::Duration,
    longest: std::time::Duration,
}
/// Timings are collected during warmup, before the clean FPS windows.
pub(crate) struct ResidentProfile {
    pub legacy_recovery: bool,
    enabled: bool,
    collecting: bool,
    reported: bool,
    operations: [Operation; 3],
    unsupported: usize,
}
impl Default for ResidentProfile {
    fn default() -> Self {
        let profiling = std::env::var_os("HITHER_PROFILE_FOREST").is_some();
        Self {
            legacy_recovery: profiling
                && std::env::var_os("HITHER_PROFILE_GOBLIN_LEGACY_RECOVERY").is_some(),
            enabled: profiling && std::env::var_os("HITHER_PROFILE_GOBLIN_TIMINGS").is_some(),
            collecting: false,
            reported: false,
            operations: default(),
            unsupported: 0,
        }
    }
}
impl ResidentProfile {
    pub fn begin(&mut self, elapsed: f32) {
        self.collecting = self.enabled && (45.0..60.0).contains(&elapsed);
        if self.enabled && elapsed >= 60. && !self.reported {
            self.reported = true;
            for (label, op) in ["settle", "steer", "walk"]
                .into_iter()
                .zip(&self.operations)
            {
                println!(
                    "PROFILE goblin {label}: calls={} total_ms={:.3} max_ms={:.3}",
                    op.calls,
                    op.total.as_secs_f64() * 1000.,
                    op.longest.as_secs_f64() * 1000.
                );
            }
            println!("PROFILE goblin unsupported_settles={}", self.unsupported);
        }
    }
    pub fn start(&self) -> Option<std::time::Instant> {
        self.collecting.then(std::time::Instant::now)
    }
    pub fn finish(&mut self, operation: usize, start: Option<std::time::Instant>) {
        if let Some(start) = start {
            let elapsed = start.elapsed();
            let op = &mut self.operations[operation];
            op.calls += 1;
            op.total += elapsed;
            op.longest = op.longest.max(elapsed);
        }
    }
    pub fn supported(&mut self, supported: bool) {
        if self.collecting && !supported {
            self.unsupported += 1;
        }
    }
}

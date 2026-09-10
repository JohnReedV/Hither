//! Measured downstream mesh/command costs feed the next frame's admission.
//! These are estimates, not a hard bound: individual engine operations are atomic.
use bevy::{
    prelude::*,
    render::{
        ExtractSchedule, Render, RenderApp, RenderSystems,
        mesh::{RenderMesh, allocator::allocate_and_free_meshes},
        render_asset::{AssetExtractionSystems, ExtractedAssets, prepare_assets},
    },
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};

#[derive(Resource, Clone)]
pub(super) struct Feedback {
    mesh_us_per_mib: Arc<AtomicUsize>,
    command_us: Arc<AtomicUsize>,
    extract_us_per_mib: Arc<AtomicUsize>,
}
impl Default for Feedback {
    fn default() -> Self {
        Self {
            mesh_us_per_mib: Arc::new(AtomicUsize::new(250)),
            command_us: Arc::new(AtomicUsize::new(4)),
            extract_us_per_mib: Arc::new(AtomicUsize::new(50)),
        }
    }
}
impl Feedback {
    pub(super) fn estimate(&self, bytes: usize, entities: usize) -> usize {
        bytes.saturating_mul(
            self.mesh_us_per_mib.load(Ordering::Relaxed)
                + self.extract_us_per_mib.load(Ordering::Relaxed),
        ) / (1024 * 1024)
            + entities.saturating_mul(self.command_us.load(Ordering::Relaxed))
    }
}
fn observe(value: &AtomicUsize, sample: usize, minimum: usize, maximum: usize) {
    let sample = sample.clamp(minimum, maximum);
    let _ = value.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
        Some((old * 3 + sample).div_ceil(4))
    });
}
pub(super) fn install(app: &mut App, feedback: Feedback) {
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render
            .insert_resource(feedback)
            .init_resource::<MeshTiming>()
            .init_resource::<ExtractionTiming>()
            .add_systems(
                ExtractSchedule,
                extraction_start.before(AssetExtractionSystems),
            )
            .add_systems(
                ExtractSchedule,
                extraction_end.after(AssetExtractionSystems),
            )
            .add_systems(
                Render,
                mesh_start
                    .in_set(RenderSystems::PrepareAssets)
                    .before(allocate_and_free_meshes),
            )
            .add_systems(
                Render,
                mesh_end
                    .in_set(RenderSystems::PrepareAssets)
                    .after(prepare_assets::<RenderMesh>),
            );
    }
}
#[derive(Resource, Default)]
struct ExtractionTiming(Option<Instant>);
fn extraction_start(mut timing: ResMut<ExtractionTiming>) {
    timing.0 = Some(Instant::now());
}
fn extraction_end(
    mut timing: ResMut<ExtractionTiming>,
    meshes: Res<ExtractedAssets<RenderMesh>>,
    feedback: Res<Feedback>,
) {
    if let Some(start) = timing.0.take() {
        let us = start.elapsed().as_micros() as usize;
        let bytes: usize = meshes
            .extracted
            .iter()
            .map(|(_, mesh)| super::mesh_bytes(mesh))
            .sum();
        if bytes >= 64 * 1024 {
            // The public extraction set includes other asset classes. Charge its
            // wall time conservatively; this is admission feedback, not exclusive
            // mesh attribution or a GPU duration.
            observe(
                &feedback.extract_us_per_mib,
                us.saturating_mul(1024 * 1024) / bytes,
                10,
                4000,
            );
        }
    }
}
#[derive(Resource, Default)]
struct MeshTiming {
    start: Option<Instant>,
    bytes: usize,
}
fn mesh_start(mut timing: ResMut<MeshTiming>, meshes: Res<ExtractedAssets<RenderMesh>>) {
    timing.bytes = meshes
        .extracted
        .iter()
        .map(|(_, mesh)| super::mesh_bytes(mesh))
        .sum();
    timing.start = Some(Instant::now());
}
fn mesh_end(mut timing: ResMut<MeshTiming>, feedback: Res<Feedback>) {
    if let Some(start) = timing.start.take() {
        // Tiny batches are dominated by scheduling noise. Include allocation,
        // staging writes and render mesh preparation, without waiting on the GPU.
        if timing.bytes >= 64 * 1024 {
            let us = start.elapsed().as_micros() as usize;
            observe(
                &feedback.mesh_us_per_mib,
                us.saturating_mul(1024 * 1024) / timing.bytes,
                50,
                16_000,
            );
        }
    }
}
pub(crate) struct CommandTiming {
    start: Arc<Mutex<Option<Instant>>>,
    cost: Arc<AtomicUsize>,
}
impl super::Coordinator {
    pub(crate) fn time_commands(&self, commands: &mut Commands) -> CommandTiming {
        let start = Arc::new(Mutex::new(None));
        let marker = start.clone();
        commands.queue(move |_: &mut World| *marker.lock().unwrap() = Some(Instant::now()));
        CommandTiming {
            start,
            cost: self.feedback.command_us.clone(),
        }
    }
}
impl CommandTiming {
    pub(crate) fn finish(self, commands: &mut Commands, count: usize) {
        commands.queue(move |_: &mut World| {
            if let Some(start) = self.start.lock().unwrap().take()
                && count > 0
            {
                observe(
                    &self.cost,
                    (start.elapsed().as_micros() as usize).div_ceil(count),
                    2,
                    200,
                );
            }
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_large_waiting_upload_gets_admission_before_small_early_uploads() {
        use super::super::{Coordinator, Layer};
        let mut c = Coordinator::default();
        c.feedback.mesh_us_per_mib.store(4000, Ordering::Relaxed);
        assert!(c.install(Layer::Terrain, 4, 256 * 1024).is_some());
        assert!(c.install(Layer::Grass, 4, 512 * 1024).is_none());
        c.begin();
        assert!(c.install(Layer::Terrain, 4, 256 * 1024).is_none());
        assert!(c.install(Layer::Terrain, 1, 0).is_some());
        assert!(c.install(Layer::Grass, 4, 512 * 1024).is_some());
        c.begin();
        assert!(c.install(Layer::Terrain, 4, 256 * 1024).is_some());
    }
    #[test]
    fn downstream_cost_changes_admission_without_starving_atomic_jobs() {
        let mut c = super::super::Coordinator::default();
        c.feedback.mesh_us_per_mib.store(4000, Ordering::Relaxed);
        assert!(
            c.install(super::super::Layer::Terrain, 4, 512 * 1024)
                .is_some()
        );
        assert!(
            c.install(super::super::Layer::Grass, 4, 512 * 1024)
                .is_none()
        );
        c.begin();
        assert!(
            c.install(super::super::Layer::Grass, 4, 512 * 1024)
                .is_some()
        );
    }
    #[test]
    fn measurements_are_smoothed_and_bounded() {
        let c = AtomicUsize::new(250);
        observe(&c, 100_000, 50, 16_000);
        assert_eq!(c.load(Ordering::Relaxed), 4188);
        observe(&c, 0, 50, 16_000);
        assert_eq!(c.load(Ordering::Relaxed), 3154);
    }
}

//! Opt-in stage measurements. Deferred timing covers the explicit final flush;
//! Bevy may also apply commands earlier to satisfy inter-system dependencies.
use bevy::{ecs::schedule::ApplyDeferred, prelude::*};
use std::time::Instant;

#[derive(Resource, Default)]
struct Clock {
    start: Option<Instant>,
    samples: Vec<f64>,
}
fn begin(mut clock: ResMut<Clock>) {
    clock.start = Some(Instant::now());
}
fn finish(mut clock: ResMut<Clock>) {
    if let Some(start) = clock.start.take() {
        clock.samples.push(start.elapsed().as_secs_f64() * 1000.0);
        if clock.samples.len() == 240 {
            clock.samples.sort_by(f64::total_cmp);
            info!(
                "STREAMING_DEFERRED_FLUSH median_ms={:.4} p95_ms={:.4}",
                clock.samples[120], clock.samples[228]
            );
            clock.samples.clear();
        }
    }
}
#[derive(Resource, Default)]
struct RenderClock {
    assets: Option<Instant>,
    meshes: Option<Instant>,
    prepare: Option<Instant>,
    samples: Vec<(f64, f64, f64)>,
    asset_ms: f64,
    mesh_ms: f64,
}
fn asset_begin(mut clock: ResMut<RenderClock>) {
    clock.assets = Some(Instant::now());
}
fn asset_end(mut clock: ResMut<RenderClock>) {
    let now = Instant::now();
    clock.asset_ms = now
        .duration_since(clock.assets.take().unwrap())
        .as_secs_f64()
        * 1000.0;
    clock.meshes = Some(now);
}
fn prepare_begin(mut clock: ResMut<RenderClock>) {
    let now = Instant::now();
    clock.mesh_ms = now
        .duration_since(clock.meshes.take().unwrap())
        .as_secs_f64()
        * 1000.0;
    clock.prepare = Some(now);
}
fn prepare_end(mut clock: ResMut<RenderClock>) {
    let value = (
        clock.asset_ms,
        clock.mesh_ms,
        clock.prepare.take().unwrap().elapsed().as_secs_f64() * 1000.0,
    );
    clock.samples.push(value);
    if clock.samples.len() == 240 {
        let mut assets: Vec<_> = clock.samples.iter().map(|x| x.0).collect();
        let mut meshes: Vec<_> = clock.samples.iter().map(|x| x.1).collect();
        let mut prepare: Vec<_> = clock.samples.iter().map(|x| x.2).collect();
        assets.sort_by(f64::total_cmp);
        meshes.sort_by(f64::total_cmp);
        prepare.sort_by(f64::total_cmp);
        info!(
            "RENDER_PREPARATION asset_median_ms={:.4} asset_p95_ms={:.4} mesh_median_ms={:.4} mesh_p95_ms={:.4} prepare_median_ms={:.4} prepare_p95_ms={:.4}",
            assets[120], assets[228], meshes[120], meshes[228], prepare[120], prepare[228]
        );
        clock.samples.clear();
    }
}
pub(crate) fn plugin(app: &mut App) {
    if std::env::var_os("HITHER_PROFILE_STREAMING").is_none() {
        return;
    }
    use crate::world::streaming::Layer;
    let mut start = begin.into_configs();
    for layer in [
        Layer::Terrain,
        Layer::Grass,
        Layer::Citrus,
        Layer::Conifers,
        Layer::Oaks,
        Layer::Alpine,
        Layer::Rocks,
        Layer::Understory,
        Layer::Logs,
        Layer::Dens,
        Layer::Goblins,
        Layer::Orcs,
        Layer::Contacts,
    ] {
        start = start.after_ignore_deferred(layer);
    }
    app.init_resource::<Clock>()
        .add_systems(Update, (start, ApplyDeferred, finish).chain());
    if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
        install_render_probes(render);
    }
}
fn install_render_probes(render: &mut bevy::app::SubApp) {
    use bevy::render::{Render, RenderSystems};
    render.init_resource::<RenderClock>().add_systems(
        Render,
        (
            asset_begin
                .after(RenderSystems::ExtractCommands)
                .before(RenderSystems::PrepareAssets),
            asset_end
                .after(RenderSystems::PrepareAssets)
                .before(RenderSystems::PrepareMeshes),
            prepare_begin
                .after(RenderSystems::PrepareMeshes)
                .before(RenderSystems::CreateViews),
            prepare_end
                .after(RenderSystems::Prepare)
                .before(RenderSystems::Render),
        ),
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn render_probes_cover_meshes_and_view_creation_without_gaps() {
        use bevy::render::{Render, RenderSystems};
        let mut render = bevy::app::SubApp::new();
        render.add_schedule(Render::base_schedule());
        install_render_probes(&mut render);
        render.add_systems(
            Render,
            (
                (|clock: Res<RenderClock>| {
                    assert!(clock.assets.is_some());
                    assert!(clock.meshes.is_none());
                })
                .in_set(RenderSystems::PrepareAssets),
                (|clock: Res<RenderClock>| {
                    assert!(clock.assets.is_none());
                    assert!(clock.meshes.is_some());
                    assert!(clock.prepare.is_none());
                })
                .in_set(RenderSystems::PrepareMeshes),
                (|clock: Res<RenderClock>| {
                    assert!(clock.meshes.is_none());
                    assert!(clock.prepare.is_some());
                })
                .in_set(RenderSystems::CreateViews),
            ),
        );
        render.world_mut().run_schedule(Render);
        let clock = render.world().resource::<RenderClock>();
        assert_eq!(clock.samples.len(), 1);
        assert!(clock.prepare.is_none());
        let (assets, meshes, prepare) = clock.samples[0];
        assert!(assets >= 0.0 && meshes >= 0.0 && prepare >= 0.0);
    }
}

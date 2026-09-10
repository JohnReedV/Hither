//! Offscreen profiles have no swapchain acquisition to bound GPU run-ahead.
//! Keep two render submissions in flight so frame times measure sustained
//! rendering rather than an arbitrarily growing queue of unfinished frames.
use bevy::{
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        renderer::{RenderDevice, RenderQueue},
    },
};
use std::collections::VecDeque;

#[derive(Resource, Default)]
struct PendingFrames(VecDeque<wgpu::SubmissionIndex>);

pub(crate) fn install(app: &mut App) {
    let render = app
        .get_sub_app_mut(RenderApp)
        .expect("offscreen profiling requires the render app");
    render.init_resource::<PendingFrames>().add_systems(
        Render,
        finish_frame
            .after(RenderSystems::Render)
            .before(RenderSystems::Cleanup),
    );
    println!("PROFILE gpu_frames_in_flight=2");
}

fn finish_frame(
    mut pending: ResMut<PendingFrames>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    // This empty submission marks completion of all preceding frame commands,
    // including the render graph, screenshot/readback copies and cache bakes.
    pending.0.push_back(queue.submit([]));
    if pending.0.len() >= 2 {
        let index = pending.0.pop_front().unwrap();
        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(index),
                timeout: None,
            })
            .expect("offscreen GPU frame completion failed");
    }
}

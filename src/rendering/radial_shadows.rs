//! One directional-shadow distance policy for standard and custom materials.
use bevy::{prelude::*, shader::Source};
#[derive(Resource)]
struct ShadowLibrary {
    handle: Handle<Shader>,
    installed: bool,
}
pub(crate) fn plugin(app: &mut App) {
    app.add_systems(Startup, setup).add_systems(Update, install);
}
fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(ShadowLibrary {
        handle: assets.load("embedded://bevy_pbr/render/shadows.wgsl"),
        installed: false,
    });
}
fn install(library: Option<ResMut<ShadowLibrary>>, mut shaders: ResMut<Assets<Shader>>) {
    let Some(mut library) = library else {
        return;
    };
    if library.installed {
        return;
    }
    let Some(mut shader) = shaders.get_mut(&library.handle) else {
        return;
    };
    let source = shader.source.as_str();
    let start = source
        .find("fn fetch_directional_shadow(")
        .expect("Bevy directional shadow entry point changed");
    let end = source
        .find("fn cascade_debug_visualization(")
        .expect("Bevy shadow library layout changed");
    // Preserve imports/dependencies/definitions and the original asset identity.
    shader.source = Source::Wgsl(
        format!(
            "{}\n{}\n{}",
            &source[..start],
            include_str!("../../assets/shaders/radial_shadows.wgsl"),
            &source[end..]
        )
        .into(),
    );
    library.installed = true;
}

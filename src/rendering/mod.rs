//! Shared rendering primitives, materials and live quality controls.
pub(crate) mod geometry;
pub(crate) mod graphics;
pub(crate) mod occlusion;
pub(crate) mod scene;
pub(crate) mod sdf;
pub(crate) mod view_distance;

pub(crate) mod mipmaps;

pub(crate) mod plant_lod;
pub(crate) mod surface_cache;

pub(crate) mod radial_shadows;

pub(crate) mod lighting;

mod lighting_environment;

pub(crate) mod hand_lighting;

pub(crate) mod point_shadow_cache;

//! Whole-scene GPU occlusion and deterministic, same-build benchmark modes.
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Legacy,
    Depth,
    Gpu,
}
impl Mode {
    pub(crate) fn depth(self) -> bool {
        self != Self::Legacy
    }
    pub(crate) fn gpu(self) -> bool {
        self == Self::Gpu
    }
}
pub(crate) fn mode() -> Mode {
    static MODE: OnceLock<Mode> = OnceLock::new();
    *MODE.get_or_init(|| {
        if std::env::var_os("HITHER_PROFILE_FOREST").is_some() {
            match std::env::var("HITHER_VISIBILITY_MODE").as_deref() {
                Ok("legacy") => return Mode::Legacy,
                Ok("depth") => return Mode::Depth,
                Ok("gpu") => return Mode::Gpu,
                Ok(value) => panic!("Unknown HITHER_VISIBILITY_MODE: {value}"),
                Err(_) => {}
            }
        }
        Mode::Gpu
    })
}

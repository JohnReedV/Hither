//! Hither executable. Runtime composition lives in `app`.
mod app;
mod diagnostics;
mod player;
mod rendering;
mod ui;
mod world;

fn main() {
    app::run();
}

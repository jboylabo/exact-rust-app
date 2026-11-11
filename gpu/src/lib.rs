//! Exact Rust App's GPU module: one surface, `macaron()`, a purple macaron
//! ray-marched in a fragment shader (`shaders/macaron.wgsl`) and turned by
//! the frame clock. wgpu draws it: WebGPU on the web, Metal on Apple platforms.

#![deny(missing_docs)]

pub mod macaron;
pub use macaron::MacaronSurface;

use exact_gpu::{Registry, Surface};

/// The shaders under `shaders/`, reflected at build (`build.rs`): each one's
/// entry points, bindings and layouts as Rust. The WGSL declares them once.
pub mod shaders {
    include!(concat!(env!("OUT_DIR"), "/shaders.rs"));
}

/// Where this crate's shaders live in the source tree.
#[cfg(not(target_arch = "wasm32"))]
pub fn shader_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("shaders")
}

fn macaron() -> Box<dyn Surface> {
    Box::new(MacaronSurface::new())
}

/// Each surface: its name, its number of inputs, its factory.
pub static REGISTRY: Registry = Registry {
    surfaces: &[("macaron", 0, macaron)],
    shaders: shaders::SHADERS,
};

exact_gpu::module!(REGISTRY);

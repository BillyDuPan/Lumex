//! Lumex WGPU Native Graphics Engine
//!
//! Submodules for WGPU lifecycle context, offscreen targets, uniform buffers,
//! and the multi-pass post-processing pipeline.

pub mod context;
pub mod pipeline;
pub mod targets;
pub mod uniforms;

pub use context::WgpuContext;
pub use pipeline::PostProcessPipeline;
pub use targets::{BlurPyramid, EchoTargets, PostProcessTargets, RenderTarget, BLUR_LEVELS};
pub use uniforms::{FxParams, FxStateMachine, FxUniforms};

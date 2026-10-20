//! Lumex GPU Compute Simulation Subsystems
//!
//! Provides high-performance GPU compute pipelines for:
//! - Floating Balls (5,000 agents with drift, mouse forces, swell, edge wrap/bounce)
//! - Particle Sparks (65,536 particles with drag, gravity, audio burst excitation)
//! - Intelligent Life Boids (10,000 agents with toroidal wrapping, flocking, spore foraging)
//! - GPU Vector Brush Drawing (Analytical segment SDFs, neon glow, and eraser accumulation)

pub mod balls;
pub mod boids;
pub mod drawing;
pub mod sparks;

pub use balls::{Ball, BallsComputePipeline, BallsUniforms, MAX_BALLS};
pub use boids::{Boid, BoidsComputePipeline, BoidsUniforms, Spore, MAX_BOIDS, MAX_SPORES};
pub use drawing::{BrushStyle, DrawUniforms, DrawingPipeline, StrokeSegment, MAX_SEGMENTS};
pub use sparks::{Particle, SparksComputePipeline, SparksUniforms, MAX_SPARKS};

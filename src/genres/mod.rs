// src/genres/mod.rs

// High-level “genre” orchestrations that coordinate multiple systems.
// Each genre is feature-gated so downstream games enable only what they use.

pub mod sdk;
pub use sdk::*;

#[cfg(feature = "genre-idle")]
pub mod idle;

#[cfg(feature = "genre-idle")]
pub use idle::*;

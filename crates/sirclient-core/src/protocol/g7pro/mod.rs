//! GameSir G7 Pro 8K (a.k.a. G7 Pro CE) configuration protocol.

pub mod commands;
pub mod curve;
pub mod keys;
pub mod light;
pub mod profile;
pub mod state;
pub mod xbox;

pub use commands::{Reply, Slot};
pub use light::LightProfile;
pub use profile::Profile;
pub use state::State;

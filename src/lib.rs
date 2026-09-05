pub mod authoring;
pub mod music;
pub mod playback;

// Compatibility facades for existing engine clients.
pub use authoring::library;
pub use music as config;
pub mod engine {
    pub use crate::music::resolve::*;
    pub use crate::music::rhythm::*;
}

pub mod update;

pub mod player;

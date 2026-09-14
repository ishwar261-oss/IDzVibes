//! # idz-audio
//!
//! Ultra-low-latency audio engine for IDzVibes.
//!
//! Features:
//! - Pre-decoded 32-bit floating point PCM audio buffers
//! - Zero-allocation pre-allocated voice pool
//! - Lock-free SPSC event queue between input/UI and audio thread
//! - Persistent audio stream to eliminate hardware wake-up latency
//! - Natural pitch & volume variations
//! - Soft limiter to prevent clipping during ultra-rapid typing
//! - Independent keyboard, mouse, and ambient sound channels

pub mod engine;
pub mod mixer;
pub mod pcm;
pub mod preloader;
pub mod soundpack;
pub mod voice_pool;

pub use engine::{AudioCommand, AudioEngine, AudioHandle, EngineStats};
pub use mixer::{AmbientChannel, Mixer};
pub use pcm::PcmBuffer;
pub use preloader::{load_audio_bytes, load_audio_file};
pub use soundpack::{KeySound, PlaybackMode, SelectionMode, Soundpack, SoundpackMetadata};
pub use voice_pool::{Voice, VoicePool};

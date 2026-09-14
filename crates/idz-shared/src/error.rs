//! Shared error types for IDzVibes.
//!
//! Provides a unified error hierarchy used across all crates.

use thiserror::Error;

/// Top-level error type for IDzVibes operations.
#[derive(Error, Debug)]
pub enum IdzError {
    #[error("Audio engine error: {0}")]
    Audio(#[from] AudioError),

    #[error("Input engine error: {0}")]
    Input(#[from] InputError),

    #[error("Soundpack error: {0}")]
    Soundpack(#[from] SoundpackError),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Errors related to audio engine operations.
#[derive(Error, Debug)]
pub enum AudioError {
    #[error("No audio output device available")]
    NoDevice,

    #[error("Failed to open audio device: {0}")]
    DeviceOpen(String),

    #[error("Audio stream error: {0}")]
    Stream(String),

    #[error("Failed to decode audio file: {0}")]
    Decode(String),

    #[error("Unsupported audio format: {0}")]
    UnsupportedFormat(String),

    #[error("Voice pool exhausted (all {0} voices are active)")]
    VoicePoolExhausted(usize),

    #[error("Sample not found for key: {0}")]
    SampleNotFound(String),

    #[error("Audio device disconnected")]
    DeviceDisconnected,

    #[error("Resampling error: {0}")]
    Resample(String),
}

/// Errors related to input engine operations.
#[derive(Error, Debug)]
pub enum InputError {
    #[error("Failed to register raw input device: {0}")]
    Registration(String),

    #[error("Failed to create input window: {0}")]
    WindowCreation(String),

    #[error("Input thread error: {0}")]
    Thread(String),

    #[error("Windows API error: {0}")]
    WinApi(String),
}

/// Errors related to soundpack operations.
#[derive(Error, Debug)]
pub enum SoundpackError {
    #[error("Invalid soundpack metadata: {0}")]
    InvalidMetadata(String),

    #[error("Soundpack not found: {0}")]
    NotFound(String),

    #[error("Invalid archive: {0}")]
    InvalidArchive(String),

    #[error("Security violation: {0}")]
    Security(String),

    #[error("Unsupported sound format: {0}")]
    UnsupportedFormat(String),
}

/// Errors related to configuration.
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(String),

    #[error("Failed to write config file: {0}")]
    WriteError(String),

    #[error("Invalid configuration: {0}")]
    Invalid(String),

    #[error("Config migration failed from version {from} to {to}: {reason}")]
    MigrationFailed {
        from: u32,
        to: u32,
        reason: String,
    },
}

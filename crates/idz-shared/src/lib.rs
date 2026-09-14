//! # idz-shared
//!
//! Shared types and utilities used across all IDzVibes crates.
//! Contains key codes, error types, and common data structures.

pub mod error;
pub mod key_code;

pub use error::IdzError;
pub use key_code::KeyCode;

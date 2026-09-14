//! # idz-input
//!
//! Windows Raw Input keyboard and mouse engine.
//!
//! Captures hardware-level keyboard make/break codes without polling or UI thread hooks.
//! Runs on a dedicated OS thread with a message-only HWND.

pub mod event;
#[cfg(windows)]
pub mod windows;

pub use event::{InputEvent, InputEventKind};
#[cfg(windows)]
pub use windows::WindowsInputEngine as InputEngine;

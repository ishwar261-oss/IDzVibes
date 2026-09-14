//! Input event definitions for keyboard and mouse triggers.

use idz_shared::KeyCode;
use serde::{Deserialize, Serialize};

/// Type of input action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputEventKind {
    KeyDown,
    KeyUp,
    MouseDown,
    MouseUp,
}

/// A captured hardware input event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputEvent {
    /// Normalized KeyCode.
    pub key: KeyCode,
    /// Type of event (down/up).
    pub kind: InputEventKind,
    /// High-resolution microsecond timestamp when event was captured.
    pub timestamp_us: u64,
    /// Physical device handle (used for multi-keyboard filtering).
    pub device_id: usize,
}

impl InputEvent {
    pub fn is_down(&self) -> bool {
        matches!(self.kind, InputEventKind::KeyDown | InputEventKind::MouseDown)
    }

    pub fn is_mouse(&self) -> bool {
        matches!(self.kind, InputEventKind::MouseDown | InputEventKind::MouseUp)
    }
}

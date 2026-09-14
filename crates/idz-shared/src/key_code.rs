//! Unified key code enumeration for IDzVibes.
//!
//! Maps Windows scan codes and virtual key codes to a platform-independent
//! key representation. This is the canonical key identity used throughout
//! the application for sound mapping, configuration, and display.

use serde::{Deserialize, Serialize};

/// Represents a physical keyboard key, independent of keyboard layout.
///
/// Based on USB HID usage codes and Windows scan codes.
/// Each variant maps to a specific physical key position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyCode {
    // Row 0: Function keys
    Escape,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    PrintScreen,
    ScrollLock,
    Pause,

    // Row 1: Number row
    Backquote,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Digit0,
    Minus,
    Equal,
    Backspace,

    // Row 2: QWERTY row
    Tab,
    KeyQ,
    KeyW,
    KeyE,
    KeyR,
    KeyT,
    KeyY,
    KeyU,
    KeyI,
    KeyO,
    KeyP,
    BracketLeft,
    BracketRight,
    Backslash,

    // Row 3: Home row
    CapsLock,
    KeyA,
    KeyS,
    KeyD,
    KeyF,
    KeyG,
    KeyH,
    KeyJ,
    KeyK,
    KeyL,
    Semicolon,
    Quote,
    Enter,

    // Row 4: Bottom row
    ShiftLeft,
    KeyZ,
    KeyX,
    KeyC,
    KeyV,
    KeyB,
    KeyN,
    KeyM,
    Comma,
    Period,
    Slash,
    ShiftRight,

    // Row 5: Bottom control row
    ControlLeft,
    MetaLeft,
    AltLeft,
    Space,
    AltRight,
    MetaRight,
    ContextMenu,
    ControlRight,

    // Navigation cluster
    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,

    // Arrow keys
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,

    // Numpad
    NumLock,
    NumpadDivide,
    NumpadMultiply,
    NumpadSubtract,
    NumpadAdd,
    NumpadEnter,
    NumpadDecimal,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,

    // Mouse buttons (for mouse sound support)
    MouseLeft,
    MouseRight,
    MouseMiddle,
    MouseWheelUp,
    MouseWheelDown,

    // Catch-all for unmapped keys
    Unknown(u32),
}

impl KeyCode {
    /// Convert a Windows scan code to a KeyCode.
    ///
    /// Uses the standard AT/PS2 scan code set. Extended keys (prefixed with 0xE0)
    /// should have bit 8 set in the scan code value (e.g., Right Ctrl = 0x11D).
    pub fn from_scan_code(scan_code: u32, extended: bool) -> Self {
        let code = if extended {
            scan_code | 0x100
        } else {
            scan_code
        };

        match code {
            0x01 => KeyCode::Escape,
            0x3B => KeyCode::F1,
            0x3C => KeyCode::F2,
            0x3D => KeyCode::F3,
            0x3E => KeyCode::F4,
            0x3F => KeyCode::F5,
            0x40 => KeyCode::F6,
            0x41 => KeyCode::F7,
            0x42 => KeyCode::F8,
            0x43 => KeyCode::F9,
            0x44 => KeyCode::F10,
            0x57 => KeyCode::F11,
            0x58 => KeyCode::F12,
            0x137 => KeyCode::PrintScreen,
            0x46 => KeyCode::ScrollLock,
            0x145 => KeyCode::Pause,

            0x29 => KeyCode::Backquote,
            0x02 => KeyCode::Digit1,
            0x03 => KeyCode::Digit2,
            0x04 => KeyCode::Digit3,
            0x05 => KeyCode::Digit4,
            0x06 => KeyCode::Digit5,
            0x07 => KeyCode::Digit6,
            0x08 => KeyCode::Digit7,
            0x09 => KeyCode::Digit8,
            0x0A => KeyCode::Digit9,
            0x0B => KeyCode::Digit0,
            0x0C => KeyCode::Minus,
            0x0D => KeyCode::Equal,
            0x0E => KeyCode::Backspace,

            0x0F => KeyCode::Tab,
            0x10 => KeyCode::KeyQ,
            0x11 => KeyCode::KeyW,
            0x12 => KeyCode::KeyE,
            0x13 => KeyCode::KeyR,
            0x14 => KeyCode::KeyT,
            0x15 => KeyCode::KeyY,
            0x16 => KeyCode::KeyU,
            0x17 => KeyCode::KeyI,
            0x18 => KeyCode::KeyO,
            0x19 => KeyCode::KeyP,
            0x1A => KeyCode::BracketLeft,
            0x1B => KeyCode::BracketRight,
            0x2B => KeyCode::Backslash,

            0x3A => KeyCode::CapsLock,
            0x1E => KeyCode::KeyA,
            0x1F => KeyCode::KeyS,
            0x20 => KeyCode::KeyD,
            0x21 => KeyCode::KeyF,
            0x22 => KeyCode::KeyG,
            0x23 => KeyCode::KeyH,
            0x24 => KeyCode::KeyJ,
            0x25 => KeyCode::KeyK,
            0x26 => KeyCode::KeyL,
            0x27 => KeyCode::Semicolon,
            0x28 => KeyCode::Quote,
            0x1C => KeyCode::Enter,

            0x2A => KeyCode::ShiftLeft,
            0x2C => KeyCode::KeyZ,
            0x2D => KeyCode::KeyX,
            0x2E => KeyCode::KeyC,
            0x2F => KeyCode::KeyV,
            0x30 => KeyCode::KeyB,
            0x31 => KeyCode::KeyN,
            0x32 => KeyCode::KeyM,
            0x33 => KeyCode::Comma,
            0x34 => KeyCode::Period,
            0x35 => KeyCode::Slash,
            0x36 => KeyCode::ShiftRight,

            0x1D => KeyCode::ControlLeft,
            0x15B => KeyCode::MetaLeft,
            0x38 => KeyCode::AltLeft,
            0x39 => KeyCode::Space,
            0x138 => KeyCode::AltRight,
            0x15C => KeyCode::MetaRight,
            0x15D => KeyCode::ContextMenu,
            0x11D => KeyCode::ControlRight,

            0x152 => KeyCode::Insert,
            0x153 => KeyCode::Delete,
            0x147 => KeyCode::Home,
            0x14F => KeyCode::End,
            0x149 => KeyCode::PageUp,
            0x151 => KeyCode::PageDown,

            0x148 => KeyCode::ArrowUp,
            0x150 => KeyCode::ArrowDown,
            0x14B => KeyCode::ArrowLeft,
            0x14D => KeyCode::ArrowRight,

            0x45 => KeyCode::NumLock,
            0x135 => KeyCode::NumpadDivide,
            0x37 => KeyCode::NumpadMultiply,
            0x4A => KeyCode::NumpadSubtract,
            0x4E => KeyCode::NumpadAdd,
            0x11C => KeyCode::NumpadEnter,
            0x53 => KeyCode::NumpadDecimal,
            0x52 => KeyCode::Numpad0,
            0x4F => KeyCode::Numpad1,
            0x50 => KeyCode::Numpad2,
            0x51 => KeyCode::Numpad3,
            0x4B => KeyCode::Numpad4,
            0x4C => KeyCode::Numpad5,
            0x4D => KeyCode::Numpad6,
            0x47 => KeyCode::Numpad7,
            0x48 => KeyCode::Numpad8,
            0x49 => KeyCode::Numpad9,

            other => KeyCode::Unknown(other),
        }
    }

    /// Get a human-readable display name for this key.
    pub fn display_name(&self) -> &'static str {
        match self {
            KeyCode::Escape => "Esc",
            KeyCode::F1 => "F1",
            KeyCode::F2 => "F2",
            KeyCode::F3 => "F3",
            KeyCode::F4 => "F4",
            KeyCode::F5 => "F5",
            KeyCode::F6 => "F6",
            KeyCode::F7 => "F7",
            KeyCode::F8 => "F8",
            KeyCode::F9 => "F9",
            KeyCode::F10 => "F10",
            KeyCode::F11 => "F11",
            KeyCode::F12 => "F12",
            KeyCode::PrintScreen => "PrtSc",
            KeyCode::ScrollLock => "ScrLk",
            KeyCode::Pause => "Pause",
            KeyCode::Backquote => "`",
            KeyCode::Digit1 => "1",
            KeyCode::Digit2 => "2",
            KeyCode::Digit3 => "3",
            KeyCode::Digit4 => "4",
            KeyCode::Digit5 => "5",
            KeyCode::Digit6 => "6",
            KeyCode::Digit7 => "7",
            KeyCode::Digit8 => "8",
            KeyCode::Digit9 => "9",
            KeyCode::Digit0 => "0",
            KeyCode::Minus => "-",
            KeyCode::Equal => "=",
            KeyCode::Backspace => "Backspace",
            KeyCode::Tab => "Tab",
            KeyCode::KeyQ => "Q",
            KeyCode::KeyW => "W",
            KeyCode::KeyE => "E",
            KeyCode::KeyR => "R",
            KeyCode::KeyT => "T",
            KeyCode::KeyY => "Y",
            KeyCode::KeyU => "U",
            KeyCode::KeyI => "I",
            KeyCode::KeyO => "O",
            KeyCode::KeyP => "P",
            KeyCode::BracketLeft => "[",
            KeyCode::BracketRight => "]",
            KeyCode::Backslash => "\\",
            KeyCode::CapsLock => "Caps",
            KeyCode::KeyA => "A",
            KeyCode::KeyS => "S",
            KeyCode::KeyD => "D",
            KeyCode::KeyF => "F",
            KeyCode::KeyG => "G",
            KeyCode::KeyH => "H",
            KeyCode::KeyJ => "J",
            KeyCode::KeyK => "K",
            KeyCode::KeyL => "L",
            KeyCode::Semicolon => ";",
            KeyCode::Quote => "'",
            KeyCode::Enter => "Enter",
            KeyCode::ShiftLeft => "LShift",
            KeyCode::KeyZ => "Z",
            KeyCode::KeyX => "X",
            KeyCode::KeyC => "C",
            KeyCode::KeyV => "V",
            KeyCode::KeyB => "B",
            KeyCode::KeyN => "N",
            KeyCode::KeyM => "M",
            KeyCode::Comma => ",",
            KeyCode::Period => ".",
            KeyCode::Slash => "/",
            KeyCode::ShiftRight => "RShift",
            KeyCode::ControlLeft => "LCtrl",
            KeyCode::MetaLeft => "Win",
            KeyCode::AltLeft => "LAlt",
            KeyCode::Space => "Space",
            KeyCode::AltRight => "RAlt",
            KeyCode::MetaRight => "RWin",
            KeyCode::ContextMenu => "Menu",
            KeyCode::ControlRight => "RCtrl",
            KeyCode::Insert => "Ins",
            KeyCode::Delete => "Del",
            KeyCode::Home => "Home",
            KeyCode::End => "End",
            KeyCode::PageUp => "PgUp",
            KeyCode::PageDown => "PgDn",
            KeyCode::ArrowUp => "↑",
            KeyCode::ArrowDown => "↓",
            KeyCode::ArrowLeft => "←",
            KeyCode::ArrowRight => "→",
            KeyCode::NumLock => "NumLk",
            KeyCode::NumpadDivide => "Num/",
            KeyCode::NumpadMultiply => "Num*",
            KeyCode::NumpadSubtract => "Num-",
            KeyCode::NumpadAdd => "Num+",
            KeyCode::NumpadEnter => "NumEnter",
            KeyCode::NumpadDecimal => "Num.",
            KeyCode::Numpad0 => "Num0",
            KeyCode::Numpad1 => "Num1",
            KeyCode::Numpad2 => "Num2",
            KeyCode::Numpad3 => "Num3",
            KeyCode::Numpad4 => "Num4",
            KeyCode::Numpad5 => "Num5",
            KeyCode::Numpad6 => "Num6",
            KeyCode::Numpad7 => "Num7",
            KeyCode::Numpad8 => "Num8",
            KeyCode::Numpad9 => "Num9",
            KeyCode::MouseLeft => "Mouse1",
            KeyCode::MouseRight => "Mouse2",
            KeyCode::MouseMiddle => "Mouse3",
            KeyCode::MouseWheelUp => "WheelUp",
            KeyCode::MouseWheelDown => "WheelDn",
            KeyCode::Unknown(_) => "Unknown",
        }
    }

    /// Get a filesystem-safe identifier for this key (used in soundpack directories).
    pub fn fs_name(&self) -> String {
        match self {
            KeyCode::Unknown(code) => format!("unknown_{code}"),
            other => {
                let name = format!("{other:?}");
                name.to_lowercase()
            }
        }
    }
}

impl std::fmt::Display for KeyCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_code_mapping() {
        assert_eq!(KeyCode::from_scan_code(0x1E, false), KeyCode::KeyA);
        assert_eq!(KeyCode::from_scan_code(0x39, false), KeyCode::Space);
        assert_eq!(KeyCode::from_scan_code(0x1C, false), KeyCode::Enter);
        assert_eq!(KeyCode::from_scan_code(0x01, false), KeyCode::Escape);
    }

    #[test]
    fn test_extended_scan_codes() {
        // Right Ctrl has scan code 0x1D with extended flag
        assert_eq!(KeyCode::from_scan_code(0x1D, true), KeyCode::ControlRight);
        // Right Alt has scan code 0x38 with extended flag
        assert_eq!(KeyCode::from_scan_code(0x38, true), KeyCode::AltRight);
        // Arrow keys are extended
        assert_eq!(KeyCode::from_scan_code(0x48, true), KeyCode::ArrowUp);
    }

    #[test]
    fn test_display_name() {
        assert_eq!(KeyCode::KeyA.display_name(), "A");
        assert_eq!(KeyCode::Space.display_name(), "Space");
        assert_eq!(KeyCode::Enter.display_name(), "Enter");
    }

    #[test]
    fn test_fs_name() {
        assert_eq!(KeyCode::KeyA.fs_name(), "keya");
        assert_eq!(KeyCode::Space.fs_name(), "space");
        assert_eq!(KeyCode::Unknown(0xFF).fs_name(), "unknown_255");
    }
}

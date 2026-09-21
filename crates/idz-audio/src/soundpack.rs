//! Soundpack data structures, validation, and sample management.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use rand::Rng;
use serde::{Deserialize, Serialize};

use idz_shared::error::SoundpackError;
use idz_shared::KeyCode;
use crate::pcm::PcmBuffer;
use crate::preloader::load_audio_file;

/// Mode for triggering sounds on key state transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackMode {
    #[default]
    KeyDownOnly,
    KeyUpOnly,
    KeyDownAndUp,
}

/// Sample selection strategy when multiple samples are available for a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SelectionMode {
    #[default]
    AvoidRepeat,
    Random,
    Sequential,
}

/// Metadata stored in `metadata.json` inside a soundpack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundpackMetadata {
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_license")]
    pub license: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub preview: Option<String>,
    #[serde(default)]
    pub has_release_sounds: bool,
}

fn default_license() -> String {
    "MIT".to_string()
}

/// Key sound container with samples and dynamic variation parameters.
#[derive(Debug, Clone)]
pub struct KeySound {
    pub down_samples: Vec<PcmBuffer>,
    pub up_samples: Vec<PcmBuffer>,
    pub volume: f32,
    pub pitch_variance: f32,   // e.g. 0.02 = ±2%
    pub volume_variance: f32,  // e.g. 0.03 = ±3%
    pub last_down_idx: usize,
    pub last_up_idx: usize,
    pub selection_mode: SelectionMode,
}

impl Default for KeySound {
    fn default() -> Self {
        Self {
            down_samples: Vec::new(),
            up_samples: Vec::new(),
            volume: 1.0,
            pitch_variance: 0.02,
            volume_variance: 0.03,
            last_down_idx: usize::MAX,
            last_up_idx: usize::MAX,
            selection_mode: SelectionMode::AvoidRepeat,
        }
    }
}

impl KeySound {
    /// Select a down sample with natural variance.
    pub fn select_down(&mut self) -> Option<(PcmBuffer, f32, f32)> {
        if self.down_samples.is_empty() {
            return None;
        }

        let idx = self.pick_sample_index(self.down_samples.len(), self.last_down_idx);
        self.last_down_idx = idx;

        let (vol, pitch) = self.calculate_variance();
        Some((self.down_samples[idx].clone(), vol, pitch))
    }

    /// Select an up/release sample with natural variance.
    pub fn select_up(&mut self) -> Option<(PcmBuffer, f32, f32)> {
        if self.up_samples.is_empty() {
            return None;
        }

        let idx = self.pick_sample_index(self.up_samples.len(), self.last_up_idx);
        self.last_up_idx = idx;

        let (vol, pitch) = self.calculate_variance();
        Some((self.up_samples[idx].clone(), vol, pitch))
    }

    fn pick_sample_index(&self, count: usize, last_idx: usize) -> usize {
        if count <= 1 {
            return 0;
        }

        let mut rng = rand::thread_rng();
        match self.selection_mode {
            SelectionMode::Random => rng.gen_range(0..count),
            SelectionMode::Sequential => {
                if last_idx == usize::MAX { 0 } else { (last_idx + 1) % count }
            }
            SelectionMode::AvoidRepeat => {
                let mut chosen = rng.gen_range(0..count);
                if chosen == last_idx {
                    chosen = (chosen + 1 + rng.gen_range(0..(count - 1))) % count;
                }
                chosen
            }
        }
    }

    fn calculate_variance(&self) -> (f32, f32) {
        let mut rng = rand::thread_rng();
        let vol_var = if self.volume_variance > 0.0 {
            rng.gen_range(-self.volume_variance..=self.volume_variance)
        } else {
            0.0
        };
        let pitch_var = if self.pitch_variance > 0.0 {
            rng.gen_range(-self.pitch_variance..=self.pitch_variance)
        } else {
            0.0
        };

        let final_vol = (self.volume * (1.0 + vol_var)).clamp(0.0, 2.0);
        let final_pitch = (1.0 + pitch_var).clamp(0.8, 1.2);

        (final_vol, final_pitch)
    }
}

/// A fully preloaded in-memory soundpack ready for zero-latency playback.
#[derive(Debug, Clone)]
pub struct Soundpack {
    pub metadata: SoundpackMetadata,
    pub key_sounds: HashMap<KeyCode, KeySound>,
    pub fallback_sound: KeySound,
    pub mouse_sounds: HashMap<KeyCode, KeySound>,
    pub playback_mode: PlaybackMode,
}

impl Soundpack {
    /// Create an empty soundpack with metadata.
    pub fn new(metadata: SoundpackMetadata) -> Self {
        Self {
            metadata,
            key_sounds: HashMap::new(),
            fallback_sound: KeySound::default(),
            mouse_sounds: HashMap::new(),
            playback_mode: PlaybackMode::KeyDownOnly,
        }
    }

    /// Load and decode a soundpack from a directory.
    pub fn load_from_dir<P: AsRef<Path>>(dir_path: P) -> Result<Self, SoundpackError> {
        let dir = dir_path.as_ref();
        if !dir.is_dir() {
            return Err(SoundpackError::NotFound(format!("Path is not a directory: {}", dir.display())));
        }

        let meta_file = dir.join("metadata.json");
        let config_file = dir.join("config.json");

        if meta_file.exists() {
            let mut file = File::open(&meta_file)
                .map_err(|e| SoundpackError::InvalidMetadata(format!("Cannot open metadata.json: {e}")))?;
            let mut content = String::new();
            file.read_to_string(&mut content)
                .map_err(|e| SoundpackError::InvalidMetadata(format!("Cannot read metadata.json: {e}")))?;
            let metadata: SoundpackMetadata = serde_json::from_str(&content)
                .map_err(|e| SoundpackError::InvalidMetadata(format!("JSON parse error: {e}")))?;

            let mut soundpack = Soundpack::new(metadata);
            let sounds_dir = if dir.join("sounds").is_dir() {
                dir.join("sounds")
            } else {
                dir.to_path_buf()
            };

            soundpack.scan_and_load_sounds(&sounds_dir)?;
            Ok(soundpack)
        } else if config_file.exists() {
            Self::load_mechvibes_pack(dir, &config_file)
        } else {
            // Default metadata derived from folder name
            let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("soundpack");
            let metadata = SoundpackMetadata {
                id: dir_name.to_lowercase().replace(' ', "_"),
                name: dir_name.to_string(),
                author: "Local".to_string(),
                version: "1.0.0".to_string(),
                description: String::new(),
                license: "MIT".to_string(),
                tags: vec![],
                preview: None,
                has_release_sounds: false,
            };

            let mut soundpack = Soundpack::new(metadata);
            let sounds_dir = if dir.join("sounds").is_dir() {
                dir.join("sounds")
            } else {
                dir.to_path_buf()
            };

            soundpack.scan_and_load_sounds(&sounds_dir)?;
            Ok(soundpack)
        }
    }

    fn load_mechvibes_pack(dir: &Path, config_path: &Path) -> Result<Self, SoundpackError> {
        let mut file = File::open(config_path)
            .map_err(|e| SoundpackError::InvalidMetadata(format!("Cannot open config.json: {e}")))?;
        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(|e| SoundpackError::InvalidMetadata(format!("Cannot read config.json: {e}")))?;

        let cfg: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| SoundpackError::InvalidMetadata(format!("JSON parse error in config.json: {e}")))?;

        let folder_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("soundpack");
        let id = cfg.get("id").and_then(|v| v.as_str()).unwrap_or(folder_name).to_string();
        let name = cfg.get("name").and_then(|v| v.as_str()).unwrap_or(folder_name).to_string();

        let metadata = SoundpackMetadata {
            id,
            name,
            author: "Mechvibes Community".to_string(),
            version: "1.0.0".to_string(),
            description: "Custom mechanical switch sound profile".to_string(),
            license: "MIT".to_string(),
            tags: vec!["mechvibes".to_string()],
            preview: None,
            has_release_sounds: false,
        };

        let mut soundpack = Soundpack::new(metadata);
        let define_type = cfg.get("key_define_type").and_then(|v| v.as_str()).unwrap_or("multi");
        let defines = cfg.get("defines").and_then(|v| v.as_object());

        if define_type.eq_ignore_ascii_case("single") {
            let sound_rel = cfg.get("sound").and_then(|v| v.as_str()).unwrap_or("sound.ogg");
            let sound_file = dir.join(sound_rel);
            let sound_path = if sound_file.exists() {
                sound_file
            } else {
                let mut found = None;
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_file() {
                            let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                            if matches!(ext.as_str(), "ogg" | "wav" | "mp3" | "flac") {
                                found = Some(p);
                                break;
                            }
                        }
                    }
                }
                found.ok_or_else(|| SoundpackError::NotFound(format!("Audio file {sound_rel} not found"))) ?
            };

            let master_buf = load_audio_file(&sound_path)
                .map_err(|e| SoundpackError::UnsupportedFormat(format!("Failed to load {}: {e}", sound_path.display())))?;

            if let Some(defs) = defines {
                for (key_str, val) in defs {
                    if let Some(key) = parse_mechvibes_key(key_str) {
                        if let Some(arr) = val.as_array() {
                            if arr.len() >= 2 {
                                let offset_ms = arr[0].as_u64().unwrap_or(0);
                                let duration_ms = arr[1].as_u64().unwrap_or(0);
                                if duration_ms > 0 {
                                    let sub_buf = master_buf.slice_ms(offset_ms, duration_ms);
                                    soundpack.key_sounds.entry(key).or_default().down_samples.push(sub_buf);
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // "multi" mode
            let mut cache: HashMap<String, PcmBuffer> = HashMap::new();
            if let Some(defs) = defines {
                for (key_str, val) in defs {
                    if let Some(filename) = val.as_str() {
                        if let Some(key) = parse_mechvibes_key(key_str) {
                            let sample_buf = if let Some(cached) = cache.get(filename) {
                                Some(cached.clone())
                            } else {
                                let audio_path = dir.join(filename);
                                if audio_path.exists() {
                                    if let Ok(buf) = load_audio_file(&audio_path) {
                                        cache.insert(filename.to_string(), buf.clone());
                                        Some(buf)
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            };

                            if let Some(buf) = sample_buf {
                                soundpack.key_sounds.entry(key).or_default().down_samples.push(buf);
                            }
                        }
                    }
                }
            }
        }

        // Set fallback sound from space or first key if fallback is empty
        if soundpack.fallback_sound.down_samples.is_empty() {
            if let Some(space_sound) = soundpack.key_sounds.get(&KeyCode::Space) {
                soundpack.fallback_sound.down_samples = space_sound.down_samples.clone();
            } else if let Some((_, first_sound)) = soundpack.key_sounds.iter().next() {
                soundpack.fallback_sound.down_samples = first_sound.down_samples.clone();
            }
        }

        Ok(soundpack)
    }

    fn scan_and_load_sounds(&mut self, root: &Path) -> Result<(), SoundpackError> {
        let entries = std::fs::read_dir(root)
            .map_err(|e| SoundpackError::NotFound(format!("Failed to read sounds directory: {e}")))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                self.load_key_dir(dir_name, &path)?;
            } else if path.is_file() {
                // Top level audio file could be a fallback or specific key file (e.g. keya.wav, space.wav, press.wav)
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                if matches!(ext.as_str(), "wav" | "mp3" | "ogg" | "flac") {
                    if let Ok(buf) = load_audio_file(&path) {
                        if stem.eq_ignore_ascii_case("press") || stem.eq_ignore_ascii_case("down") || stem.eq_ignore_ascii_case("generic") {
                            self.fallback_sound.down_samples.push(buf);
                        } else if stem.eq_ignore_ascii_case("release") || stem.eq_ignore_ascii_case("up") {
                            self.fallback_sound.up_samples.push(buf);
                        } else if let Some(key) = parse_key_name(stem) {
                            self.key_sounds.entry(key).or_default().down_samples.push(buf);
                        } else {
                            // Generic/numbered audio files (e.g. apex1.wav, key1.wav, 1.wav)
                            // that don't match a recognized key name → load as fallback
                            // sounds so every key press in this pack produces audio
                            self.fallback_sound.down_samples.push(buf);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn load_key_dir(&mut self, key_name: &str, dir: &Path) -> Result<(), SoundpackError> {
        let key_opt = parse_key_name(key_name);
        let entries = std::fs::read_dir(dir)
            .map_err(|e| SoundpackError::NotFound(format!("Failed to read key folder {key_name}: {e}")))?;

        let mut down_samples = Vec::new();
        let mut up_samples = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                if matches!(ext.as_str(), "wav" | "mp3" | "ogg" | "flac") {
                    if let Ok(buf) = load_audio_file(&path) {
                        if stem.contains("up") || stem.contains("release") {
                            up_samples.push(buf);
                        } else {
                            down_samples.push(buf);
                        }
                    }
                }
            }
        }

        if let Some(key) = key_opt {
            let entry = self.key_sounds.entry(key).or_default();
            entry.down_samples.extend(down_samples);
            entry.up_samples.extend(up_samples);
        } else if key_name.eq_ignore_ascii_case("generic") || key_name.eq_ignore_ascii_case("fallback") {
            self.fallback_sound.down_samples.extend(down_samples);
            self.fallback_sound.up_samples.extend(up_samples);
        }

        Ok(())
    }

    /// Resample all audio buffers in this soundpack to match target audio device.
    pub fn resample_to_device(&mut self, target_sample_rate: u32, target_channels: u16) {
        for ks in self.key_sounds.values_mut() {
            for sample in &mut ks.down_samples {
                *sample = sample.resample_and_remix(target_sample_rate, target_channels);
            }
            for sample in &mut ks.up_samples {
                *sample = sample.resample_and_remix(target_sample_rate, target_channels);
            }
        }
        for sample in &mut self.fallback_sound.down_samples {
            *sample = sample.resample_and_remix(target_sample_rate, target_channels);
        }
        for sample in &mut self.fallback_sound.up_samples {
            *sample = sample.resample_and_remix(target_sample_rate, target_channels);
        }
        for ks in self.mouse_sounds.values_mut() {
            for sample in &mut ks.down_samples {
                *sample = sample.resample_and_remix(target_sample_rate, target_channels);
            }
            for sample in &mut ks.up_samples {
                *sample = sample.resample_and_remix(target_sample_rate, target_channels);
            }

            
        }
    }

    /// Get sound for a key down event.
    pub fn get_down_sample(&mut self, key: KeyCode) -> Option<(PcmBuffer, f32, f32)> {
        if let Some(ks) = self.key_sounds.get_mut(&key) {
            if let Some(res) = ks.select_down() {
                return Some(res);
            }
        }
        self.fallback_sound.select_down()
    }

    /// Get sound for a key up event.
    pub fn get_up_sample(&mut self, key: KeyCode) -> Option<(PcmBuffer, f32, f32)> {
        if let Some(ks) = self.key_sounds.get_mut(&key) {
            if let Some(res) = ks.select_up() {
                return Some(res);
            }
        }
        // Do NOT fall back to fallback_sound on key release.
        // Returning None prevents double key sounds (playing a second sound when releasing a key).
        None
    }
}

fn parse_key_name(name: &str) -> Option<KeyCode> {
    let lower = name.to_lowercase();
    match lower.as_str() {
        "space" => Some(KeyCode::Space),
        "enter" | "return" => Some(KeyCode::Enter),
        "backspace" => Some(KeyCode::Backspace),
        "tab" => Some(KeyCode::Tab),
        "escape" | "esc" => Some(KeyCode::Escape),
        "capslock" | "caps" => Some(KeyCode::CapsLock),
        "shift" | "shiftleft" | "lshift" => Some(KeyCode::ShiftLeft),
        "shiftright" | "rshift" => Some(KeyCode::ShiftRight),
        "ctrl" | "control" | "controlleft" | "lctrl" => Some(KeyCode::ControlLeft),
        "controlright" | "rctrl" => Some(KeyCode::ControlRight),
        "alt" | "altleft" | "lalt" => Some(KeyCode::AltLeft),
        "altright" | "ralt" => Some(KeyCode::AltRight),
        "win" | "metaleft" | "cmd" => Some(KeyCode::MetaLeft),
        "a" | "keya" => Some(KeyCode::KeyA),
        "b" | "keyb" => Some(KeyCode::KeyB),
        "c" | "keyc" => Some(KeyCode::KeyC),
        "d" | "keyd" => Some(KeyCode::KeyD),
        "e" | "keye" => Some(KeyCode::KeyE),
        "f" | "keyf" => Some(KeyCode::KeyF),
        "g" | "keyg" => Some(KeyCode::KeyG),
        "h" | "keyh" => Some(KeyCode::KeyH),
        "i" | "keyi" => Some(KeyCode::KeyI),
        "j" | "keyj" => Some(KeyCode::KeyJ),
        "k" | "keyk" => Some(KeyCode::KeyK),
        "l" | "keyl" => Some(KeyCode::KeyL),
        "m" | "keym" => Some(KeyCode::KeyM),
        "n" | "keyn" => Some(KeyCode::KeyN),
        "o" | "keyo" => Some(KeyCode::KeyO),
        "p" | "keyp" => Some(KeyCode::KeyP),
        "q" | "keyq" => Some(KeyCode::KeyQ),
        "r" | "keyr" => Some(KeyCode::KeyR),
        "s" | "keys" => Some(KeyCode::KeyS),
        "t" | "keyt" => Some(KeyCode::KeyT),
        "u" | "keyu" => Some(KeyCode::KeyU),
        "v" | "keyv" => Some(KeyCode::KeyV),
        "w" | "keyw" => Some(KeyCode::KeyW),
        "x" | "keyx" => Some(KeyCode::KeyX),
        "y" | "keyy" => Some(KeyCode::KeyY),
        "z" | "keyz" => Some(KeyCode::KeyZ),
        "0" | "digit0" => Some(KeyCode::Digit0),
        "1" | "digit1" => Some(KeyCode::Digit1),
        "2" | "digit2" => Some(KeyCode::Digit2),
        "3" | "digit3" => Some(KeyCode::Digit3),
        "4" | "digit4" => Some(KeyCode::Digit4),
        "5" | "digit5" => Some(KeyCode::Digit5),
        "6" | "digit6" => Some(KeyCode::Digit6),
        "7" | "digit7" => Some(KeyCode::Digit7),
        "8" | "digit8" => Some(KeyCode::Digit8),
        "9" | "digit9" => Some(KeyCode::Digit9),
        "arrowup" | "up_arrow" => Some(KeyCode::ArrowUp),
        "arrowdown" | "down_arrow" => Some(KeyCode::ArrowDown),
        "arrowleft" | "left_arrow" => Some(KeyCode::ArrowLeft),
        "arrowright" | "right_arrow" => Some(KeyCode::ArrowRight),
        "insert" | "ins" => Some(KeyCode::Insert),
        "delete" | "del" => Some(KeyCode::Delete),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "pageup" | "pgup" => Some(KeyCode::PageUp),
        "pagedown" | "pgdn" => Some(KeyCode::PageDown),
        "mouse1" | "mouseleft" => Some(KeyCode::MouseLeft),
        "mouse2" | "mouseright" => Some(KeyCode::MouseRight),
        "mouse3" | "mousemiddle" => Some(KeyCode::MouseMiddle),
        _ => None,
    }
}

fn parse_mechvibes_key(key_str: &str) -> Option<KeyCode> {
    if let Ok(num) = key_str.parse::<u32>() {
        let (scancode, extended) = if num >= 57344 {
            (num - 57344, true)
        } else if num >= 3584 {
            (num - 3584, true)
        } else if num >= 256 {
            (num & 0xFF, true)
        } else {
            (num, false)
        };
        let kc = KeyCode::from_scan_code(scancode, extended);
        if !matches!(kc, KeyCode::Unknown(_)) {
            return Some(kc);
        }
    }
    parse_key_name(key_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_mechvibes_soundpacks() {
        let soundpacks_dir = Path::new("../../soundpacks");
        if soundpacks_dir.exists() {
            let entries = std::fs::read_dir(soundpacks_dir).unwrap();
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let res = Soundpack::load_from_dir(&p);
                    assert!(res.is_ok(), "Failed to load soundpack from {}: {:?}", p.display(), res.err());
                    let pack = res.unwrap();
                    assert!(!pack.key_sounds.is_empty(), "Loaded 0 keys for soundpack {}", p.display());
                }
            }
        }
    }
}



//! Local UI Server providing REST API and Web Dashboard for IDzVibes.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;
use serde::{Deserialize, Serialize};
use tiny_http::{Header, Method, Response, Server, StatusCode};

use idz_audio::{AudioHandle, Soundpack, SoundpackMetadata};
use idz_shared::KeyCode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundpackInfo {
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: String,
    pub tags: Vec<String>,
    pub path: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateDto {
    pub active_soundpack_id: String,
    pub active_soundpack_name: String,
    pub master_volume: f32,
    pub keyboard_volume: f32,
    pub is_muted: bool,
    pub active_voices: u32,
    pub voice_capacity: u32,
    pub total_events: u64,
    pub scheduling_delay_ms: f64,
    pub device_name: String,
    pub sample_rate: u32,
    pub soundpacks: Vec<SoundpackInfo>,
}

pub struct ServerState {
    pub audio_handle: AudioHandle,
    pub active_soundpack_id: Mutex<String>,
    pub active_soundpack_name: Mutex<String>,
    pub master_volume: Mutex<f32>,
    pub keyboard_volume: Mutex<f32>,
    pub is_muted: Mutex<bool>,
    pub soundpacks_dir: PathBuf,
}

pub fn start_ui_server(
    audio_handle: AudioHandle,
    initial_pack_id: String,
    initial_pack_name: String,
    soundpacks_dir: PathBuf,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server = Server::http(format!("127.0.0.1:{port}"))
        .map_err(|e| format!("Failed to bind server: {e}"))?;

    let state = Arc::new(ServerState {
        audio_handle,
        active_soundpack_id: Mutex::new(initial_pack_id),
        active_soundpack_name: Mutex::new(initial_pack_name),
        master_volume: Mutex::new(0.8),
        keyboard_volume: Mutex::new(1.0),
        is_muted: Mutex::new(false),
        soundpacks_dir,
    });

    thread::Builder::new()
        .name("idz-ui-server".to_string())
        .spawn(move || {
            for mut request in server.incoming_requests() {
                let state_clone = Arc::clone(&state);
                let url = request.url().to_string();
                let method = request.method().clone();

                match (method, url.as_str()) {
                    (Method::Get, "/") | (Method::Get, "/index.html") => {
                        let html = include_str!("ui/index.html");
                        let mut resp = Response::from_string(html);
                        resp.add_header(Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap());
                        resp.add_header(Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap());
                        let _ = request.respond(resp);
                    }
                    (Method::Get, "/api/state") => {
                        let dto = get_state_dto(&state_clone);
                        let json = serde_json::to_string(&dto).unwrap_or_default();
                        let mut resp = Response::from_string(json);
                        resp.add_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
                        resp.add_header(Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap());
                        let _ = request.respond(resp);
                    }
                    (Method::Get, "/api/soundpacks") => {
                        let active_id = state_clone.active_soundpack_id.lock().unwrap().clone();
                        let list = scan_soundpacks(&state_clone.soundpacks_dir, &active_id);
                        let json = serde_json::to_string(&list).unwrap_or_default();
                        let mut resp = Response::from_string(json);
                        resp.add_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
                        resp.add_header(Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap());
                        let _ = request.respond(resp);
                    }
                    (Method::Post, "/api/soundpacks/apply") => {
                        let mut body = String::new();
                        let _ = request.as_reader().read_to_string(&mut body);

                        #[derive(Deserialize)]
                        struct ApplyReq {
                            id: String,
                        }

                        let res = if let Ok(req) = serde_json::from_str::<ApplyReq>(&body) {
                            apply_soundpack(&state_clone, &req.id)
                        } else {
                            Err("Invalid request payload".to_string())
                        };

                        let json_resp = match res {
                            Ok(name) => format!(r#"{{"success":true,"message":"Applied soundpack: {}"}}"#, name),
                            Err(e) => format!(r#"{{"success":false,"error":"{}"}}"#, e),
                        };

                        let mut resp = Response::from_string(json_resp);
                        resp.add_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
                        resp.add_header(Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap());
                        let _ = request.respond(resp);
                    }
                    (Method::Post, "/api/volume") => {
                        let mut body = String::new();
                        let _ = request.as_reader().read_to_string(&mut body);

                        #[derive(Deserialize)]
                        struct VolReq {
                            master: Option<f32>,
                            keyboard: Option<f32>,
                        }

                        if let Ok(req) = serde_json::from_str::<VolReq>(&body) {
                            if let Some(m) = req.master {
                                *state_clone.master_volume.lock().unwrap() = m;
                                state_clone.audio_handle.set_master_volume(m);
                            }
                            if let Some(k) = req.keyboard {
                                *state_clone.keyboard_volume.lock().unwrap() = k;
                                state_clone.audio_handle.set_keyboard_volume(k);
                            }
                        }

                        let mut resp = Response::from_string(r#"{"success":true}"#);
                        resp.add_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
                        resp.add_header(Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap());
                        let _ = request.respond(resp);
                    }
                    (Method::Post, "/api/mute") => {
                        let mut body = String::new();
                        let _ = request.as_reader().read_to_string(&mut body);

                        #[derive(Deserialize)]
                        struct MuteReq {
                            muted: bool,
                        }

                        if let Ok(req) = serde_json::from_str::<MuteReq>(&body) {
                            *state_clone.is_muted.lock().unwrap() = req.muted;
                            state_clone.audio_handle.set_mute(req.muted);
                        }

                        let mut resp = Response::from_string(r#"{"success":true}"#);
                        resp.add_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
                        resp.add_header(Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap());
                        let _ = request.respond(resp);
                    }
                    (Method::Post, "/api/test-key") => {
                        let mut body = String::new();
                        let _ = request.as_reader().read_to_string(&mut body);

                        #[derive(Deserialize)]
                        struct KeyReq {
                            key: Option<String>,
                        }

                        let key = match serde_json::from_str::<KeyReq>(&body) {
                            Ok(r) => match r.key.as_deref() {
                                Some("space") => KeyCode::Space,
                                Some("enter") => KeyCode::Enter,
                                Some("backspace") => KeyCode::Backspace,
                                _ => KeyCode::KeyA,
                            },
                            Err(_) => KeyCode::Space,
                        };

                        state_clone.audio_handle.trigger_key_down(key, 0);
                        thread::sleep(std::time::Duration::from_millis(30));
                        state_clone.audio_handle.trigger_key_up(key, 0);

                        let mut resp = Response::from_string(r#"{"success":true}"#);
                        resp.add_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
                        resp.add_header(Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap());
                        let _ = request.respond(resp);
                    }
                    (Method::Options, _) => {
                        let mut resp = Response::empty(StatusCode(200));
                        resp.add_header(Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap());
                        resp.add_header(Header::from_bytes(&b"Access-Control-Allow-Methods"[..], &b"GET, POST, OPTIONS"[..]).unwrap());
                        resp.add_header(Header::from_bytes(&b"Access-Control-Allow-Headers"[..], &b"Content-Type"[..]).unwrap());
                        let _ = request.respond(resp);
                    }
                    _ => {
                        let resp = Response::empty(StatusCode(404));
                        let _ = request.respond(resp);
                    }
                }
            }
        })?;

    Ok(())
}

fn apply_soundpack(state: &ServerState, id: &str) -> Result<String, String> {
    let pack_dir = state.soundpacks_dir.join(id);
    if !pack_dir.exists() {
        return Err(format!("Soundpack directory not found: {}", pack_dir.display()));
    }

    let soundpack = Soundpack::load_from_dir(&pack_dir)
        .map_err(|e| format!("Failed to load soundpack: {e}"))?;

    let name = soundpack.metadata.name.clone();
    state.audio_handle.set_soundpack(soundpack);

    *state.active_soundpack_id.lock().unwrap() = id.to_string();
    *state.active_soundpack_name.lock().unwrap() = name.clone();

    Ok(name)
}

fn get_state_dto(state: &ServerState) -> AppStateDto {
    let stats = state.audio_handle.stats();
    let active_id = state.active_soundpack_id.lock().unwrap().clone();
    let active_name = state.active_soundpack_name.lock().unwrap().clone();
    let master_vol = *state.master_volume.lock().unwrap();
    let kb_vol = *state.keyboard_volume.lock().unwrap();
    let muted = *state.is_muted.lock().unwrap();

    let soundpacks = scan_soundpacks(&state.soundpacks_dir, &active_id);

    AppStateDto {
        active_soundpack_id: active_id,
        active_soundpack_name: active_name,
        master_volume: master_vol,
        keyboard_volume: kb_vol,
        is_muted: muted,
        active_voices: stats.active_voices.load(Ordering::Relaxed),
        voice_capacity: stats.voice_capacity.load(Ordering::Relaxed),
        total_events: stats.total_events_processed.load(Ordering::Relaxed),
        scheduling_delay_ms: stats.last_key_scheduling_delay_us.load(Ordering::Relaxed) as f64 / 1000.0,
        device_name: state.audio_handle.device_name().to_string(),
        sample_rate: state.audio_handle.sample_rate(),
        soundpacks,
    }
}

pub fn scan_soundpacks(dir: &Path, active_id: &str) -> Vec<SoundpackInfo> {
    let mut packs = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let folder_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                let meta_path = path.join("metadata.json");

                let meta: SoundpackMetadata = if meta_path.exists() {
                    fs::read_to_string(&meta_path)
                        .ok()
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_else(|| fallback_meta(folder_name))
                } else {
                    fallback_meta(folder_name)
                };

                let id = if meta.id.is_empty() { folder_name.to_string() } else { meta.id };
                let is_active = id == active_id || folder_name == active_id;

                packs.push(SoundpackInfo {
                    id,
                    name: meta.name,
                    author: meta.author,
                    version: meta.version,
                    description: meta.description,
                    tags: meta.tags,
                    path: path.to_string_lossy().to_string(),
                    is_active,
                });
            }
        }
    }
    packs
}

fn fallback_meta(name: &str) -> SoundpackMetadata {
    SoundpackMetadata {
        id: name.to_string(),
        name: name.replace('_', " ").to_uppercase(),
        author: "Community".to_string(),
        version: "1.0.0".to_string(),
        description: "Custom mechanical switch sound profile".to_string(),
        license: "MIT".to_string(),
        tags: vec!["custom".to_string()],
        preview: None,
        has_release_sounds: false,
    }
}

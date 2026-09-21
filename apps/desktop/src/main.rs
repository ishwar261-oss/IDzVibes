//! # IDzVibes Desktop Runner & UI Server
//!
//! Ultra-low-latency keyboard sound engine with dynamic soundpack switcher UI.

pub mod server;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

use idz_audio::{AudioEngine, Soundpack};
use idz_input::{InputEngine, InputEventKind};
use server::start_ui_server;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    println!();
    println!("  =======================================================");
    println!("   IDzVibes — Ultra-Low-Latency Keyboard Sound Engine");
    println!("   \"Make Every Keystroke Feel Alive.\"");
    println!("  =======================================================");
    println!();

    let soundpacks_dir = resolve_soundpacks_dir();
    println!("  [*] Soundpacks location: {}", soundpacks_dir.display());

    if !soundpacks_dir.exists() || !soundpacks_dir.join("creamy_thock").exists() {
        println!("  [*] Generating built-in preset soundpacks...");
        crate_gen_sounds(&soundpacks_dir)?;
    }

    let initial_pack_path = if soundpacks_dir.join("creamy_thock").exists() {
        soundpacks_dir.join("creamy_thock")
    } else if let Ok(mut entries) = std::fs::read_dir(&soundpacks_dir) {
        entries.find_map(|e| e.ok().map(|entry| entry.path())).unwrap_or_else(|| soundpacks_dir.join("creamy_thock"))
    } else {
        soundpacks_dir.join("creamy_thock")
    };

    println!("  [*] Loading initial soundpack: {}", initial_pack_path.display());
    let soundpack = Soundpack::load_from_dir(&initial_pack_path)?;
    let initial_name = soundpack.metadata.name.clone();
    let initial_id = soundpack.metadata.id.clone();

    println!(
        "      Active: {} (v{}) - {} mapped keys",
        initial_name,
        soundpack.metadata.version,
        soundpack.key_sounds.len()
    );

    // Initialize Audio Engine (Persistent stream, 32 voices)
    println!("  [*] Initializing Audio Engine (WASAPI / Low-Latency)...");
    let audio_engine = AudioEngine::new(32, Some(soundpack))?;
    let audio_handle = audio_engine.handle();
    println!("      Audio Device: {}", audio_handle.device_name());
    println!("      Sample Rate:  {} Hz", audio_handle.sample_rate());
    println!("      Channels:     {}", audio_handle.channels());

    // Initialize Windows Raw Input Keyboard Engine (dedicated thread, message-only HWND)
    // Mouse capture disabled by default to eliminate mouse click/scroll sounds
    println!("  [*] Initializing Windows Raw Input Keyboard Engine (Keyboard-Only Mode)...");
    let input_engine = InputEngine::new(false)?;
    let input_rx = input_engine.receiver();
    println!("      Windows Raw Input: Active & Listening (Keyboards Only)");

    // Start Web UI Server on port 8420
    let ui_port = 8420;
    println!("  [*] Starting IDzVibes UI Dashboard on http://localhost:{} ...", ui_port);
    let _ = start_ui_server(
        audio_handle.clone(),
        initial_id,
        initial_name,
        soundpacks_dir,
        ui_port,
    );

    println!();
    println!("  [+] IDzVibes Dashboard is OPEN: http://localhost:{}", ui_port);
    println!("  [+] Type on your keyboard to hear instant acoustic feedback!");
    println!("      Press Ctrl+C or type 'q' to exit.");
    println!();


    let running = Arc::new(AtomicBool::new(true));
    let r_clone = Arc::clone(&running);

    ctrlc_handler(move || {
        println!("\n  [*] Shutting down IDzVibes...");
        r_clone.store(false, Ordering::SeqCst);
    });

    let audio_clone = audio_handle.clone();
    let run_loop = Arc::clone(&running);

    // Hot-path dispatcher thread: takes Raw Input events and routes to lock-free Audio queue.
    // Uses a HashMap with Instant timestamps to deduplicate held-key repeat events (Windows typematic repeat)
    // while ensuring multi-key combinations (Alt+Tab, Ctrl+C, etc.) trigger sounds once per key press,
    // and recovering automatically if Windows swallows KeyUp during Alt+Tab / focus changes.
    let dispatch_thread = thread::Builder::new()
        .name("idz-event-dispatcher".to_string())
        .spawn(move || {
            let mut keys_held: HashMap<idz_shared::KeyCode, Instant> = HashMap::new();
            while run_loop.load(Ordering::Relaxed) {
                match input_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(event) => {
                        if event.is_mouse() {
                            let is_down = matches!(event.kind, InputEventKind::MouseDown);
                            audio_clone.trigger_mouse(event.key, is_down);
                        } else {
                            match event.kind {
                                InputEventKind::KeyDown => {
                                    let now = Instant::now();
                                    let is_alt_held = keys_held.contains_key(&idz_shared::KeyCode::AltLeft)
                                        || keys_held.contains_key(&idz_shared::KeyCode::AltRight);

                                    let is_modifier = matches!(
                                        event.key,
                                        idz_shared::KeyCode::AltLeft
                                            | idz_shared::KeyCode::AltRight
                                            | idz_shared::KeyCode::ControlLeft
                                            | idz_shared::KeyCode::ControlRight
                                            | idz_shared::KeyCode::MetaLeft
                                            | idz_shared::KeyCode::MetaRight
                                            | idz_shared::KeyCode::ShiftLeft
                                            | idz_shared::KeyCode::ShiftRight
                                    );

                                    // Tab key and any non-modifier key pressed while Alt is held (Alt+Tab combo)
                                    // should ALWAYS trigger a sound on every press.
                                    let is_combo_or_tab = event.key == idz_shared::KeyCode::Tab || (is_alt_held && !is_modifier);

                                    let should_trigger = if is_combo_or_tab {
                                        keys_held.insert(event.key, now);
                                        true
                                    } else {
                                        match keys_held.get_mut(&event.key) {
                                            Some(last_time) => {
                                                let elapsed = now.duration_since(*last_time);
                                                *last_time = now;
                                                // If > 350ms has elapsed since the last KeyDown event for this key,
                                                // the previous KeyUp was likely missed (e.g. Alt+Tab focus loss).
                                                // If <= 350ms, it's an active typematic auto-repeat for a held key.
                                                elapsed > Duration::from_millis(350)
                                            }
                                            None => {
                                                keys_held.insert(event.key, now);
                                                true
                                            }
                                        }
                                    };

                                    if should_trigger {
                                        audio_clone.trigger_key_down(event.key, event.timestamp_us);
                                    }
                                }
                                InputEventKind::KeyUp => {
                                    keys_held.remove(&event.key);
                                    audio_clone.trigger_key_up(event.key, event.timestamp_us);
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                }
            }
        })?;

    // Telemetry monitor loop
    let stats = audio_handle.stats();
    while running.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(1500));
        let events = stats.total_events_processed.load(Ordering::Relaxed);
        let voices = stats.active_voices.load(Ordering::Relaxed);
        let cap = stats.voice_capacity.load(Ordering::Relaxed);
        let sched_delay = stats.last_key_scheduling_delay_us.load(Ordering::Relaxed);
        let underruns = stats.underrun_count.load(Ordering::Relaxed);

        if events > 0 {
            let sched_ms = sched_delay as f64 / 1000.0;
            print!(
                "\r  [Live] Events: {:<6} | Voices: {:>2}/{} | Sched Delay: {:>5.2} ms | Underruns: {}",
                events, voices, cap, sched_ms, underruns
            );
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
    }

    let _ = dispatch_thread.join();
    println!("\n  [*] Clean exit complete. Keep vibrating!");
    Ok(())
}

fn crate_gen_sounds(target: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(target)?;
    let _ = std::process::Command::new("cargo")
        .args(["run", "-p", "idz-desktop", "--bin", "gen-sounds"])
        .status();
    Ok(())
}

fn ctrlc_handler<F: FnOnce() + Send + 'static>(handler: F) {
    let handler = std::sync::Mutex::new(Some(handler));
    #[cfg(windows)]
    unsafe {
        let _ = windows::Win32::System::Console::SetConsoleCtrlHandler(
            Some(console_ctrl_handler),
            windows::Win32::Foundation::BOOL(1),
        );
    }
    thread::spawn(move || {
        let mut buf = String::new();
        while std::io::stdin().read_line(&mut buf).is_ok() {
            if buf.trim().eq_ignore_ascii_case("q") || buf.trim().eq_ignore_ascii_case("quit") {
                if let Ok(mut lock) = handler.lock() {
                    if let Some(f) = lock.take() {
                        f();
                    }
                }
                break;
            }
            buf.clear();
        }
    });
}

#[cfg(windows)]
unsafe extern "system" fn console_ctrl_handler(_ctrl_type: u32) -> windows::Win32::Foundation::BOOL {
    windows::Win32::Foundation::BOOL(1)
}

fn resolve_soundpacks_dir() -> PathBuf {
    let mut candidates = Vec::new();

    // Primary workspace soundpacks path
    candidates.push(PathBuf::from(r"D:\Github Uplods\IDzVibes\soundpacks"));

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("soundpacks"));
        let cwd_str = cwd.to_string_lossy();
        if cwd_str.to_lowercase().contains("projects") {
            let fixed = cwd_str.replace("d:\\projects\\Prompts", "d:\\Github Uplods")
                               .replace("D:\\projects\\Prompts", "D:\\Github Uplods")
                               .replace("d:/projects/Prompts", "d:/Github Uplods")
                               .replace("D:/projects/Prompts", "D:/Github Uplods");
            candidates.push(PathBuf::from(fixed).join("soundpacks"));
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("soundpacks"));
            candidates.push(exe_dir.join("../../soundpacks"));
            candidates.push(exe_dir.join("../../../soundpacks"));

            let exe_str = exe_dir.to_string_lossy();
            if exe_str.to_lowercase().contains("projects") {
                let fixed = exe_str.replace("d:\\projects\\Prompts", "d:\\Github Uplods")
                                   .replace("D:\\projects\\Prompts", "D:\\Github Uplods")
                                   .replace("d:/projects/Prompts", "d:/Github Uplods")
                                   .replace("D:/projects/Prompts", "D:/Github Uplods");
                let fixed_path = PathBuf::from(fixed);
                candidates.push(fixed_path.join("soundpacks"));
                candidates.push(fixed_path.join("../../soundpacks"));
                candidates.push(fixed_path.join("../../../soundpacks"));
            }
        }
    }

    let mut best_path = PathBuf::from("soundpacks");
    let mut max_count = 0;

    for cand in candidates {
        if cand.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&cand) {
                let count = entries.flatten().filter(|e| e.path().is_dir()).count();
                if count > max_count {
                    max_count = count;
                    best_path = cand;
                }
            }
        }
    }

    best_path
}



//! # IDzVibes Desktop Runner & UI Server
//!
//! Ultra-low-latency keyboard sound engine with dynamic soundpack switcher UI.

pub mod server;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
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

    let soundpacks_dir = PathBuf::from("soundpacks");
    if !soundpacks_dir.exists() || !soundpacks_dir.join("creamy_thock").exists() {
        println!("  [*] Generating built-in preset soundpacks...");
        crate_gen_sounds(&soundpacks_dir)?;
    }

    let initial_pack_path = soundpacks_dir.join("creamy_thock");
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

    // Auto-open browser on Windows
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", &format!("http://localhost:{}", ui_port)])
            .spawn();
    }

    let running = Arc::new(AtomicBool::new(true));
    let r_clone = Arc::clone(&running);

    ctrlc_handler(move || {
        println!("\n  [*] Shutting down IDzVibes...");
        r_clone.store(false, Ordering::SeqCst);
    });

    let audio_clone = audio_handle.clone();
    let run_loop = Arc::clone(&running);

    // Hot-path dispatcher thread: takes Raw Input events and routes to lock-free Audio queue
    let dispatch_thread = thread::Builder::new()
        .name("idz-event-dispatcher".to_string())
        .spawn(move || {
            while run_loop.load(Ordering::Relaxed) {
                match input_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(event) => {
                        if event.is_mouse() {
                            let is_down = matches!(event.kind, InputEventKind::MouseDown);
                            audio_clone.trigger_mouse(event.key, is_down);
                        } else {
                            match event.kind {
                                InputEventKind::KeyDown => {
                                    audio_clone.trigger_key_down(event.key, event.timestamp_us);
                                }
                                InputEventKind::KeyUp => {
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

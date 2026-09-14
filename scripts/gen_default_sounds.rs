//! Generates high-fidelity acoustic sound profiles for all distinct keyboard buttons:
//! Space, Enter, Backspace, Shift, Tab, CapsLock, Alt, Win, Ctrl, Esc, Arrows, and Alphanumeric keys.

use std::f32::consts::PI;
use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::Path;

fn write_wav_file(path: &Path, samples: &[f32], sample_rate: u32) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    let num_samples = samples.len() as u32;
    let byte_rate = sample_rate * 2;
    let block_align = 2u16;
    let data_size = num_samples * 2;
    let file_size = 36 + data_size;

    file.write_all(b"RIFF")?;
    file.write_all(&file_size.to_le_bytes())?;
    file.write_all(b"WAVE")?;

    file.write_all(b"fmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&sample_rate.to_le_bytes())?;
    file.write_all(&byte_rate.to_le_bytes())?;
    file.write_all(&block_align.to_le_bytes())?;
    file.write_all(&16u16.to_le_bytes())?;

    file.write_all(b"data")?;
    file.write_all(&data_size.to_le_bytes())?;

    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * 32767.0) as i16;
        file.write_all(&sample_i16.to_le_bytes())?;
    }

    Ok(())
}

fn generate_switch_sound(
    sample_rate: u32,
    base_freq: f32,
    click_freq: f32,
    duration_s: f32,
    decay_rate: f32,
    click_intensity: f32,
    noise_intensity: f32,
    stabilizer_hollow: f32,
) -> Vec<f32> {
    let total_frames = (sample_rate as f32 * duration_s) as usize;
    let mut out = Vec::with_capacity(total_frames);

    for i in 0..total_frames {
        let t = i as f32 / sample_rate as f32;

        // 1. Transient snap on initial contact (0 - 6ms)
        let click_env = (-t * 750.0).exp();
        let click = (2.0 * PI * click_freq * t).sin() * click_env * click_intensity;

        // 2. Primary body resonance
        let body_env = (-t * decay_rate).exp();
        let body1 = (2.0 * PI * base_freq * t).sin() * body_env * 0.6;
        let body2 = (2.0 * PI * (base_freq * 1.55) * t).sin() * ((-t * (decay_rate * 1.25)).exp()) * 0.3;

        // 3. Low stabilizer hollow resonance (for Space, Enter, Shift, Backspace)
        let sub = if stabilizer_hollow > 0.0 {
            (2.0 * PI * (base_freq * 0.5) * t).sin() * ((-t * (decay_rate * 0.7)).exp()) * stabilizer_hollow
        } else {
            0.0
        };

        // 4. Acoustic case impact noise burst
        let noise = ((i * 1103515245 + 12345) % 65536) as f32 / 32768.0 - 1.0;
        let noise_comp = noise * (-t * 350.0).exp() * noise_intensity;

        out.push(click + body1 + body2 + sub + noise_comp);
    }
    out
}

fn generate_release_sound(sample_rate: u32, base_freq: f32, duration_s: f32) -> Vec<f32> {
    let total_frames = (sample_rate as f32 * duration_s) as usize;
    let mut out = Vec::with_capacity(total_frames);

    for i in 0..total_frames {
        let t = i as f32 / sample_rate as f32;
        let env = (-t * 140.0).exp();
        let tone = (2.0 * PI * (base_freq * 1.6) * t).sin() * env * 0.35;
        let noise = ((i * 1664525 + 1013904223) % 65536) as f32 / 32768.0 - 1.0;
        let noise_comp = noise * (-t * 300.0).exp() * 0.15;
        out.push(tone + noise_comp);
    }
    out
}

fn generate_pack(
    target_dir: &Path,
    id: &str,
    name: &str,
    desc: &str,
    tags: &[&str],
    freq_scale: f32,
    click_scale: f32,
    decay_scale: f32,
) -> std::io::Result<()> {
    let sounds_dir = target_dir.join("sounds");
    create_dir_all(&sounds_dir)?;
    let sr = 44100;

    // Helper closure to generate a key folder
    let gen_key_sound = |folder: &str, base_f: f32, click_f: f32, dur: f32, decay: f32, click_int: f32, noise_int: f32, hollow: f32| -> std::io::Result<()> {
        let key_dir = sounds_dir.join(folder);
        create_dir_all(&key_dir)?;

        let bf = base_f * freq_scale;
        let cf = click_f * click_scale;
        let dec = decay * decay_scale;

        write_wav_file(&key_dir.join("01.wav"), &generate_switch_sound(sr, bf, cf, dur, dec, click_int, noise_int, hollow), sr)?;
        write_wav_file(&key_dir.join("02.wav"), &generate_switch_sound(sr, bf * 1.03, cf * 1.02, dur, dec, click_int, noise_int, hollow), sr)?;
        write_wav_file(&key_dir.join("03.wav"), &generate_switch_sound(sr, bf * 0.97, cf * 0.98, dur, dec, click_int, noise_int, hollow), sr)?;
        write_wav_file(&key_dir.join("up.wav"), &generate_release_sound(sr, bf * 1.2, 0.04), sr)?;
        Ok(())
    };

    // 1. Spacebar: Extra deep, heavy, hollow acoustic
    gen_key_sound("space", 68.0, 950.0, 0.10, 50.0, 0.5, 0.35, 0.7)?;

    // 2. Enter key: Heavy stabilizer thump
    gen_key_sound("enter", 88.0, 1150.0, 0.08, 60.0, 0.6, 0.30, 0.5)?;

    // 3. Backspace: Snappy stabilizer clack
    gen_key_sound("backspace", 102.0, 1300.0, 0.07, 70.0, 0.65, 0.28, 0.35)?;

    // 4. Shift Left & Right: Wide stabilizer snap
    gen_key_sound("shiftleft", 96.0, 1250.0, 0.075, 65.0, 0.6, 0.28, 0.4)?;
    gen_key_sound("shiftright", 98.0, 1270.0, 0.075, 65.0, 0.6, 0.28, 0.4)?;

    // 5. Tab key: Crisp mechanical click
    gen_key_sound("tab", 132.0, 1600.0, 0.065, 80.0, 0.7, 0.25, 0.15)?;

    // 6. Caps Lock: Solid tactile clack
    gen_key_sound("capslock", 120.0, 1450.0, 0.07, 75.0, 0.65, 0.25, 0.2)?;

    // 7. Alt Left & Right: Bottom row solid tap
    gen_key_sound("altleft", 112.0, 1380.0, 0.065, 75.0, 0.6, 0.24, 0.2)?;
    gen_key_sound("altright", 114.0, 1400.0, 0.065, 75.0, 0.6, 0.24, 0.2)?;

    // 8. Windows / Meta: Modifier snap
    gen_key_sound("metaleft", 128.0, 1500.0, 0.065, 80.0, 0.65, 0.22, 0.15)?;

    // 9. Control Left & Right: Bottom corner acoustic
    gen_key_sound("controlleft", 118.0, 1420.0, 0.065, 75.0, 0.6, 0.24, 0.2)?;
    gen_key_sound("controlright", 120.0, 1440.0, 0.065, 75.0, 0.6, 0.24, 0.2)?;

    // 10. Escape: High-pitch top-row click
    gen_key_sound("escape", 160.0, 2100.0, 0.06, 90.0, 0.8, 0.25, 0.0)?;

    // 11. Navigation / Arrow Keys
    gen_key_sound("arrowup", 140.0, 1750.0, 0.06, 85.0, 0.7, 0.22, 0.1)?;
    gen_key_sound("arrowdown", 138.0, 1720.0, 0.06, 85.0, 0.7, 0.22, 0.1)?;
    gen_key_sound("arrowleft", 142.0, 1780.0, 0.06, 85.0, 0.7, 0.22, 0.1)?;
    gen_key_sound("arrowright", 144.0, 1800.0, 0.06, 85.0, 0.7, 0.22, 0.1)?;

    // 12. Generic Alphanumeric keys (A-Z, 0-9, Punctuation)
    gen_key_sound("generic", 110.0, 1400.0, 0.065, 70.0, 0.6, 0.25, 0.1)?;

    let tags_json = tags.iter().map(|t| format!("\"{t}\"")).collect::<Vec<_>>().join(", ");
    let meta = format!(
        r#"{{
  "id": "{id}",
  "name": "{name}",
  "author": "IDzVibes Studio",
  "version": "1.0.0",
  "description": "{desc}",
  "license": "MIT",
  "tags": [{tags_json}],
  "has_release_sounds": true
}}"#
    );
    let mut f = File::create(target_dir.join("metadata.json"))?;
    f.write_all(meta.as_bytes())?;
    Ok(())
}

pub fn generate_all_soundpacks(root_dir: &Path) -> std::io::Result<()> {
    create_dir_all(root_dir)?;

    // 1. Creamy Thock
    generate_pack(
        &root_dir.join("creamy_thock"),
        "creamy_thock",
        "Creamy Thock",
        "Deep, creamy mechanical keyboard sound with dedicated acoustics for space, enter, shift, alt, win, ctrl, tab, and keys.",
        &["mechanical", "thocky", "creamy", "linear"],
        1.0, 1.0, 1.0,
    )?;

    // 2. Blue Clicky
    generate_pack(
        &root_dir.join("blue_clicky"),
        "blue_clicky",
        "Blue Clicky",
        "Crisp, tactile blue switches with distinct click snaps and custom modifier harmonics.",
        &["clicky", "tactile", "crisp", "loud"],
        1.45, 1.8, 1.15,
    )?;

    // 3. Red Linear
    generate_pack(
        &root_dir.join("red_linear"),
        "red_linear",
        "Red Linear",
        "Smooth, soft-bottoming silent linear switches with muted stabilizer tones.",
        &["linear", "silent", "smooth", "quiet"],
        0.8, 0.7, 1.3,
    )?;

    // 4. Retro Typewriter
    generate_pack(
        &root_dir.join("retro_typewriter"),
        "retro_typewriter",
        "Retro Typewriter",
        "Heavy vintage mechanical typewriter with distinctive carriage spacebar and metal key levers.",
        &["vintage", "typewriter", "heavy", "mechanical"],
        1.7, 2.2, 0.8,
    )?;

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new("soundpacks");
    println!("Generating realistic per-key acoustic soundpacks in: {}", root.display());
    generate_all_soundpacks(root)?;
    println!("All per-key soundpacks generated successfully!");
    Ok(())
}

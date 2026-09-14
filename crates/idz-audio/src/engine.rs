//! Core low-latency Audio Engine managing the CPAL stream, lock-free command queue,
//! and continuous stream health.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, Device, SampleFormat, Stream, StreamConfig};
use crossbeam_channel::{bounded, Receiver, Sender};
use tracing::{error, info};

use idz_shared::error::AudioError;
use idz_shared::KeyCode;
use crate::mixer::Mixer;
use crate::pcm::PcmBuffer;
use crate::soundpack::Soundpack;
use crate::voice_pool::VoicePool;

/// Audio commands sent across the lock-free channel into the audio rendering thread.
#[derive(Debug)]
pub enum AudioCommand {
    /// Play sound for a keyboard key event.
    PlayKey {
        key: KeyCode,
        is_down: bool,
        input_time_us: u64,
    },
    /// Play sound for a mouse event.
    PlayMouse {
        key: KeyCode,
        is_down: bool,
    },
    /// Hot-swap active soundpack.
    SetSoundpack(Box<Soundpack>),
    /// Update master volume (0.0 .. 1.0).
    SetMasterVolume(f32),
    /// Update keyboard volume (0.0 .. 1.0).
    SetKeyboardVolume(f32),
    /// Update mouse volume (0.0 .. 1.0).
    SetMouseVolume(f32),
    /// Update ambient sound volume (0.0 .. 1.0).
    SetAmbientVolume(f32),
    /// Load/change ambient audio buffer.
    SetAmbientSound(Option<PcmBuffer>),
    /// Toggle mute state.
    SetMute(bool),
}

/// Real-time engine telemetry and latency stats.
#[derive(Debug, Default)]
pub struct EngineStats {
    pub active_voices: AtomicU32,
    pub voice_capacity: AtomicU32,
    pub sample_rate: AtomicU32,
    pub channels: AtomicU32,
    pub buffer_size_frames: AtomicU32,
    pub underrun_count: AtomicU64,
    pub total_events_processed: AtomicU64,
    pub last_key_scheduling_delay_us: AtomicU64,
    pub is_running: AtomicBool,
}

/// A lightweight, thread-safe, cloneable handle to trigger sounds and send commands to the audio engine.
#[derive(Clone)]
pub struct AudioHandle {
    command_tx: Sender<AudioCommand>,
    stats: Arc<EngineStats>,
    device_name: String,
    sample_rate: u32,
    channels: u16,
}

impl AudioHandle {
    /// Trigger key press sound (non-blocking lock-free send).
    #[inline]
    pub fn trigger_key_down(&self, key: KeyCode, input_time_us: u64) {
        let _ = self.command_tx.try_send(AudioCommand::PlayKey {
            key,
            is_down: true,
            input_time_us,
        });
    }

    /// Trigger key release sound (non-blocking lock-free send).
    #[inline]
    pub fn trigger_key_up(&self, key: KeyCode, input_time_us: u64) {
        let _ = self.command_tx.try_send(AudioCommand::PlayKey {
            key,
            is_down: false,
            input_time_us,
        });
    }

    /// Trigger mouse sound.
    #[inline]
    pub fn trigger_mouse(&self, button: KeyCode, is_down: bool) {
        let _ = self.command_tx.try_send(AudioCommand::PlayMouse {
            key: button,
            is_down,
        });
    }

    /// Hot-swap the active soundpack.
    pub fn set_soundpack(&self, soundpack: Soundpack) {
        let _ = self
            .command_tx
            .try_send(AudioCommand::SetSoundpack(Box::new(soundpack)));
    }

    /// Set master volume (0.0 .. 1.0).
    pub fn set_master_volume(&self, volume: f32) {
        let _ = self
            .command_tx
            .try_send(AudioCommand::SetMasterVolume(volume));
    }

    /// Set keyboard volume (0.0 .. 2.0).
    pub fn set_keyboard_volume(&self, volume: f32) {
        let _ = self
            .command_tx
            .try_send(AudioCommand::SetKeyboardVolume(volume));
    }

    /// Set mouse volume (0.0 .. 2.0).
    pub fn set_mouse_volume(&self, volume: f32) {
        let _ = self
            .command_tx
            .try_send(AudioCommand::SetMouseVolume(volume));
    }

    /// Set ambient volume (0.0 .. 1.0).
    pub fn set_ambient_volume(&self, volume: f32) {
        let _ = self
            .command_tx
            .try_send(AudioCommand::SetAmbientVolume(volume));
    }

    /// Set ambient background sound buffer.
    pub fn set_ambient_sound(&self, sound: Option<PcmBuffer>) {
        let _ = self.command_tx.try_send(AudioCommand::SetAmbientSound(sound));
    }

    /// Set mute status.
    pub fn set_mute(&self, muted: bool) {
        let _ = self.command_tx.try_send(AudioCommand::SetMute(muted));
    }

    /// Get current engine stats.
    pub fn stats(&self) -> Arc<EngineStats> {
        Arc::clone(&self.stats)
    }

    /// Get active device name.
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Get current sample rate.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get channel count.
    pub fn channels(&self) -> u16 {
        self.channels
    }
}

/// High-performance audio engine owning the CPAL stream.
pub struct AudioEngine {
    handle: AudioHandle,
    _stream: Stream,
}

impl AudioEngine {
    /// Initialize and start the ultra-low-latency audio engine.
    ///
    /// The persistent stream is immediately started with WASAPI/CPAL.
    pub fn new(
        voice_capacity: usize,
        initial_soundpack: Option<Soundpack>,
    ) -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(AudioError::NoDevice)?;

        let device_name = device
            .name()
            .unwrap_or_else(|_| "Default Audio Device".to_string());

        info!("Initializing AudioEngine with device: {}", device_name);

        let default_config = device
            .default_output_config()
            .map_err(|e| AudioError::DeviceOpen(e.to_string()))?;

        let sample_format = default_config.sample_format();
        let sample_rate = default_config.sample_rate().0;
        let channels = default_config.channels();

        // 128-slot bounded channel — lock-free SPSC
        let (command_tx, command_rx) = bounded::<AudioCommand>(128);

        let stats = Arc::new(EngineStats::default());
        stats.voice_capacity.store(voice_capacity as u32, Ordering::Relaxed);
        stats.sample_rate.store(sample_rate, Ordering::Relaxed);
        stats.channels.store(channels as u32, Ordering::Relaxed);
        stats.is_running.store(true, Ordering::SeqCst);

        // Pre-resample initial soundpack if provided
        let mut soundpack = initial_soundpack;
        if let Some(sp) = &mut soundpack {
            sp.resample_to_device(sample_rate, channels);
        }

        // Configure low-latency stream buffer
        let config = StreamConfig {
            channels,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: BufferSize::Default,
        };

        let stats_clone = Arc::clone(&stats);

        // Build CPAL output stream
        let stream = match sample_format {
            SampleFormat::F32 => Self::build_stream::<f32>(
                &device,
                &config,
                command_rx,
                voice_capacity,
                soundpack,
                stats_clone,
                sample_rate,
                channels,
            )?,
            SampleFormat::I16 => Self::build_stream::<i16>(
                &device,
                &config,
                command_rx,
                voice_capacity,
                soundpack,
                stats_clone,
                sample_rate,
                channels,
            )?,
            SampleFormat::U16 => Self::build_stream::<u16>(
                &device,
                &config,
                command_rx,
                voice_capacity,
                soundpack,
                stats_clone,
                sample_rate,
                channels,
            )?,
            _ => {
                return Err(AudioError::UnsupportedFormat(format!(
                    "Unsupported CPAL sample format: {sample_format:?}"
                )))
            }
        };

        stream
            .play()
            .map_err(|e| AudioError::Stream(e.to_string()))?;

        let handle = AudioHandle {
            command_tx,
            stats,
            device_name,
            sample_rate,
            channels,
        };

        Ok(Self {
            handle,
            _stream: stream,
        })
    }

    /// Obtain a cloneable, thread-safe handle to send commands to the engine.
    pub fn handle(&self) -> AudioHandle {
        self.handle.clone()
    }

    #[allow(clippy::too_many_arguments)]
    fn build_stream<T>(
        device: &Device,
        config: &StreamConfig,
        command_rx: Receiver<AudioCommand>,
        voice_capacity: usize,
        initial_soundpack: Option<Soundpack>,
        stats: Arc<EngineStats>,
        sample_rate: u32,
        channels: u16,
    ) -> Result<Stream, AudioError>
    where
        T: cpal::Sample + cpal::FromSample<f32> + cpal::SizedSample,
    {
        let mut voice_pool = VoicePool::new(voice_capacity);
        let mut mixer = Mixer::default();
        let mut active_soundpack = initial_soundpack;
        let mut intermediate_buffer: Vec<f32> = Vec::with_capacity(1024);

        let err_callback = {
            let stats = Arc::clone(&stats);
            move |err: cpal::StreamError| {
                error!("Audio stream error: {err}");
                stats.underrun_count.fetch_add(1, Ordering::Relaxed);
            }
        };

        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    let num_samples = data.len();
                    let ch_count = channels as usize;
                    if intermediate_buffer.len() != num_samples {
                        intermediate_buffer.resize(num_samples, 0.0);
                    }

                    // 1. Drain all pending commands from lock-free channel (non-blocking)
                    while let Ok(cmd) = command_rx.try_recv() {
                        stats.total_events_processed.fetch_add(1, Ordering::Relaxed);
                        match cmd {
                            AudioCommand::PlayKey {
                                key,
                                is_down,
                                input_time_us,
                            } => {
                                if input_time_us > 0 {
                                    let now_us = Instant::now().elapsed().as_micros() as u64;
                                    if now_us >= input_time_us {
                                        stats.last_key_scheduling_delay_us.store(
                                            now_us - input_time_us,
                                            Ordering::Relaxed,
                                        );
                                    }
                                }

                                if let Some(sp) = &mut active_soundpack {
                                    let sample_opt = if is_down {
                                        sp.get_down_sample(key)
                                    } else {
                                        sp.get_up_sample(key)
                                    };

                                    if let Some((buf, vol, pitch)) = sample_opt {
                                        voice_pool.trigger(buf, vol, pitch);
                                    }
                                }
                            }
                            AudioCommand::PlayMouse { key, is_down } => {
                                if let Some(sp) = &mut active_soundpack {
                                    let sample_opt = if is_down {
                                        sp.get_down_sample(key)
                                    } else {
                                        sp.get_up_sample(key)
                                    };
                                    if let Some((buf, vol, pitch)) = sample_opt {
                                        let mouse_vol = vol * mixer.mouse_volume;
                                        voice_pool.trigger(buf, mouse_vol, pitch);
                                    }
                                }
                            }
                            AudioCommand::SetSoundpack(mut sp) => {
                                sp.resample_to_device(sample_rate, channels);
                                voice_pool.clear();
                                active_soundpack = Some(*sp);
                            }
                            AudioCommand::SetMasterVolume(vol) => {
                                mixer.master_volume = vol.clamp(0.0, 1.0);
                            }
                            AudioCommand::SetKeyboardVolume(vol) => {
                                mixer.keyboard_volume = vol.clamp(0.0, 2.0);
                            }
                            AudioCommand::SetMouseVolume(vol) => {
                                mixer.mouse_volume = vol.clamp(0.0, 2.0);
                            }
                            AudioCommand::SetAmbientVolume(vol) => {
                                mixer.ambient.set_volume(vol);
                            }
                            AudioCommand::SetAmbientSound(buf) => {
                                let resampled = buf.map(|b| b.resample_and_remix(sample_rate, channels));
                                mixer.ambient.set_sound(resampled);
                            }
                            AudioCommand::SetMute(muted) => {
                                mixer.is_muted = muted;
                            }
                        }
                    }

                    // 2. Process audio & mix
                    mixer.process(&mut voice_pool, &mut intermediate_buffer, ch_count);

                    // 3. Write mixed float samples to device output buffer
                    for (i, &sample_f32) in intermediate_buffer.iter().enumerate() {
                        data[i] = T::from_sample(sample_f32);
                    }

                    // 4. Update atomic stats
                    stats
                        .active_voices
                        .store(voice_pool.active_count() as u32, Ordering::Relaxed);
                    stats
                        .buffer_size_frames
                        .store((num_samples / ch_count) as u32, Ordering::Relaxed);
                },
                err_callback,
                None,
            )
            .map_err(|e| AudioError::DeviceOpen(e.to_string()))?;

        Ok(stream)
    }
}

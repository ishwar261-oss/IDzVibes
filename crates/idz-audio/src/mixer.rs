//! Audio mixer with volume scaling, ambient sound loop, and soft limiter.

use crate::pcm::PcmBuffer;
use crate::voice_pool::VoicePool;

/// Ambient background audio channel (rain, café, fireplace, etc.).
pub struct AmbientChannel {
    buffer: Option<PcmBuffer>,
    cursor_frames: usize,
    volume: f32,
    target_volume: f32,
    fade_step: f32,
    is_playing: bool,
}

impl Default for AmbientChannel {
    fn default() -> Self {
        Self {
            buffer: None,
            cursor_frames: 0,
            volume: 0.0,
            target_volume: 0.3,
            fade_step: 0.0005,
            is_playing: false,
        }
    }
}

impl AmbientChannel {
    pub fn set_sound(&mut self, buffer: Option<PcmBuffer>) {
        self.buffer = buffer;
        self.cursor_frames = 0;
    }

    pub fn play(&mut self, target_volume: f32) {
        self.target_volume = target_volume.clamp(0.0, 1.0);
        self.is_playing = true;
    }

    pub fn stop(&mut self) {
        self.target_volume = 0.0;
        if self.volume <= 0.001 {
            self.is_playing = false;
        }
    }

    pub fn set_volume(&mut self, vol: f32) {
        self.target_volume = vol.clamp(0.0, 1.0);
        if !self.is_playing && self.target_volume > 0.0 {
            self.is_playing = true;
        }
    }

    #[inline]
    pub fn render_and_mix(&mut self, out_buffer: &mut [f32], channels: usize) {
        if !self.is_playing || channels == 0 || out_buffer.is_empty() {
            return;
        }

        let buf = match &self.buffer {
            Some(b) => b,
            None => return,
        };

        let src_channels = buf.channels as usize;
        let src_frames = if src_channels > 0 { buf.samples.len() / src_channels } else { 0 };
        if src_frames == 0 {
            return;
        }

        let num_frames = out_buffer.len() / channels;

        for frame_idx in 0..num_frames {
            // Smooth volume fade
            if (self.volume - self.target_volume).abs() > self.fade_step {
                if self.volume < self.target_volume {
                    self.volume += self.fade_step;
                } else {
                    self.volume -= self.fade_step;
                }
            } else {
                self.volume = self.target_volume;
                if self.target_volume <= 0.0 {
                    self.is_playing = false;
                    break;
                }
            }

            let cur_frame = self.cursor_frames;
            for ch in 0..channels {
                let src_ch = if src_channels == 1 { 0 } else { ch % src_channels };
                let sample = buf.samples[cur_frame * src_channels + src_ch];
                out_buffer[frame_idx * channels + ch] += sample * self.volume;
            }

            self.cursor_frames = (self.cursor_frames + 1) % src_frames;
        }
    }
}

/// Central audio mixer.
pub struct Mixer {
    pub master_volume: f32,
    pub keyboard_volume: f32,
    pub mouse_volume: f32,
    pub ambient: AmbientChannel,
    pub is_muted: bool,
}

impl Default for Mixer {
    fn default() -> Self {
        Self {
            master_volume: 0.8,
            keyboard_volume: 1.0,
            mouse_volume: 0.7,
            ambient: AmbientChannel::default(),
            is_muted: false,
        }
    }
}

impl Mixer {
    /// Render active voices and ambient audio, apply volume and soft limiting.
    #[inline]
    pub fn process(
        &mut self,
        voice_pool: &mut VoicePool,
        out_buffer: &mut [f32],
        channels: usize,
    ) {
        // Clear destination buffer to silence
        out_buffer.fill(0.0);

        if self.is_muted || self.master_volume <= 0.0001 {
            return;
        }

        // 1. Render keyboard / mouse voices
        voice_pool.render_and_mix(out_buffer, channels);

        // Scale by keyboard volume factor
        if (self.keyboard_volume - 1.0).abs() > 0.001 {
            for sample in out_buffer.iter_mut() {
                *sample *= self.keyboard_volume;
            }
        }

        // 2. Render ambient channel
        self.ambient.render_and_mix(out_buffer, channels);

        // 3. Apply master volume & soft-clipping limiter (smooth polynomial knee)
        let master = self.master_volume;
        for sample in out_buffer.iter_mut() {
            let s = *sample * master;
            // Soft-knee limiter: smooth curve prevents harsh digital clipping
            *sample = if s > 1.0 {
                1.0 - (-s + 1.0).exp() * 0.1
            } else if s < -1.0 {
                -1.0 + (s + 1.0).exp() * 0.1
            } else {
                s
            }.clamp(-1.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mixer_mute() {
        let mut mixer = Mixer {
            is_muted: true,
            ..Default::default()
        };
        let mut pool = VoicePool::new(4);
        let mut buf = vec![1.0; 10];
        mixer.process(&mut pool, &mut buf, 2);
        for s in buf {
            assert_eq!(s, 0.0);
        }
    }

    #[test]
    fn test_mixer_soft_limiter() {
        let mut mixer = Mixer {
            master_volume: 1.0,
            ..Default::default()
        };
        let mut pool = VoicePool::new(4);
        // Trigger 4 overlapping loud sounds
        for _ in 0..4 {
            pool.trigger(PcmBuffer::new(vec![0.8; 20], 2, 44100), 1.0, 1.0);
        }
        let mut buf = vec![0.0; 20];
        mixer.process(&mut pool, &mut buf, 2);
        for s in buf {
            assert!(s.abs() <= 1.0, "Sample exceeds ±1.0: {s}");
        }
    }
}


//! Zero-allocation voice pool for overlapping mechanical keyboard sounds.

use crate::pcm::PcmBuffer;

/// A single active or idle playback voice.
#[derive(Debug, Clone)]
pub struct Voice {
    /// The PCM audio buffer being played.
    buffer: Option<PcmBuffer>,
    /// Current playhead position in frames.
    cursor_frames: usize,
    /// Playback volume (0.0 .. 1.0+).
    pub volume: f32,
    /// Playback pitch multiplier (e.g., 0.98 .. 1.02 for natural variations).
    pub pitch_ratio: f32,
    /// Whether this voice is currently actively rendering audio.
    pub is_active: bool,
    /// Monotonically increasing sequence number when triggered (used for oldest-voice stealing).
    pub seq_id: u64,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            buffer: None,
            cursor_frames: 0,
            volume: 1.0,
            pitch_ratio: 1.0,
            is_active: false,
            seq_id: 0,
        }
    }
}

/// A fixed-size pool of voices for concurrent audio playback.
pub struct VoicePool {
    voices: Vec<Voice>,
    capacity: usize,
    next_seq: u64,
}

impl VoicePool {
    /// Create a new pre-allocated voice pool with the given capacity (e.g. 32 or 64).
    pub fn new(capacity: usize) -> Self {
        let mut voices = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            voices.push(Voice::default());
        }
        Self {
            voices,
            capacity,
            next_seq: 1,
        }
    }

    /// Trigger playback of a sample buffer on an available or stolen voice.
    ///
    /// This method is strictly non-allocating.
    pub fn trigger(
        &mut self,
        buffer: PcmBuffer,
        volume: f32,
        pitch_ratio: f32,
    ) {
        if buffer.samples.is_empty() {
            return;
        }

        self.next_seq = self.next_seq.wrapping_add(1);
        let seq = self.next_seq;

        // 1. First, search for an idle/inactive voice
        if let Some(voice) = self.voices.iter_mut().find(|v| !v.is_active) {
            voice.buffer = Some(buffer);
            voice.cursor_frames = 0;
            voice.volume = volume.max(0.0);
            voice.pitch_ratio = pitch_ratio.clamp(0.5, 2.0);
            voice.is_active = true;
            voice.seq_id = seq;
            return;
        }

        // 2. If all voices are active, steal the oldest active voice (smallest seq_id)
        if let Some(oldest_voice) = self.voices.iter_mut().min_by_key(|v| v.seq_id) {
            oldest_voice.buffer = Some(buffer);
            oldest_voice.cursor_frames = 0;
            oldest_voice.volume = volume.max(0.0);
            oldest_voice.pitch_ratio = pitch_ratio.clamp(0.5, 2.0);
            oldest_voice.is_active = true;
            oldest_voice.seq_id = seq;
        }
    }

    /// Number of currently active voices.
    pub fn active_count(&self) -> usize {
        self.voices.iter().filter(|v| v.is_active).count()
    }

    /// Total capacity of the voice pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Reset and silence all active voices.
    pub fn clear(&mut self) {
        for voice in &mut self.voices {
            voice.is_active = false;
            voice.buffer = None;
            voice.cursor_frames = 0;
        }
    }

    /// Render and mix audio from all active voices into the destination buffer.
    ///
    /// `out_buffer` is an interleaved slice (e.g. L, R, L, R, ...).
    /// `channels` is the number of destination channels (1 or 2).
    #[inline]
    pub fn render_and_mix(&mut self, out_buffer: &mut [f32], channels: usize) {
        if channels == 0 || out_buffer.is_empty() {
            return;
        }

        let num_frames = out_buffer.len() / channels;

        for voice in &mut self.voices {
            if !voice.is_active {
                continue;
            }

            let buf = match &voice.buffer {
                Some(b) => b,
                None => {
                    voice.is_active = false;
                    continue;
                }
            };

            let src_channels = buf.channels as usize;
            let src_total_frames = if src_channels > 0 {
                buf.samples.len() / src_channels
            } else {
                0
            };

            if src_total_frames == 0 || voice.cursor_frames >= src_total_frames {
                voice.is_active = false;
                continue;
            }

            let vol = voice.volume;
            let pitch = voice.pitch_ratio;

            let mut frame_idx = 0;
            while frame_idx < num_frames {
                let current_src_frame_f = voice.cursor_frames as f32 + (frame_idx as f32 * pitch);
                let src_frame_0 = current_src_frame_f.floor() as usize;

                if src_frame_0 >= src_total_frames {
                    voice.is_active = false;
                    break;
                }

                let src_frame_1 = (src_frame_0 + 1).min(src_total_frames - 1);
                let frac = current_src_frame_f - src_frame_0 as f32;

                for ch in 0..channels {
                    let src_ch = if src_channels == 1 { 0 } else { ch % src_channels };
                    let s0 = buf.samples[src_frame_0 * src_channels + src_ch];
                    let s1 = buf.samples[src_frame_1 * src_channels + src_ch];
                    let interpolated = s0 + (s1 - s0) * frac;

                    out_buffer[frame_idx * channels + ch] += interpolated * vol;
                }

                frame_idx += 1;
            }

            if voice.is_active {
                let advanced_frames = (num_frames as f32 * pitch).ceil() as usize;
                voice.cursor_frames += advanced_frames;
                if voice.cursor_frames >= src_total_frames {
                    voice.is_active = false;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_pool_capacity_and_trigger() {
        let mut pool = VoicePool::new(4);
        assert_eq!(pool.capacity(), 4);
        assert_eq!(pool.active_count(), 0);

        let buf = PcmBuffer::new(vec![0.5; 100], 1, 44100);
        pool.trigger(buf.clone(), 1.0, 1.0);
        assert_eq!(pool.active_count(), 1);

        pool.trigger(buf.clone(), 1.0, 1.0);
        pool.trigger(buf.clone(), 1.0, 1.0);
        pool.trigger(buf.clone(), 1.0, 1.0);
        assert_eq!(pool.active_count(), 4);

        // 5th trigger steals oldest voice without panic or allocation
        pool.trigger(buf, 1.0, 1.0);
        assert_eq!(pool.active_count(), 4);
    }

    #[test]
    fn test_voice_pool_render_and_complete() {
        let mut pool = VoicePool::new(4);
        let samples = vec![0.5; 10];
        let buf = PcmBuffer::new(samples, 1, 44100);

        pool.trigger(buf, 1.0, 1.0);
        assert_eq!(pool.active_count(), 1);

        let mut out = vec![0.0; 20]; // 10 frames stereo
        pool.render_and_mix(&mut out, 2);

        // All 10 frames should have played and voice should now be inactive
        assert_eq!(pool.active_count(), 0);
        assert!((out[0] - 0.5).abs() < 1e-4);
    }
}


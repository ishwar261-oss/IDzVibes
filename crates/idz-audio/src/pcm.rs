//! In-memory audio buffer and fast resampling utilities.

use std::sync::Arc;

/// A pre-decoded, in-memory audio sample.
///
/// Samples are decoded and converted to 32-bit floating point PCM at load time
/// and resampled to the output device's native sample rate.
/// Playback in the audio callback is 1:1 memory reads without conversion or resampling.
#[derive(Debug, Clone)]
pub struct PcmBuffer {
    /// Interleaved audio channels or mono samples normalized to [-1.0, 1.0].
    pub samples: Arc<[f32]>,
    /// Number of audio channels (1 = Mono, 2 = Stereo).
    pub channels: u16,
    /// Sample rate of this buffer in Hz.
    pub sample_rate: u32,
    /// Duration in milliseconds.
    pub duration_ms: f32,
}

impl PcmBuffer {
    /// Create a new PcmBuffer.
    pub fn new(samples: Vec<f32>, channels: u16, sample_rate: u32) -> Self {
        let total_samples = samples.len();
        let frames = if channels > 0 { total_samples / channels as usize } else { total_samples };
        let duration_ms = if sample_rate > 0 {
            (frames as f32 / sample_rate as f32) * 1000.0
        } else {
            0.0
        };

        Self {
            samples: samples.into(),
            channels,
            sample_rate,
            duration_ms,
        }
    }

    /// Resample this buffer to a target sample rate and channel count using high-quality linear interpolation.
    pub fn resample_and_remix(&self, target_sample_rate: u32, target_channels: u16) -> Self {
        if self.sample_rate == target_sample_rate && self.channels == target_channels {
            return self.clone();
        }

        let src_channels = self.channels as usize;
        let dst_channels = target_channels as usize;
        if src_channels == 0 || dst_channels == 0 || self.samples.is_empty() {
            return Self::new(vec![], target_channels, target_sample_rate);
        }

        let src_frames = self.samples.len() / src_channels;
        if src_frames == 0 {
            return Self::new(vec![], target_channels, target_sample_rate);
        }

        let ratio = target_sample_rate as f64 / self.sample_rate as f64;
        let dst_frames = (src_frames as f64 * ratio).ceil() as usize;
        let mut dst_samples = Vec::with_capacity(dst_frames * dst_channels);

        for frame_idx in 0..dst_frames {
            let src_frame_f = frame_idx as f64 / ratio;
            let src_frame_0 = src_frame_f.floor() as usize;
            let src_frame_1 = (src_frame_0 + 1).min(src_frames - 1);
            let frac = (src_frame_f - src_frame_0 as f64) as f32;

            for ch in 0..dst_channels {
                // Map target channel to source channel
                let src_ch = if src_channels == 1 {
                    0
                } else if dst_channels == 1 {
                    0 // Average stereo to mono if needed
                } else {
                    ch % src_channels
                };

                let sample_0 = if dst_channels == 1 && src_channels > 1 {
                    let mut sum = 0.0;
                    for sc in 0..src_channels {
                        sum += self.samples[src_frame_0 * src_channels + sc];
                    }
                    sum / src_channels as f32
                } else {
                    self.samples[src_frame_0 * src_channels + src_ch]
                };

                let sample_1 = if dst_channels == 1 && src_channels > 1 {
                    let mut sum = 0.0;
                    for sc in 0..src_channels {
                        sum += self.samples[src_frame_1 * src_channels + sc];
                    }
                    sum / src_channels as f32
                } else {
                    self.samples[src_frame_1 * src_channels + src_ch]
                };

                // Linear interpolation
                let sample = sample_0 + (sample_1 - sample_0) * frac;
                dst_samples.push(sample);
            }
        }

        Self::new(dst_samples, target_channels, target_sample_rate)
    }

    /// Extract a sub-range of audio by start offset and duration in milliseconds.
    pub fn slice_ms(&self, offset_ms: u64, duration_ms: u64) -> Self {
        let channels = self.channels as usize;
        if channels == 0 || self.samples.is_empty() {
            return Self::new(vec![], self.channels, self.sample_rate);
        }

        let start_frame = ((offset_ms as f64 / 1000.0) * self.sample_rate as f64) as usize;
        let frame_count = ((duration_ms as f64 / 1000.0) * self.sample_rate as f64) as usize;
        let total_frames = self.samples.len() / channels;

        if start_frame >= total_frames {
            return Self::new(vec![], self.channels, self.sample_rate);
        }

        let end_frame = (start_frame + frame_count).min(total_frames);
        let start_sample = start_frame * channels;
        let end_sample = end_frame * channels;

        let sliced_samples = self.samples[start_sample..end_sample].to_vec();
        Self::new(sliced_samples, self.channels, self.sample_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pcm_duration() {
        let buf = PcmBuffer::new(vec![0.0; 44100], 1, 44100);
        assert!((buf.duration_ms - 1000.0).abs() < 1e-2);
    }

    #[test]
    fn test_pcm_resample() {
        let buf = PcmBuffer::new(vec![1.0; 100], 1, 44100);
        let resampled = buf.resample_and_remix(48000, 2);
        assert_eq!(resampled.channels, 2);
        assert_eq!(resampled.sample_rate, 48000);
        assert!(!resampled.samples.is_empty());
    }

    #[test]
    fn test_pcm_slice_ms() {
        let samples: Vec<f32> = (0..44100).map(|i| i as f32).collect();
        let buf = PcmBuffer::new(samples, 1, 44100);
        let slice = buf.slice_ms(500, 200); // 0.5s to 0.7s -> 8820 samples
        assert_eq!(slice.samples.len(), 8820);
        assert_eq!(slice.samples[0], 22050.0);
    }
}



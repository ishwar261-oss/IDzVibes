//! Audio file decoder and preloader using Symphonia.

use std::fs::File;
use std::io::Cursor;
use std::path::Path;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use idz_shared::error::AudioError;
use crate::pcm::PcmBuffer;

/// Decodes an audio file from disk into a `PcmBuffer`.
pub fn load_audio_file<P: AsRef<Path>>(path: P) -> Result<PcmBuffer, AudioError> {
    let file = File::open(path.as_ref())
        .map_err(|e| AudioError::Decode(format!("Failed to open file {}: {e}", path.as_ref().display())))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.as_ref().extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }

    decode_media_source(mss, &hint)
}

/// Decodes audio data from in-memory bytes into a `PcmBuffer`.
pub fn load_audio_bytes(bytes: &[u8], format_hint: Option<&str>) -> Result<PcmBuffer, AudioError> {
    let cursor = Cursor::new(bytes.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = format_hint {
        hint.with_extension(ext);
    }

    decode_media_source(mss, &hint)
}

fn decode_media_source(
    mss: MediaSourceStream,
    hint: &Hint,
) -> Result<PcmBuffer, AudioError> {
    let format_opts = FormatOptions {
        enable_gapless: true,
        ..Default::default()
    };
    let metadata_opts = MetadataOptions::default();
    let decoder_opts = DecoderOptions::default();

    // Probe media format
    let probed = symphonia::default::get_probe()
        .format(hint, mss, &format_opts, &metadata_opts)
        .map_err(|e| AudioError::UnsupportedFormat(format!("Failed to probe audio format: {e}")))?;

    let mut format = probed.format;

    // Find the first default audio track
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| AudioError::Decode("No playable audio track found".to_string()))?;

    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or_else(|| AudioError::Decode("Audio track missing sample rate".to_string()))?;

    let channels = track
        .codec_params
        .channels
        .map(|c| c.count() as u16)
        .unwrap_or(2);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &decoder_opts)
        .map_err(|e| AudioError::Decode(format!("Failed to create codec decoder: {e}")))?;

    let mut decoded_samples: Vec<f32> = Vec::new();

    // Decode all packets in the stream
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(SymphoniaError::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(_e) => {
                // If EOF or stream end
                break;
            }
        };

        match decoder.decode(&packet) {
            Ok(decoded_ref) => {
                extract_samples_to_f32(&decoded_ref, &mut decoded_samples);
            }
            Err(SymphoniaError::IoError(_)) => break,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(_) => break,
        }
    }

    if decoded_samples.is_empty() {
        return Err(AudioError::Decode("Decoded buffer is empty".to_string()));
    }

    Ok(PcmBuffer::new(decoded_samples, channels, sample_rate))
}

fn extract_samples_to_f32(buf_ref: &AudioBufferRef, out: &mut Vec<f32>) {
    match buf_ref {
        AudioBufferRef::F32(buf) => {
            let num_channels = buf.spec().channels.count();
            let num_frames = buf.frames();
            for frame in 0..num_frames {
                for ch in 0..num_channels {
                    out.push(buf.chan(ch)[frame]);
                }
            }
        }
        AudioBufferRef::U8(buf) => {
            let num_channels = buf.spec().channels.count();
            let num_frames = buf.frames();
            for frame in 0..num_frames {
                for ch in 0..num_channels {
                    let s = buf.chan(ch)[frame];
                    out.push((s as f32 - 128.0) / 128.0);
                }
            }
        }
        AudioBufferRef::U16(buf) => {
            let num_channels = buf.spec().channels.count();
            let num_frames = buf.frames();
            for frame in 0..num_frames {
                for ch in 0..num_channels {
                    let s = buf.chan(ch)[frame];
                    out.push((s as f32 - 32768.0) / 32768.0);
                }
            }
        }
        AudioBufferRef::S16(buf) => {
            let num_channels = buf.spec().channels.count();
            let num_frames = buf.frames();
            for frame in 0..num_frames {
                for ch in 0..num_channels {
                    let s = buf.chan(ch)[frame];
                    out.push(s as f32 / 32768.0);
                }
            }
        }
        AudioBufferRef::S24(buf) => {
            let num_channels = buf.spec().channels.count();
            let num_frames = buf.frames();
            for frame in 0..num_frames {
                for ch in 0..num_channels {
                    let s = buf.chan(ch)[frame].0;
                    out.push(s as f32 / 8388608.0);
                }
            }
        }
        AudioBufferRef::S32(buf) => {
            let num_channels = buf.spec().channels.count();
            let num_frames = buf.frames();
            for frame in 0..num_frames {
                for ch in 0..num_channels {
                    let s = buf.chan(ch)[frame];
                    out.push(s as f32 / 2147483648.0);
                }
            }
        }
        AudioBufferRef::S8(buf) => {
            let num_channels = buf.spec().channels.count();
            let num_frames = buf.frames();
            for frame in 0..num_frames {
                for ch in 0..num_channels {
                    let s = buf.chan(ch)[frame];
                    out.push(s as f32 / 128.0);
                }
            }
        }
        AudioBufferRef::U24(buf) => {
            let num_channels = buf.spec().channels.count();
            let num_frames = buf.frames();
            for frame in 0..num_frames {
                for ch in 0..num_channels {
                    let s = buf.chan(ch)[frame].0;
                    out.push((s as f32 - 8388608.0) / 8388608.0);
                }
            }
        }
        AudioBufferRef::U32(buf) => {
            let num_channels = buf.spec().channels.count();
            let num_frames = buf.frames();
            for frame in 0..num_frames {
                for ch in 0..num_channels {
                    let s = buf.chan(ch)[frame];
                    out.push((s as f32 - 2147483648.0) / 2147483648.0);
                }
            }
        }
        AudioBufferRef::F64(buf) => {
            let num_channels = buf.spec().channels.count();
            let num_frames = buf.frames();
            for frame in 0..num_frames {
                for ch in 0..num_channels {
                    out.push(buf.chan(ch)[frame] as f32);
                }
            }
        }
    }
}

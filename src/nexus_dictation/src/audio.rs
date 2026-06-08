//! Live microphone capture via cpal with resampling to 16 kHz mono f32.

use crate::error::{DictationError, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, Stream};
use std::sync::{Arc, Mutex};

/// Whisper expects 16 kHz mono PCM.
pub const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// Thread-safe buffer collecting resampled audio samples.
#[derive(Debug, Default)]
pub struct AudioBuffer {
    inner: Mutex<Vec<f32>>,
}

impl AudioBuffer {
    /// Create an empty buffer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append samples.
    pub fn push_samples(&self, samples: &[f32]) {
        if let Ok(mut buf) = self.inner.lock() {
            buf.extend_from_slice(samples);
        }
    }

    /// Drain all collected samples.
    pub fn take_samples(&self) -> Vec<f32> {
        self.inner
            .lock()
            .map(|mut buf| std::mem::take(&mut *buf))
            .unwrap_or_default()
    }

    /// Number of samples currently buffered.
    pub fn len(&self) -> usize {
        self.inner.lock().map(|b| b.len()).unwrap_or(0)
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.lock().is_ok_and(|b| b.is_empty())
    }

    /// Clear without returning samples.
    pub fn clear(&self) {
        if let Ok(mut buf) = self.inner.lock() {
            buf.clear();
        }
    }
}

/// List available input device names.
pub fn list_input_devices() -> Result<Vec<String>> {
    let host = cpal::default_host();
    let mut names = Vec::new();
    for device in host
        .input_devices()
        .map_err(|e| DictationError::Audio(e.to_string()))?
    {
        if let Ok(name) = device.name() {
            names.push(name);
        }
    }
    Ok(names)
}

/// Resample mono f32 samples from `from_rate` to `to_rate` via linear interpolation.
pub fn resample_mono(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if input.is_empty() || from_rate == to_rate {
        return input.to_vec();
    }

    let ratio = f64::from(from_rate) / f64::from(to_rate);
    let out_len = ((f64::from(input.len() as u32)) / ratio).ceil() as usize;
    let mut output = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let src_idx = f64::from(i as u32) * ratio;
        let idx0 = src_idx.floor() as usize;
        let idx1 = (idx0 + 1).min(input.len().saturating_sub(1));
        let frac = (src_idx - f64::from(idx0 as u32)) as f32;
        let sample = input[idx0] * (1.0 - frac) + input[idx1] * frac;
        output.push(sample);
    }

    output
}

/// Convert interleaved multi-channel samples to mono by averaging channels.
pub fn downmix_to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    let channels = usize::from(channels.max(1));
    if channels == 1 {
        return samples.to_vec();
    }

    samples
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Live microphone capture stream writing resampled mono 16 kHz samples into `buffer`.
pub struct MicCapture {
    _stream: Stream,
    buffer: Arc<AudioBuffer>,
}

impl MicCapture {
    /// Start capturing from the default or named input device.
    pub fn start(device_name: Option<&str>) -> Result<Self> {
        let host = cpal::default_host();
        let device = match device_name {
            Some(name) => host
                .input_devices()
                .map_err(|e| DictationError::Audio(e.to_string()))?
                .find(|d| d.name().map(|n| n == name).unwrap_or(false))
                .ok_or_else(|| DictationError::Audio(format!("device not found: {name}")))?,
            None => host
                .default_input_device()
                .ok_or_else(|| DictationError::Audio("no default input device".into()))?,
        };

        let config = device
            .default_input_config()
            .map_err(|e| DictationError::Audio(e.to_string()))?;

        let sample_rate = config.sample_rate().0;
        let channels = config.channels();
        let buffer = Arc::new(AudioBuffer::new());
        let buf_clone = Arc::clone(&buffer);

        let stream = match config.sample_format() {
            SampleFormat::F32 => {
                build_stream::<f32>(&device, &config.into(), buf_clone, sample_rate, channels)?
            }
            SampleFormat::I16 => {
                build_stream::<i16>(&device, &config.into(), buf_clone, sample_rate, channels)?
            }
            SampleFormat::U16 => {
                build_stream::<u16>(&device, &config.into(), buf_clone, sample_rate, channels)?
            }
            other => {
                return Err(DictationError::Audio(format!(
                    "unsupported sample format: {other:?}"
                )));
            }
        };

        stream
            .play()
            .map_err(|e| DictationError::Audio(e.to_string()))?;

        Ok(Self {
            _stream: stream,
            buffer,
        })
    }

    /// Shared audio buffer receiving samples.
    #[must_use]
    pub fn buffer(&self) -> Arc<AudioBuffer> {
        Arc::clone(&self.buffer)
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: Arc<AudioBuffer>,
    sample_rate: u32,
    channels: u16,
) -> Result<Stream>
where
    T: Sample + cpal::SizedSample,
    f32: FromSample<T>,
{
    let channels_copy = channels;
    let err_fn = |err| tracing::error!(%err, "audio stream error");

    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                let f32_samples: Vec<f32> = data.iter().map(|s| s.to_sample()).collect();
                let mono = downmix_to_mono(&f32_samples, channels_copy);
                let resampled = resample_mono(&mono, sample_rate, WHISPER_SAMPLE_RATE);
                buffer.push_samples(&resampled);
            },
            err_fn,
            None,
        )
        .map_err(|e| DictationError::Audio(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_halves_length_when_halving_rate() {
        let input: Vec<f32> = (0..441).map(|i| i as f32).collect();
        let out = resample_mono(&input, 44_100, 22_050);
        assert!((out.len() as i32 - 220).abs() <= 2);
    }

    #[test]
    fn downmix_stereo_to_mono() {
        let stereo = vec![1.0, 3.0, 2.0, 4.0];
        let mono = downmix_to_mono(&stereo, 2);
        assert_eq!(mono, vec![2.0, 3.0]);
    }
}

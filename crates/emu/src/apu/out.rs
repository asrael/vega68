use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use cpal::FrameCount;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

const CAPACITY: usize = 12800;
const FALLBACK_TARGET: usize = 3200;
const PERIOD: FrameCount = 800;
const QUEUE_TARGET: usize = 1600;

fn find_config(device: &cpal::Device) -> Option<cpal::SupportedStreamConfig> {
    let rate = cpal::SampleRate(48_000);
    let ranges: Vec<_> = device
        .supported_output_configs()
        .ok()?
        .filter(|c| c.channels() == 2 && c.min_sample_rate() <= rate && rate <= c.max_sample_rate())
        .collect();

    ranges
        .iter()
        .find(|c| c.sample_format() == cpal::SampleFormat::I16)
        .or_else(|| {
            ranges
                .iter()
                .find(|c| c.sample_format() == cpal::SampleFormat::F32)
        })
        .cloned()
        .map(|c| c.with_sample_rate(rate))
}

fn build_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    ring: Ring,
    buffer_size: cpal::BufferSize,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let mut stream_config: cpal::StreamConfig = config.clone().into();
    stream_config.buffer_size = buffer_size;
    let err_fn = |e| eprintln!("audio: stream error: {e}");

    match config.sample_format() {
        cpal::SampleFormat::I16 => device.build_output_stream(
            &stream_config,
            move |data: &mut [i16], _| ring.pop_slice(data),
            err_fn,
            None,
        ),

        cpal::SampleFormat::F32 => device.build_output_stream(
            &stream_config,
            move |data: &mut [f32], _| ring.pop_slice_f32(data),
            err_fn,
            None,
        ),

        f => unreachable!("find_config only returns i16 or f32 configs, got {f:?}"),
    }
}

pub struct AudioOut {
    ring: Ring,
    #[allow(dead_code)]
    stream: cpal::Stream,
    target: usize,
}

impl AudioOut {
    pub fn new() -> Option<AudioOut> {
        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            eprintln!("audio: no output device, running silent");
            return None;
        };
        let Some(config) = find_config(&device) else {
            eprintln!("audio: no 48 kHz stereo output config, running silent");
            return None;
        };

        let ring = Ring::new(CAPACITY);

        // One 60 Hz frame per period keeps delivery smooth; a backend that
        // refuses it gets its default buffering and a deeper queue instead.
        let (stream, target) = match build_stream(
            &device,
            &config,
            ring.clone(),
            cpal::BufferSize::Fixed(PERIOD),
        ) {
            Ok(s) => (s, QUEUE_TARGET),
            Err(e) => {
                eprintln!("audio: {PERIOD}-frame periods refused ({e}), buffering deeper");

                match build_stream(&device, &config, ring.clone(), cpal::BufferSize::Default) {
                    Ok(s) => (s, FALLBACK_TARGET),
                    Err(e) => {
                        eprintln!("audio: failed to build output stream: {e}");
                        return None;
                    }
                }
            }
        };

        if let Err(e) = stream.play() {
            eprintln!("audio: failed to start stream: {e}");
            return None;
        }

        Some(AudioOut {
            ring,
            stream,
            target,
        })
    }

    pub fn target(&self) -> usize {
        self.target
    }

    pub fn push(&self, samples: &[i16]) {
        self.ring.push(samples);
    }

    pub fn queued(&self) -> usize {
        self.ring.queued()
    }
}

#[derive(Clone)]
struct Ring {
    buf: Arc<Mutex<VecDeque<i16>>>,
    capacity: usize,
}

impl Ring {
    fn new(capacity: usize) -> Ring {
        Ring {
            buf: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            capacity,
        }
    }

    fn pop_slice(&self, out: &mut [i16]) {
        let mut buf = self.buf.lock().unwrap();

        for slot in out {
            *slot = buf.pop_front().unwrap_or(0);
        }
    }

    fn pop_slice_f32(&self, out: &mut [f32]) {
        let mut buf = self.buf.lock().unwrap();

        for slot in out {
            *slot = buf.pop_front().unwrap_or(0) as f32 / 32768.0;
        }
    }

    fn push(&self, samples: &[i16]) {
        let mut buf = self.buf.lock().unwrap();

        buf.extend(samples);
        let over = buf.len().saturating_sub(self.capacity);
        let over = over + (over & 1);
        buf.drain(..over);
    }

    fn queued(&self) -> usize {
        self.buf.lock().unwrap().len() / 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pop(ring: &Ring) -> i16 {
        let mut s = [0i16; 1];
        ring.pop_slice(&mut s);
        s[0]
    }

    #[test]
    fn ring_drops_oldest_when_full_and_yields_silence_when_empty() {
        let ring = Ring::new(4);

        assert_eq!(pop(&ring), 0, "empty ring must yield silence");

        ring.push(&[1, 2, 3, 4]);
        ring.push(&[5, 6]);

        assert_eq!(
            [pop(&ring), pop(&ring), pop(&ring), pop(&ring)],
            [3, 4, 5, 6]
        );
    }

    #[test]
    fn push_rounds_the_drop_oldest_drain_up_to_an_even_count() {
        let ring = Ring::new(4);

        ring.push(&[1, 2, 3, 4]);
        ring.push(&[5, 6, 7]);

        assert_eq!(
            [pop(&ring), pop(&ring), pop(&ring), pop(&ring)],
            [5, 6, 7, 0],
            "an odd drain count would flip which samples land on L vs R"
        );
    }

    #[test]
    fn pop_slice_fills_from_the_ring_and_zero_fills_any_shortfall() {
        let ring = Ring::new(8);

        ring.push(&[1, 2, 3]);

        let mut out = [9i16; 5];
        ring.pop_slice(&mut out);

        assert_eq!(out, [1, 2, 3, 0, 0]);
    }

    #[test]
    fn queued_counts_stereo_frames_not_samples() {
        let ring = Ring::new(8);

        ring.push(&[1, 2, 3, 4]);

        assert_eq!(ring.queued(), 2);
    }

    #[test]
    fn pop_slice_f32_converts_i16_to_the_minus_one_to_one_range() {
        let ring = Ring::new(8);

        ring.push(&[i16::MIN, 0, i16::MAX]);

        let mut out = [9.0f32; 4];
        ring.pop_slice_f32(&mut out);

        assert_eq!(out, [-1.0, 0.0, 32767.0 / 32768.0, 0.0]);
    }
}

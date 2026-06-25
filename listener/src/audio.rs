//! Cross-platform audio capture via cpal (CoreAudio / WASAPI / ALSA-PipeWire).
//! Streams mono f32 samples (channel 0) over an mpsc channel; the caller runs
//! the detector. Detection cost is trivial so a simple channel is fine.

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc::{channel, Receiver};

pub struct Capture {
    pub sample_rate: u32,
    pub rx: Receiver<Vec<f32>>,
    _stream: cpal::Stream, // kept alive for the duration
}

pub fn list_devices() -> Result<()> {
    let host = cpal::default_host();
    let def = host
        .default_input_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_else(|| "<none>".into());
    println!("default input: {def}");
    println!("input devices:");
    for d in host.input_devices()? {
        if let Ok(name) = d.name() {
            let sr = d
                .default_input_config()
                .map(|c| c.sample_rate().0)
                .unwrap_or(0);
            println!("  - {name}  ({sr} Hz)");
        }
    }
    Ok(())
}

pub fn start(device_name: Option<&str>) -> Result<Capture> {
    let host = cpal::default_host();
    let device = match device_name {
        Some(want) => host
            .input_devices()?
            .find(|d| d.name().map(|n| n.contains(want)).unwrap_or(false))
            .ok_or_else(|| anyhow!("no input device matching {want:?}"))?,
        None => host
            .default_input_device()
            .ok_or_else(|| anyhow!("no default input device"))?,
    };
    let supported = device.default_input_config()?;
    let sample_rate = supported.sample_rate().0;
    let channels = supported.channels() as usize;
    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let err_fn = |e| eprintln!("audio stream error: {e}");

    macro_rules! build {
        ($t:ty, $conv:expr) => {{
            let tx = {
                let (tx, rx) = channel::<Vec<f32>>();
                let stream = device.build_input_stream(
                    &config,
                    move |data: &[$t], _| {
                        let mono: Vec<f32> =
                            data.chunks(channels).map(|c| $conv(c[0])).collect();
                        let _ = tx.send(mono);
                    },
                    err_fn,
                    None,
                )?;
                (stream, rx)
            };
            tx
        }};
    }

    let (stream, rx) = match sample_format {
        cpal::SampleFormat::F32 => build!(f32, |x: f32| x),
        cpal::SampleFormat::I16 => build!(i16, |x: i16| x as f32 / 32768.0),
        cpal::SampleFormat::U16 => build!(u16, |x: u16| (x as f32 - 32768.0) / 32768.0),
        other => return Err(anyhow!("unsupported sample format: {other:?}")),
    };
    stream.play()?;
    Ok(Capture {
        sample_rate,
        rx,
        _stream: stream,
    })
}

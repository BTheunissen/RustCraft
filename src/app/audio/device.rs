//! CPAL/CoreAudio ownership on one worker; device errors never fail gameplay or saves.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig, SupportedStreamConfig};

use super::callback::Callback;
use super::catalog::SampleBank;
use super::shared::Shared;

pub(super) enum DeviceStatus {
    Ready {
        name: String,
        rate: u32,
        channels: u16,
        bytes: usize,
    },
    Unavailable(String),
}

pub(super) fn run(shared: Arc<Shared>, stop: Receiver<()>, status: SyncSender<DeviceStatus>) {
    let result = initialize(&shared);
    let stream = match result {
        Ok((stream, ready)) => {
            if shared.stopped() {
                return;
            }
            if let Err(error) = stream.play() {
                shared.failed.store(true, Ordering::Release);
                let _ = status.try_send(DeviceStatus::Unavailable(error.to_string()));
                return;
            }
            let _ = status.try_send(ready);
            shared.ready.store(true, Ordering::Release);
            stream
        }
        Err(error) => {
            shared.failed.store(true, Ordering::Release);
            let _ = status.try_send(DeviceStatus::Unavailable(error));
            return;
        }
    };
    loop {
        if shared.stopped() {
            break;
        }
        if shared.failed.load(Ordering::Acquire) {
            let _ = status.try_send(DeviceStatus::Unavailable(
                "audio stream failed; restart RustCraft to retry".into(),
            ));
            break;
        }
        match stop.recv_timeout(Duration::from_millis(20)) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
    let _ = stream.pause();
    drop(stream);
    shared.ready.store(false, Ordering::Release);
}

fn initialize(shared: &Arc<Shared>) -> Result<(cpal::Stream, DeviceStatus), String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("no output audio device")?;
    let supported = configuration(&device)?;
    let format = supported.sample_format();
    let config = supported.config();
    let name = device.to_string();
    let bank = Arc::new(SampleBank::generate());
    let bytes = bank.bytes();
    if shared.stopped() {
        return Err("audio initialization cancelled".into());
    }
    let callback = Callback::new(
        Arc::clone(shared),
        bank,
        config.sample_rate,
        config.channels,
    )?;
    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, config, callback, shared),
        SampleFormat::I16 => build::<i16>(&device, config, callback, shared),
        SampleFormat::U16 => build::<u16>(&device, config, callback, shared),
        _ => return Err("unsupported audio sample format".into()),
    }
    .map_err(|error| error.to_string())?;
    Ok((
        stream,
        DeviceStatus::Ready {
            name,
            rate: config.sample_rate,
            channels: config.channels,
            bytes,
        },
    ))
}

fn configuration(device: &cpal::Device) -> Result<SupportedStreamConfig, String> {
    let default = device
        .default_output_config()
        .map_err(|error| error.to_string())?;
    if acceptable(&default) {
        return Ok(default);
    }
    let mut configs = device
        .supported_output_configs()
        .map_err(|error| error.to_string())?;
    configs
        .find_map(|range| {
            let rate = 48_000_u32.clamp(range.min_sample_rate(), range.max_sample_rate());
            range.try_with_sample_rate(rate).filter(acceptable)
        })
        .ok_or_else(|| "no supported mono/stereo f32/i16/u16 output".into())
}

fn acceptable(config: &SupportedStreamConfig) -> bool {
    (1..=2).contains(&config.channels())
        && (8_000..=192_000).contains(&config.sample_rate())
        && matches!(
            config.sample_format(),
            SampleFormat::F32 | SampleFormat::I16 | SampleFormat::U16
        )
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    mut callback: Callback,
    shared: &Arc<Shared>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample + FromSample<f32>,
{
    let shared = Arc::clone(shared);
    device.build_output_stream(
        config,
        move |data: &mut [T], _| callback.render(data, Instant::now()),
        move |_| {
            shared.failed.store(true, Ordering::Release);
        },
        Some(Duration::from_millis(500)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpal::SupportedBufferSize;

    #[test]
    fn unsupported_devices_fail_closed_without_unbounded_format_assumptions() {
        for (channels, rate, format, expected) in [
            (2, 48_000, SampleFormat::F32, true),
            (1, 44_100, SampleFormat::I16, true),
            (2, 48_000, SampleFormat::U16, true),
            (0, 48_000, SampleFormat::F32, false),
            (6, 48_000, SampleFormat::F32, false),
            (2, 0, SampleFormat::F32, false),
            (2, u32::MAX, SampleFormat::F32, false),
            (2, 48_000, SampleFormat::F64, false),
        ] {
            let config =
                SupportedStreamConfig::new(channels, rate, SupportedBufferSize::Unknown, format);
            assert_eq!(acceptable(&config), expected);
        }
    }
}

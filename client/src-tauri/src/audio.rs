use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SizedSample};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

pub struct Recorded {
    pub samples: Vec<f32>,
    pub rate: u32,
}

impl Recorded {
    pub fn duration_secs(&self) -> f32 {
        if self.rate == 0 {
            return 0.0;
        }
        self.samples.len() as f32 / self.rate as f32
    }
}

enum AudioCmd {
    Start {
        device: Option<String>,
        app: AppHandle,
        resp: SyncSender<Result<(), String>>,
    },
    Stop {
        resp: SyncSender<Result<Recorded, String>>,
    },
    Cancel,
}

/// cpal streams are not Send, so a dedicated thread owns the stream and is
/// driven through this channel-based handle.
pub struct AudioHandle {
    tx: Sender<AudioCmd>,
}

impl AudioHandle {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || run_loop(rx));
        Self { tx }
    }

    pub fn start(&self, device: Option<String>, app: AppHandle) -> Result<(), String> {
        let (resp_tx, resp_rx) = mpsc::sync_channel(1);
        self.tx
            .send(AudioCmd::Start { device, app, resp: resp_tx })
            .map_err(|e| e.to_string())?;
        resp_rx.recv().map_err(|e| e.to_string())?
    }

    pub fn stop(&self) -> Result<Recorded, String> {
        let (resp_tx, resp_rx) = mpsc::sync_channel(1);
        self.tx
            .send(AudioCmd::Stop { resp: resp_tx })
            .map_err(|e| e.to_string())?;
        resp_rx.recv().map_err(|e| e.to_string())?
    }

    pub fn cancel(&self) {
        let _ = self.tx.send(AudioCmd::Cancel);
    }
}

struct Active {
    _stream: cpal::Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    rate: u32,
}

fn run_loop(rx: Receiver<AudioCmd>) {
    let mut active: Option<Active> = None;
    while let Ok(cmd) = rx.recv() {
        match cmd {
            AudioCmd::Start { device, app, resp } => {
                if active.is_some() {
                    let _ = resp.send(Err("Aufnahme läuft bereits".into()));
                    continue;
                }
                match start_stream(device, app) {
                    Ok(a) => {
                        active = Some(a);
                        let _ = resp.send(Ok(()));
                    }
                    Err(e) => {
                        let _ = resp.send(Err(e));
                    }
                }
            }
            AudioCmd::Stop { resp } => match active.take() {
                Some(Active { _stream, samples, rate }) => {
                    drop(_stream);
                    let samples = Arc::try_unwrap(samples)
                        .map(|m| m.into_inner().unwrap_or_default())
                        .unwrap_or_else(|arc| arc.lock().unwrap().clone());
                    let _ = resp.send(Ok(Recorded { samples, rate }));
                }
                None => {
                    let _ = resp.send(Err("Keine aktive Aufnahme".into()));
                }
            },
            AudioCmd::Cancel => {
                active = None;
            }
        }
    }
}

pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    host.input_devices()
        .map(|devices| devices.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

fn start_stream(device_name: Option<String>, app: AppHandle) -> Result<Active, String> {
    let host = cpal::default_host();
    let device = match &device_name {
        Some(name) => host
            .input_devices()
            .map_err(|e| e.to_string())?
            .find(|d| d.name().map(|n| &n == name).unwrap_or(false))
            .ok_or_else(|| format!("Mikrofon „{name}“ nicht gefunden"))?,
        None => host
            .default_input_device()
            .ok_or_else(|| "Kein Mikrofon gefunden".to_string())?,
    };
    let supported = device.default_input_config().map_err(|e| e.to_string())?;
    let rate = supported.sample_rate().0;
    let channels = supported.channels();
    let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let config: cpal::StreamConfig = supported.config();

    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build::<f32>(&device, &config, channels, samples.clone(), app),
        cpal::SampleFormat::I16 => build::<i16>(&device, &config, channels, samples.clone(), app),
        cpal::SampleFormat::U16 => build::<u16>(&device, &config, channels, samples.clone(), app),
        other => Err(format!("Nicht unterstütztes Sample-Format: {other:?}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(Active { _stream: stream, samples, rate })
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: u16,
    samples: Arc<Mutex<Vec<f32>>>,
    app: AppHandle,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let rate = config.sample_rate.0 as usize;
    let mut since_emit: usize = 0;
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                let mut sum_sq = 0f32;
                let mut count = 0usize;
                {
                    let mut buf = samples.lock().unwrap();
                    for frame in data.chunks(channels as usize) {
                        let mono = frame
                            .iter()
                            .map(|s| f32::from_sample(*s))
                            .sum::<f32>()
                            / channels as f32;
                        buf.push(mono);
                        sum_sq += mono * mono;
                        count += 1;
                    }
                }
                since_emit += count;
                // Level updates for the overlay meter, roughly every 50 ms.
                if since_emit >= rate / 20 && count > 0 {
                    since_emit = 0;
                    let rms = (sum_sq / count as f32).sqrt();
                    let _ = app.emit_to("overlay", "level", rms);
                }
            },
            |err| eprintln!("audio stream error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())
}

/// Resamples to 16 kHz mono and encodes as 16-bit PCM WAV.
pub fn to_wav_16k(recorded: &Recorded) -> Vec<u8> {
    const TARGET: u32 = 16_000;
    let src = &recorded.samples;
    let resampled: Vec<f32> = if recorded.rate == TARGET || src.is_empty() {
        src.clone()
    } else {
        let ratio = recorded.rate as f64 / TARGET as f64;
        let out_len = (src.len() as f64 / ratio) as usize;
        (0..out_len)
            .map(|i| {
                let pos = i as f64 * ratio;
                let idx = pos as usize;
                let frac = (pos - idx as f64) as f32;
                let a = src[idx.min(src.len() - 1)];
                let b = *src.get(idx + 1).unwrap_or(&a);
                a + (b - a) * frac
            })
            .collect()
    };

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec).expect("wav writer");
        for s in &resampled {
            let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            let _ = writer.write_sample(v);
        }
        writer.finalize().expect("wav finalize");
    }
    cursor.into_inner()
}

// https://www.reddit.com/r/rust/comments/173bk09/need_help_recording_audio_to_file/
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample};
use hound::WavWriter;
use thiserror::Error;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::error::Error;

pub struct Recorder {
    utils: Option<(Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>, cpal::Stream)>,
    selected_input: Option<usize>,
}

#[derive(Error, Debug)]
pub enum RecordingError {
    #[error("{0}")]
    Message(String)
}
impl RecordingError {
    pub fn msg(input: &str) -> Self {
        Self::Message(String::from(input))
    }
    pub fn box_msg(input: &str) -> Box<Self> {
        Box::new(Self::msg(input))
    }
}

impl Recorder {
    pub fn new() -> Self {
        Recorder { utils: None, selected_input: None }
    }
    pub fn get_input_devices(&self) -> Result<Vec<(usize, String)>, Box<dyn Error>> {
        let host = cpal::default_host();
        let mut device_names = Vec::new();
        for (i, device) in host.devices()?.into_iter().enumerate() {
            let configs = device.supported_input_configs()?;
            if let Some(_) = configs.peekable().peek() {
                device_names.push((i, device.name()?));
            }
        }
        //Ok(host.devices()?.into_iter().map(|d| d.name()).collect::<Result<Vec<_>, _>>()?)
        Ok(device_names)
    }
    pub fn set_preferred_input_device(&mut self, index: usize) {
        self.selected_input = Some(index);
    }
    pub fn start_recording(&mut self, save_location: &Path) -> Result<(), Box<dyn Error>> {
        if self.utils.is_some() {
            return Err(RecordingError::box_msg(
                "Attempted to start recording when already recording!",
            ));
        }

        let host = cpal::default_host();

        // Set up the input device and stream with the default input config.
        let mut input_device = None;
        if let Some(preferred_idx) = &self.selected_input {
            let devices = host.devices()?;
            let mut vec = devices.collect::<Vec<cpal::Device>>();
            if let Some(_) = vec.get(*preferred_idx) {
                input_device = Some(vec.remove(*preferred_idx));
            }
        }
        let device = 
            if let Some(dev) = input_device {
                dev
            } else {
                host.default_input_device().expect("failed to find input device")
            };
        println!("Input device: {}", device.name()?);

        let config = device
            .default_input_config()
            .expect("Failed to get default input config");
        println!("Default input config: {:?}", config);

        // The WAV file we're recording to.
        let spec = wav_spec_from_config(&config);
        let writer = hound::WavWriter::create(save_location, spec)?;
        let writer = Arc::new(Mutex::new(Some(writer)));

        // A flag to indicate that recording is in progress.
        // println!("Begin recording...");

        // Run the input stream on a separate thread.
        let writer_2 = writer.clone();

        let err_fn = move |err| {
            eprintln!("an error occurred on stream: {}", err);
        };

        let stream = match config.sample_format() {
            cpal::SampleFormat::I8 => device.build_input_stream(
                &config.into(),
                move |data, _: &_| write_input_data::<i8, i8>(data, &writer_2),
                err_fn,
                None,
            )?,
            cpal::SampleFormat::I16 => device.build_input_stream(
                &config.into(),
                move |data, _: &_| write_input_data::<i16, i16>(data, &writer_2),
                err_fn,
                None,
            )?,
            cpal::SampleFormat::I32 => device.build_input_stream(
                &config.into(),
                move |data, _: &_| write_input_data::<i32, i32>(data, &writer_2),
                err_fn,
                None,
            )?,
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |data, _: &_| write_input_data::<f32, f32>(data, &writer_2),
                err_fn,
                None,
            )?,
            sample_format => {
                return Err(Box::new(RecordingError::Message(format!(
                    "Unsupported sample format '{sample_format}'"
                ))))
            }
        };
        // ========================

        stream.play().unwrap();
        self.utils = Some((writer, stream));
        Ok(())
    }
    pub fn stop_recording(&mut self) -> Result<(), Box<dyn Error>> {
        match self.utils.take() {
            Some((writer, stream)) => {
                stream.pause().unwrap();
                writer.lock().unwrap().take().unwrap().finalize().unwrap();
                Ok(())
            }
            None => {
                return Err(RecordingError::box_msg(
                    "Attempted to stop recording when not recording!",
                ));
            }
        }
    }
}

fn sample_format(format: cpal::SampleFormat) -> hound::SampleFormat {
    if format.is_float() {
        hound::SampleFormat::Float
    } else {
        hound::SampleFormat::Int
    }
}

fn wav_spec_from_config(config: &cpal::SupportedStreamConfig) -> hound::WavSpec {
    hound::WavSpec {
        channels: config.channels() as _,
        sample_rate: config.sample_rate().0 as _,
        bits_per_sample: (config.sample_format().sample_size() * 8) as _,
        sample_format: sample_format(config.sample_format()),
    }
}

type WavWriterHandle = Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>;

fn write_input_data<T, U>(input: &[T], writer: &WavWriterHandle)
where
    T: Sample,
    U: Sample + hound::Sample + FromSample<T>,
{
    if let Ok(mut guard) = writer.try_lock() {
        if let Some(writer) = guard.as_mut() {
            for &sample in input.iter() {
                let sample: U = U::from_sample(sample);
                writer.write_sample(sample).ok();
            }
        }
    }
}

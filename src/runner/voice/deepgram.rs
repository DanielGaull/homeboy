use audio::channel::LinearChannel;
use audio::Buf;
use bytes::BytesMut;
use deepgram::common::audio_source::AudioSource;
use deepgram::common::options::Language;
use deepgram::speak::options::{Container, Encoding, Model};
use deepgram::{speak::options::Options, Deepgram};
use futures::stream::StreamExt;
use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, Sink};
use std::env;
use std::error::Error;
use tokio::fs::File;
use std::path::Path;

#[derive(PartialEq)]
pub enum OutputMode {
    Voice,
    Console,
}

pub struct DeepgramClient {
    client: Deepgram,
    output_mode: OutputMode,
}

impl DeepgramClient {
    pub fn init(output_mode: OutputMode) -> Result<Self, Box<dyn Error>> {
        let client = Deepgram::new(env::var(String::from("deepgram_api_secret"))?)?;
        Ok(
            DeepgramClient {
                client,
                output_mode,
            }
        )
    }

    pub async fn transcribe(&self, filepath: &Path) -> Result<String, Box<dyn Error>> {
        let file = File::open(filepath).await?;
        let source = AudioSource::from_buffer_with_mime_type(file, "audio/wav");
        let options = deepgram::common::options::Options::builder()
            .punctuate(true)
            .language(Language::en_US)
            .build();

        let response = self.client
            .transcription()
            .prerecorded(source, &options)
            .await?;
        
        let transcript = &response.results.channels[0].alternatives[0].transcript;

        Ok(transcript.clone())
    }

    pub async fn speak(&self, text: &str) -> Result<(), Box<dyn Error>> {
        if self.output_mode == OutputMode::Console {
            println!("Response: {}", text);
        } else if self.output_mode == OutputMode::Voice {
            self.do_speak(text).await?;
        }
        Ok(())
    }

    pub async fn do_speak(&self, text: &str) -> Result<(), Box<dyn Error>> {
        let sample_rate = 16000;
        let channels = 1;

        let options = Options::builder()
            .model(Model::AuraAsteriaEn)
            .encoding(Encoding::Linear16)
            .sample_rate(sample_rate)
            .container(Container::Wav)
            .build();

        let audio_stream = self.client
            .text_to_speech()
            .speak_to_stream(text, &options)
            .await?;

        // Set up audio output
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();

        // Create the audio source
        let mut source = Linear16AudioSource::new(sample_rate, channels);

        // Use the audio_stream for streaming audio and play it
        let mut stream = audio_stream;
        let mut buffer = BytesMut::new();
        let mut extra_byte: Option<u8> = None;

        // Define a threshold for the buffer (e.g., 32000 bytes for 1 second)
        let buffer_threshold = 0; // increase for slow networks

        // Accumulate initial buffer
        while let Some(data) = stream.next().await {
            // Process and accumulate the audio data here
            buffer.extend_from_slice(&data);

            // Prepend the extra byte if present
            if let Some(byte) = extra_byte.take() {
                let mut new_buffer = BytesMut::with_capacity(buffer.len() + 1);
                new_buffer.extend_from_slice(&[byte]);
                new_buffer.extend_from_slice(&buffer);
                buffer = new_buffer;
            }

            // Check if buffer has reached the initial threshold
            if buffer.len() >= buffer_threshold {
                // Convert buffer to i16 samples and push to source
                if buffer.len() % 2 != 0 {
                    extra_byte = Some(buffer.split_off(buffer.len() - 1)[0]);
                }

                let samples: Vec<i16> = buffer
                    .chunks_exact(2)
                    .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect();
                source.push_samples(&samples);

                // Start playing the audio
                play_audio(&sink, sample_rate, channels, source.take_buffer());

                // Clear the buffer
                buffer.clear();
            }
        }

        // Play any remaining buffered data
        if !buffer.is_empty() {
            // Prepend the extra byte if present
            if let Some(byte) = extra_byte {
                let mut new_buffer = BytesMut::with_capacity(buffer.len() + 1);
                new_buffer.extend_from_slice(&[byte]);
                new_buffer.extend_from_slice(&buffer);
                buffer = new_buffer;
            }

            let samples: Vec<i16> = buffer
                .chunks_exact(2)
                .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
                .collect();
            source.push_samples(&samples);

            // Play the remaining audio
            play_audio(&sink, sample_rate, channels, source.take_buffer());
        }

        // Ensure all audio is played before exiting
        sink.sleep_until_end();

        Ok(())
    }
}

fn play_audio(sink: &Sink, sample_rate: u32, channels: u16, samples: Vec<i16>) {
    // Create a rodio source from the raw audio data
    let source = SamplesBuffer::new(channels, sample_rate, samples);

    // Play the audio
    sink.append(source);
}

#[derive(Clone)]
pub struct Linear16AudioSource {
    sample_rate: u32,
    channels: u16,
    buffer: Vec<i16>,
}

impl Linear16AudioSource {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            sample_rate,
            channels,
            buffer: Vec::new(),
        }
    }

    pub fn push_samples(&mut self, samples: &[i16]) {
        self.buffer.extend_from_slice(samples);
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn take_buffer(&mut self) -> Vec<i16> {
        std::mem::take(&mut self.buffer)
    }
}

impl Buf for Linear16AudioSource {
    type Sample = i16;

    type Channel<'this>
        = LinearChannel<'this, i16>
    where
        Self: 'this;

    type IterChannels<'this>
        = std::vec::IntoIter<LinearChannel<'this, i16>>
    where
        Self: 'this;

    fn frames_hint(&self) -> Option<usize> {
        Some(self.buffer.len() / self.channels as usize)
    }

    fn channels(&self) -> usize {
        self.channels as usize
    }

    fn get_channel(&self, channel: usize) -> Option<Self::Channel<'_>> {
        if channel < self.channels as usize {
            Some(LinearChannel::new(&self.buffer[channel..]))
        } else {
            None
        }
    }

    fn iter_channels(&self) -> Self::IterChannels<'_> {
        (0..self.channels as usize)
            .map(|channel| LinearChannel::new(&self.buffer[channel..]))
            .collect::<Vec<_>>()
            .into_iter()
    }
}

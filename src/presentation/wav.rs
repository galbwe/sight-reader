//! WAV output: synthesizes the exercise's MIDI data with a SoundFont (via `rustysynth`)
//! and writes a 16-bit stereo PCM WAV file by hand.

use std::fs::File;
use std::io::{BufReader, Cursor};
use std::sync::Arc;

use rustysynth::{MidiFile, MidiFileSequencer, SoundFont, Synthesizer, SynthesizerSettings};

use super::midi::MidiRenderer;
use crate::domain::Measure;

/// Samples per second of the WAV file
pub const SAMPLE_RATE: u32 = 44_100;
/// Stereo
const CHANNELS: u16 = 2;
const BITS_PER_SAMPLE: u16 = 16;
const BYTES_PER_FRAME: u32 = (CHANNELS as u32) * (BITS_PER_SAMPLE as u32 / 8);
/// Size of the RIFF header plus the `fmt ` and `data` chunk headers
const HEADER_LEN: usize = 44;
/// Seconds rendered after the last note-off, so the final note can ring out
pub const RELEASE_TAIL_SECONDS: f64 = 1.5;

/// Convert a sample to 16 bits, clamping it to the range -1 to 1 first
fn to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

/// Scale both channels down so the peak is 1.0, if it is above that. Quieter audio is untouched.
fn normalize(left: &mut [f32], right: &mut [f32]) {
    let peak = left
        .iter()
        .chain(right.iter())
        .fold(0.0f32, |p, s| p.max(s.abs()));
    if peak > 1.0 {
        for sample in left.iter_mut().chain(right.iter_mut()) {
            *sample /= peak;
        }
    }
}

/// A complete WAV file: 16-bit PCM, two channels, interleaved left then right
pub fn wav_bytes(left: &[f32], right: &[f32], sample_rate: u32) -> Vec<u8> {
    let frames = left.len().min(right.len());
    let data_len = frames as u32 * BYTES_PER_FRAME;
    let mut bytes = Vec::with_capacity(HEADER_LEN + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(HEADER_LEN as u32 - 8 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&CHANNELS.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * BYTES_PER_FRAME).to_le_bytes());
    bytes.extend_from_slice(&(BYTES_PER_FRAME as u16).to_le_bytes());
    bytes.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..frames {
        bytes.extend_from_slice(&to_i16(left[i]).to_le_bytes());
        bytes.extend_from_slice(&to_i16(right[i]).to_le_bytes());
    }
    bytes
}

/// Renders an exercise to WAV audio. Not a `NotationRenderer`, which returns a `String`.
pub struct WavRenderer {
    midi: MidiRenderer,
    soundfont_path: String,
}

impl WavRenderer {
    pub fn new(midi: MidiRenderer, soundfont_path: &str) -> Self {
        WavRenderer {
            midi,
            soundfont_path: soundfont_path.to_string(),
        }
    }

    fn load_soundfont(&self) -> Result<Arc<SoundFont>, String> {
        let path = &self.soundfont_path;
        let file =
            File::open(path).map_err(|e| format!("cannot read SoundFont {}: {}", path, e))?;
        let soundfont = SoundFont::new(&mut BufReader::new(file))
            .map_err(|e| format!("{} is not a valid SoundFont: {}", path, e))?;
        Ok(Arc::new(soundfont))
    }

    /// Synthesize the exercise and return the bytes of the WAV file
    pub fn to_bytes(&self, measures: &[Measure]) -> Result<Vec<u8>, String> {
        let soundfont = self.load_soundfont()?;
        let midi_bytes = self.midi.to_bytes(measures);
        let midi_file = Arc::new(
            MidiFile::new(&mut Cursor::new(midi_bytes))
                .map_err(|e| format!("cannot read the generated MIDI data: {}", e))?,
        );
        let settings = SynthesizerSettings::new(SAMPLE_RATE as i32);
        let synthesizer = Synthesizer::new(&soundfont, &settings)
            .map_err(|e| format!("cannot use SoundFont {}: {}", self.soundfont_path, e))?;
        let mut sequencer = MidiFileSequencer::new(synthesizer);
        sequencer.play(&midi_file, false);

        let frames =
            ((midi_file.get_length() + RELEASE_TAIL_SECONDS) * SAMPLE_RATE as f64) as usize;
        let mut left = vec![0f32; frames];
        let mut right = vec![0f32; frames];
        sequencer.render(&mut left, &mut right);
        normalize(&mut left, &mut right);
        Ok(wav_bytes(&left, &right, SAMPLE_RATE))
    }

    /// Synthesize the exercise and write it to `path`
    pub fn save_to_file(&self, measures: &[Measure], path: &str) -> Result<(), String> {
        let bytes = self.to_bytes(measures)?;
        std::fs::write(path, bytes).map_err(|e| format!("cannot write {}: {}", path, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Key, KeyMode, Note, NoteDuration, TimeSignature};

    fn u32_at(b: &[u8], i: usize) -> u32 {
        u32::from_le_bytes(b[i..i + 4].try_into().unwrap())
    }
    fn u16_at(b: &[u8], i: usize) -> u16 {
        u16::from_le_bytes(b[i..i + 2].try_into().unwrap())
    }
    fn i16_at(b: &[u8], i: usize) -> i16 {
        i16::from_le_bytes(b[i..i + 2].try_into().unwrap())
    }

    #[test]
    fn test_header_fields() {
        let bytes = wav_bytes(&[0.0, 0.5, -0.5], &[0.0, 0.25, -0.25], 44_100);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(u32_at(&bytes, 4) as usize, bytes.len() - 8);
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[12..16], b"fmt ");
        assert_eq!(u32_at(&bytes, 16), 16);
        assert_eq!(u16_at(&bytes, 20), 1);
        assert_eq!(u16_at(&bytes, 22), 2);
        assert_eq!(u32_at(&bytes, 24), 44_100);
        assert_eq!(u32_at(&bytes, 28), 44_100 * 4);
        assert_eq!(u16_at(&bytes, 32), 4);
        assert_eq!(u16_at(&bytes, 34), 16);
        assert_eq!(&bytes[36..40], b"data");
        assert_eq!(u32_at(&bytes, 40), 3 * 4);
        assert_eq!(bytes.len(), 44 + 12);
    }

    #[test]
    fn test_samples_are_interleaved_left_then_right() {
        let bytes = wav_bytes(&[1.0, 0.0], &[-1.0, 0.5], 44_100);
        assert_eq!(i16_at(&bytes, 44), i16::MAX);
        assert_eq!(i16_at(&bytes, 46), -i16::MAX);
        assert_eq!(i16_at(&bytes, 48), 0);
        assert_eq!(i16_at(&bytes, 50), (0.5 * i16::MAX as f32) as i16);
    }

    #[test]
    fn test_to_i16_clamps() {
        assert_eq!(to_i16(1.5), i16::MAX);
        assert_eq!(to_i16(-1.5), -i16::MAX);
        assert_eq!(to_i16(0.0), 0);
    }

    #[test]
    fn test_normalize_scales_only_when_the_peak_is_above_one() {
        let (mut l, mut r) = (vec![2.0, -1.0], vec![0.5, 1.0]);
        normalize(&mut l, &mut r);
        assert_eq!(l, vec![1.0, -0.5]);
        assert_eq!(r, vec![0.25, 0.5]);

        let (mut l, mut r) = (vec![0.5, -0.25], vec![0.1, 0.0]);
        normalize(&mut l, &mut r);
        assert_eq!(l, vec![0.5, -0.25]);
        assert_eq!(r, vec![0.1, 0.0]);
    }

    #[test]
    fn test_empty_input_is_a_valid_44_byte_file() {
        let bytes = wav_bytes(&[], &[], 44_100);
        assert_eq!(bytes.len(), 44);
        assert_eq!(u32_at(&bytes, 4), 36);
        assert_eq!(u32_at(&bytes, 40), 0);
    }

    fn two_measures() -> Vec<Measure> {
        let key = Key::new(60, KeyMode::Major);
        let make = || {
            let mut m = Measure::new(1, TimeSignature::common_time(), key.signature());
            for pitch in [60, 62, 64, 65] {
                m.add_note(Note::new(pitch, NoteDuration::Quarter));
            }
            m
        };
        vec![make(), make()]
    }

    #[test]
    fn test_missing_soundfont_is_an_error_naming_the_path() {
        let renderer = WavRenderer::new(MidiRenderer::new(), "does/not/exist.sf2");
        let message = renderer.to_bytes(&two_measures()).unwrap_err();
        assert!(message.contains("does/not/exist.sf2"), "{message}");
    }

    #[test]
    fn test_a_file_that_is_not_a_soundfont_is_an_error_naming_the_path() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
        let renderer = WavRenderer::new(MidiRenderer::new(), path);
        let message = renderer.to_bytes(&two_measures()).unwrap_err();
        assert!(message.contains(path), "{message}");
        assert!(message.contains("not a valid SoundFont"), "{message}");
    }

    /// Needs a real SoundFont: `SOUNDFONT=GeneralUser.sf2 cargo test -- --ignored`
    #[test]
    #[ignore]
    fn test_end_to_end_with_a_real_soundfont() {
        let Ok(path) = std::env::var("SOUNDFONT") else {
            eprintln!("SOUNDFONT is not set, skipping");
            return;
        };
        let renderer = WavRenderer::new(MidiRenderer::new().with_tempo(120), &path);
        let bytes = renderer.to_bytes(&two_measures()).unwrap();
        let frames = (u32_at(&bytes, 40) / 4) as f64;
        // 2 measures of 4/4 at 120 BPM = 4 seconds, plus the release tail
        let expected = (4.0 + RELEASE_TAIL_SECONDS) * SAMPLE_RATE as f64;
        assert!(
            (frames - expected).abs() < SAMPLE_RATE as f64 * 0.5,
            "{frames} vs {expected}"
        );
        assert!(bytes[44..].chunks(2).any(|c| c != [0, 0]));
    }
}

//! Standard MIDI File output.
//!
//! A pure byte writer with no dependencies: format 0, one track, 96 ticks per quarter note.
//! Timing comes from the tick helpers in `beaming.rs`, so dotted notes and triplets are timed
//! the same way they are drawn. It only reads domain types.

use super::beaming::{beat_ticks, duration_ticks, TICKS_PER_QUARTER};
use crate::domain::{KeyMode, Measure, TimeSignature};

/// Default speed in quarter notes per minute
pub const DEFAULT_TEMPO_BPM: u16 = 80;

/// Channel the exercise is played on (channel 1 in musician's numbering)
const NOTE_CHANNEL: u8 = 0;
/// General MIDI percussion channel (channel 10 in musician's numbering)
const PERCUSSION_CHANNEL: u8 = 9;
/// Velocity of exercise notes
const NOTE_VELOCITY: u8 = 80;
/// Velocity of count-in clicks
const CLICK_VELOCITY: u8 = 100;
/// General MIDI percussion note for the accented first click (Hi Wood Block)
const ACCENT_CLICK_NOTE: u8 = 76;
/// General MIDI percussion note for the other clicks (Low Wood Block)
const CLICK_NOTE: u8 = 77;
/// How long a click sounds, in ticks
const CLICK_TICKS: u32 = TICKS_PER_QUARTER / 4;

const MICROS_PER_MINUTE: u32 = 60_000_000;

const STATUS_NOTE_OFF: u8 = 0x80;
const STATUS_NOTE_ON: u8 = 0x90;
const STATUS_PROGRAM_CHANGE: u8 = 0xC0;
const META: u8 = 0xFF;
const META_TRACK_NAME: u8 = 0x03;
const META_END_OF_TRACK: u8 = 0x2F;
const META_TEMPO: u8 = 0x51;
const META_TIME_SIGNATURE: u8 = 0x58;
const META_KEY_SIGNATURE: u8 = 0x59;
/// MIDI clocks per metronome click, and 32nd notes per quarter note, as the spec suggests
const CLOCKS_PER_CLICK: u8 = 24;
const THIRTY_SECONDS_PER_QUARTER: u8 = 8;

const TRACK_NAME: &str = "Sight reading exercise";

/// A General MIDI instrument. To add one: a variant, its program number and its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instrument {
    AcousticGrandPiano,
}

impl Instrument {
    /// General MIDI program number (0 to 127)
    pub fn program(&self) -> u8 {
        match self {
            Instrument::AcousticGrandPiano => 0,
        }
    }

    /// Read an instrument from its name (any case): `piano`. For a future `--instrument` flag.
    #[allow(dead_code)]
    pub fn from_name(name: &str) -> Option<Instrument> {
        match name.trim().to_lowercase().as_str() {
            "piano" | "acoustic-grand-piano" => Some(Instrument::AcousticGrandPiano),
            _ => None,
        }
    }
}

/// Writes exercises as a Standard MIDI File.
///
/// Not a `NotationRenderer`: that trait returns a `String` and MIDI is binary, like the PDF.
#[derive(Debug, Clone)]
pub struct MidiRenderer {
    tempo_bpm: u16,
    count_in: bool,
    instrument: Instrument,
}

/// One event at an absolute tick. `order` sorts events that share a tick: note-offs (0) come
/// before note-ons (1) so a repeated pitch re-attacks cleanly.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TimedEvent {
    tick: u32,
    order: u8,
    bytes: Vec<u8>,
}

impl Default for MidiRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl MidiRenderer {
    pub fn new() -> Self {
        MidiRenderer {
            tempo_bpm: DEFAULT_TEMPO_BPM,
            count_in: false,
            instrument: Instrument::AcousticGrandPiano,
        }
    }

    /// Set the speed in quarter notes per minute
    pub fn with_tempo(mut self, tempo_bpm: u16) -> Self {
        self.tempo_bpm = tempo_bpm;
        self
    }

    /// Add one measure of metronome clicks before the exercise
    pub fn with_count_in(mut self, count_in: bool) -> Self {
        self.count_in = count_in;
        self
    }

    /// Choose the instrument. For a future `--instrument` flag.
    #[allow(dead_code)]
    pub fn with_instrument(mut self, instrument: Instrument) -> Self {
        self.instrument = instrument;
        self
    }

    /// The whole file as bytes
    pub fn to_bytes(&self, measures: &[Measure]) -> Vec<u8> {
        let first = measures.first();
        let time = first
            .map(|m| m.time_signature)
            .unwrap_or_else(TimeSignature::common_time);

        let mut track = Vec::new();
        self.push_header_events(&mut track, first);

        let mut events = Vec::new();
        let mut start = 0;
        if self.count_in {
            events.extend(count_in_events(&time));
            start = measure_ticks(&time);
        }
        events.extend(note_events(measures, start));
        events.sort_by_key(|e| (e.tick, e.order));

        let mut now = 0;
        for event in events {
            write_vlq(&mut track, event.tick - now);
            track.extend(event.bytes);
            now = event.tick;
        }
        write_vlq(&mut track, 0);
        track.extend([META, META_END_OF_TRACK, 0]);

        let mut file = Vec::new();
        file.extend(b"MThd");
        file.extend(6u32.to_be_bytes());
        file.extend(0u16.to_be_bytes()); // format 0
        file.extend(1u16.to_be_bytes()); // one track
        file.extend((TICKS_PER_QUARTER as u16).to_be_bytes());
        file.extend(b"MTrk");
        file.extend((track.len() as u32).to_be_bytes());
        file.extend(track);
        file
    }

    /// Write the file to disk
    pub fn save_to_file(&self, measures: &[Measure], path: &str) -> Result<(), String> {
        std::fs::write(path, self.to_bytes(measures))
            .map_err(|e| format!("cannot write {}: {}", path, e))
    }

    /// Meta events and the program change, all at tick 0
    fn push_header_events(&self, track: &mut Vec<u8>, first: Option<&Measure>) {
        let mut meta = |kind: u8, data: &[u8]| {
            track.extend([0, META, kind, data.len() as u8]);
            track.extend(data);
        };
        meta(META_TRACK_NAME, TRACK_NAME.as_bytes());
        meta(META_TEMPO, &tempo_micros(self.tempo_bpm).to_be_bytes()[1..]);
        if let Some(measure) = first {
            let time = measure.time_signature;
            meta(
                META_TIME_SIGNATURE,
                &[
                    time.numerator,
                    denominator_power(time.denominator),
                    CLOCKS_PER_CLICK,
                    THIRTY_SECONDS_PER_QUARTER,
                ],
            );
            let mode = match measure.key_signature.mode {
                KeyMode::Major => 0,
                KeyMode::Minor => 1,
            };
            meta(
                META_KEY_SIGNATURE,
                &[measure.key_signature.accidental_count as i8 as u8, mode],
            );
        }
        track.extend([
            0,
            STATUS_PROGRAM_CHANGE | NOTE_CHANNEL,
            self.instrument.program(),
        ]);
    }
}

/// Encode a variable-length quantity (the format of delta times)
fn write_vlq(out: &mut Vec<u8>, value: u32) {
    let mut groups = vec![(value & 0x7F) as u8];
    let mut rest = value >> 7;
    while rest > 0 {
        groups.push((rest & 0x7F) as u8 | 0x80);
        rest >>= 7;
    }
    out.extend(groups.into_iter().rev());
}

/// Microseconds per quarter note at a tempo
fn tempo_micros(bpm: u16) -> u32 {
    MICROS_PER_MINUTE / bpm.max(1) as u32
}

/// The `dd` of a time signature meta event: the denominator as a power of two
fn denominator_power(denominator: u8) -> u8 {
    denominator.max(1).ilog2() as u8
}

/// Length of one measure of a time signature in ticks
fn measure_ticks(time: &TimeSignature) -> u32 {
    time.numerator as u32 * 4 * TICKS_PER_QUARTER / time.denominator.max(1) as u32
}

/// One measure of clicks on the percussion channel, one per beat, the first accented
fn count_in_events(time: &TimeSignature) -> Vec<TimedEvent> {
    let beat = beat_ticks(time);
    (0..measure_ticks(time) / beat)
        .flat_map(|n| {
            let tick = n * beat;
            let key = if n == 0 {
                ACCENT_CLICK_NOTE
            } else {
                CLICK_NOTE
            };
            [
                TimedEvent {
                    tick,
                    order: 1,
                    bytes: vec![STATUS_NOTE_ON | PERCUSSION_CHANNEL, key, CLICK_VELOCITY],
                },
                TimedEvent {
                    tick: tick + CLICK_TICKS,
                    order: 0,
                    bytes: vec![STATUS_NOTE_OFF | PERCUSSION_CHANNEL, key, 0],
                },
            ]
        })
        .collect()
}

/// Note-on and note-off for every note, starting at `start_tick`
fn note_events(measures: &[Measure], start_tick: u32) -> Vec<TimedEvent> {
    let mut events = Vec::new();
    let mut tick = start_tick;
    for measure in measures {
        let mut onset = tick;
        for note in &measure.notes {
            let length = duration_ticks(note);
            events.push(TimedEvent {
                tick: onset,
                order: 1,
                bytes: vec![
                    STATUS_NOTE_ON | NOTE_CHANNEL,
                    note.midi_number,
                    NOTE_VELOCITY,
                ],
            });
            events.push(TimedEvent {
                tick: onset + length,
                order: 0,
                bytes: vec![STATUS_NOTE_OFF | NOTE_CHANNEL, note.midi_number, 0],
            });
            onset += length;
        }
        // A measure is as long as its time signature says, even if it is not quite full
        tick += measure_ticks(&measure.time_signature);
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{KeySignature, Note, NoteDuration};

    fn measure(number: u32, notes: &[(u8, NoteDuration)]) -> Measure {
        let mut m = Measure::new(
            number,
            TimeSignature::common_time(),
            KeySignature::new(60, KeyMode::Major),
        );
        m.notes = notes
            .iter()
            .map(|&(pitch, duration)| Note::new(pitch, duration))
            .collect();
        m
    }

    fn c_d_e() -> Measure {
        measure(
            1,
            &[
                (60, NoteDuration::Quarter),
                (62, NoteDuration::Half),
                (64, NoteDuration::Quarter),
            ],
        )
    }

    /// The track bytes after the 22-byte file and track headers
    fn track(bytes: &[u8]) -> &[u8] {
        &bytes[22..]
    }

    /// Decode the channel events (note on/off) as (absolute tick, status, key) triples
    fn channel_events(bytes: &[u8]) -> Vec<(u32, u8, u8)> {
        let data = track(bytes);
        let (mut i, mut tick, mut out) = (0, 0u32, Vec::new());
        while i < data.len() {
            let mut delta = 0u32;
            loop {
                let b = data[i];
                i += 1;
                delta = (delta << 7) | (b & 0x7F) as u32;
                if b & 0x80 == 0 {
                    break;
                }
            }
            tick += delta;
            match data[i] {
                META => i += 3 + data[i + 2] as usize,
                s if s & 0xF0 == STATUS_PROGRAM_CHANGE => i += 2,
                s => {
                    out.push((tick, s, data[i + 1]));
                    i += 3;
                }
            }
        }
        out
    }

    fn vlq(value: u32) -> Vec<u8> {
        let mut out = Vec::new();
        write_vlq(&mut out, value);
        out
    }

    #[test]
    fn test_vlq_matches_the_spec_examples() {
        assert_eq!(vlq(0), [0x00]);
        assert_eq!(vlq(0x7F), [0x7F]);
        assert_eq!(vlq(0x80), [0x81, 0x00]);
        assert_eq!(vlq(0x2000), [0xC0, 0x00]);
        assert_eq!(vlq(0x3FFF), [0xFF, 0x7F]);
        assert_eq!(vlq(0x0FFF_FFFF), [0xFF, 0xFF, 0xFF, 0x7F]);
    }

    #[test]
    fn test_header_bytes() {
        let bytes = MidiRenderer::new().to_bytes(&[c_d_e()]);
        assert_eq!(&bytes[0..4], b"MThd");
        assert_eq!(&bytes[4..8], &6u32.to_be_bytes());
        assert_eq!(&bytes[8..10], &0u16.to_be_bytes());
        assert_eq!(&bytes[10..12], &1u16.to_be_bytes());
        assert_eq!(&bytes[12..14], &96u16.to_be_bytes());
        assert_eq!(&bytes[14..18], b"MTrk");
    }

    #[test]
    fn test_track_length_matches_the_chunk() {
        let bytes = MidiRenderer::new()
            .with_count_in(true)
            .to_bytes(&[c_d_e(), c_d_e()]);
        let length = u32::from_be_bytes(bytes[18..22].try_into().unwrap()) as usize;
        assert_eq!(length, bytes.len() - 22);
        assert_eq!(&bytes[bytes.len() - 3..], &[META, META_END_OF_TRACK, 0]);
    }

    #[test]
    fn test_tempo() {
        assert_eq!(tempo_micros(80), 750_000);
        assert_eq!(tempo_micros(120), 500_000);
        let bytes = MidiRenderer::new().with_tempo(120).to_bytes(&[c_d_e()]);
        let needle = [META, META_TEMPO, 3, 0x07, 0xA1, 0x20];
        assert!(track(&bytes).windows(6).any(|w| w == needle));
    }

    #[test]
    fn test_time_and_key_signature_meta() {
        let mut m = c_d_e();
        m.key_signature = KeySignature::new(58, KeyMode::Major); // B flat: two flats
        let bytes = MidiRenderer::new().to_bytes(&[m]);
        assert!(track(&bytes)
            .windows(7)
            .any(|w| w == [META, META_TIME_SIGNATURE, 4, 4, 2, 24, 8]));
        assert!(track(&bytes)
            .windows(4)
            .any(|w| w == [META, META_KEY_SIGNATURE, 2, 0xFE]));
    }

    #[test]
    fn test_one_measure_has_exact_ticks_and_pitches() {
        let bytes = MidiRenderer::new().to_bytes(&[c_d_e()]);
        let on = STATUS_NOTE_ON;
        let off = STATUS_NOTE_OFF;
        assert_eq!(
            channel_events(&bytes),
            vec![
                (0, on, 60),
                (96, off, 60),
                (96, on, 62),
                (288, off, 62),
                (288, on, 64),
                (384, off, 64)
            ]
        );
    }

    #[test]
    fn test_second_measure_starts_at_tick_384() {
        let bytes = MidiRenderer::new().to_bytes(&[c_d_e(), c_d_e()]);
        let starts: Vec<u32> = channel_events(&bytes)
            .into_iter()
            .filter(|&(_, s, k)| s == STATUS_NOTE_ON && k == 60)
            .map(|(t, _, _)| t)
            .collect();
        assert_eq!(starts, vec![0, 384]);
    }

    #[test]
    fn test_count_in_adds_accented_clicks_and_shifts_the_notes() {
        let bytes = MidiRenderer::new().with_count_in(true).to_bytes(&[c_d_e()]);
        let events = channel_events(&bytes);
        let clicks: Vec<(u32, u8)> = events
            .iter()
            .filter(|&&(_, s, _)| s == STATUS_NOTE_ON | PERCUSSION_CHANNEL)
            .map(|&(t, _, k)| (t, k))
            .collect();
        assert_eq!(clicks, vec![(0, 76), (96, 77), (192, 77), (288, 77)]);
        let first_note = events
            .iter()
            .find(|&&(_, s, _)| s == STATUS_NOTE_ON)
            .unwrap();
        assert_eq!(first_note.0, 384);
    }

    #[test]
    fn test_compound_time_count_in_clicks_once_per_dotted_beat() {
        let clicks = count_in_events(&TimeSignature::six_eight());
        let ons: Vec<u32> = clicks
            .iter()
            .filter(|e| e.order == 1)
            .map(|e| e.tick)
            .collect();
        assert_eq!(ons, vec![0, 144]);
        assert_eq!(measure_ticks(&TimeSignature::six_eight()), 288);
    }

    #[test]
    fn test_program_change_is_the_instruments() {
        let bytes = MidiRenderer::new().to_bytes(&[c_d_e()]);
        let program = Instrument::AcousticGrandPiano.program();
        assert_eq!(program, 0);
        assert!(track(&bytes)
            .windows(3)
            .any(|w| w == [0, STATUS_PROGRAM_CHANGE, program]));
    }

    #[test]
    fn test_instrument_names() {
        assert_eq!(
            Instrument::from_name(" Piano "),
            Some(Instrument::AcousticGrandPiano)
        );
        assert_eq!(Instrument::from_name("kazoo"), None);
    }

    #[test]
    fn test_repeated_pitch_releases_before_it_attacks() {
        let m = measure(1, &[(60, NoteDuration::Half), (60, NoteDuration::Half)]);
        let events = channel_events(&MidiRenderer::new().to_bytes(&[m]));
        assert_eq!(events[1], (192, STATUS_NOTE_OFF, 60));
        assert_eq!(events[2], (192, STATUS_NOTE_ON, 60));
    }

    #[test]
    fn test_dotted_notes_are_timed_like_they_are_drawn() {
        let mut m = measure(1, &[(60, NoteDuration::Quarter)]);
        m.notes[0] = m.notes[0].dotted();
        let events = channel_events(&MidiRenderer::new().to_bytes(&[m]));
        assert_eq!(events[1].0, 144);
    }

    #[test]
    fn test_empty_input_is_still_a_valid_file() {
        let bytes = MidiRenderer::new().to_bytes(&[]);
        assert_eq!(&bytes[0..4], b"MThd");
        let length = u32::from_be_bytes(bytes[18..22].try_into().unwrap()) as usize;
        assert_eq!(length, bytes.len() - 22);
        assert!(track(&bytes).windows(3).any(|w| w == [META, META_TEMPO, 3]));
        assert!(channel_events(&bytes).is_empty());
    }

    #[test]
    fn test_save_to_file_reports_errors() {
        let error = MidiRenderer::new()
            .save_to_file(&[], "/nonexistent/dir/x.mid")
            .unwrap_err();
        assert!(error.contains("/nonexistent/dir/x.mid"), "{error}");
    }
}

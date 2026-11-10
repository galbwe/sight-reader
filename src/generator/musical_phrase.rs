//! Musical phrase generation

use crate::domain::{Note, NoteDuration, Key, TimeSignature};
use crate::generator::{GenerationRules, Melody, OctaveRange};

/// Generator for musical phrases
pub struct MusicalPhraseGenerator {
    time_signature: TimeSignature,
    melody: Melody,
}

impl MusicalPhraseGenerator {
    /// Create a new musical phrase generator. Its notes come from the scale of `key`.
    pub fn new(key: Key, time_signature: TimeSignature, range: OctaveRange) -> Self {
        let interval_selection = crate::generator::IntervalSelection::default();
        let rules = GenerationRules::new(&interval_selection);
        let melody = Melody::new(&key, range.min_midi(), range.max_midi(), rules);

        MusicalPhraseGenerator {
            time_signature,
            melody,
        }
    }

    /// Generate a musical phrase with the specified number of beats
    pub fn generate_phrase(&self, beats: f64) -> Vec<Note> {
        let mut notes = Vec::new();
        let mut remaining_beats = beats;
        
        while remaining_beats > 0.001 {
            let note = self.generate_next_note(&notes, remaining_beats);
            let note_beats = Self::note_beats(&note, self.time_signature);
            
            if note_beats <= remaining_beats {
                notes.push(note);
                remaining_beats -= note_beats;
            } else {
                // Try a shorter note
                let shorter = self.shorter_duration(note.duration);
                if shorter != note.duration {
                    let shorter_note = Note::new(note.midi_number, shorter);
                    let shorter_beats = Self::note_beats(&shorter_note, self.time_signature);
                    if shorter_beats <= remaining_beats {
                        notes.push(shorter_note);
                        remaining_beats -= shorter_beats;
                    }
                }
                break;
            }
        }
        
        notes
    }

    /// Generate the next note in a phrase
    fn generate_next_note(&self, previous_notes: &[Note], remaining_beats: f64) -> Note {
        // Determine a reasonable duration for the next note
        let duration = self.select_phrase_duration(remaining_beats);

        let history: Vec<u8> = previous_notes.iter().map(|note| note.midi_number).collect();
        Note::new(self.melody.next_note(&history), duration)
    }

    /// Select a duration based on remaining beats
    fn select_phrase_duration(&self, remaining_beats: f64) -> NoteDuration {
        // Prefer durations that fit well in the remaining time
        if remaining_beats >= 4.0 {
            NoteDuration::Half
        } else if remaining_beats >= 2.0 {
            NoteDuration::Quarter
        } else if remaining_beats >= 1.0 {
            NoteDuration::Eighth
        } else if remaining_beats >= 0.5 {
            NoteDuration::Sixteenth
        } else {
            NoteDuration::Sixteenth
        }
    }

    /// Get a shorter duration
    fn shorter_duration(&self, duration: NoteDuration) -> NoteDuration {
        match duration {
            NoteDuration::Whole => NoteDuration::Half,
            NoteDuration::Half => NoteDuration::Quarter,
            NoteDuration::Quarter => NoteDuration::Eighth,
            NoteDuration::Eighth => NoteDuration::Sixteenth,
            NoteDuration::Sixteenth => NoteDuration::Sixteenth,
            NoteDuration::ThirtySecond => NoteDuration::Sixteenth,
            NoteDuration::SixtyFourth => NoteDuration::ThirtySecond,
        }
    }

    /// Calculate beats for a note based on time signature
    fn note_beats(note: &Note, ts: TimeSignature) -> f64 {
        let base = note.duration.beats_in_4_4();
        let adjustment = 4.0 / ts.denominator as f64;
        let dotted = if note.dotted { 1.5 } else { 1.0 };
        let triplet = if note.is_triplet { 2.0 / 3.0 } else { 1.0 };
        
        base * adjustment * dotted * triplet
    }

    /// Generate a complete exercise measure
    pub fn generate_measure(&self, measure_number: u32) -> Vec<Note> {
        let beats_per_measure = self.time_signature.total_beats();
        self.generate_phrase(beats_per_measure)
    }

    /// Generate multiple measures
    pub fn generate_measures(&self, count: u32) -> Vec<Vec<Note>> {
        (1..=count)
            .map(|n| self.generate_measure(n))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phrase_generation() {
        let key = Key::c_major();
        let ts = TimeSignature::common_time();
        let range = OctaveRange::centered_on_middle_c(2);
        
        let generator = MusicalPhraseGenerator::new(key, ts, range);
        let phrase = generator.generate_phrase(4.0);
        
        // Should generate notes that total approximately 4 beats
        let total_beats: f64 = phrase.iter()
            .map(|n| MusicalPhraseGenerator::note_beats(n, ts))
            .sum();
        
        assert!((total_beats - 4.0).abs() < 0.01);
    }

    #[test]
    fn test_measure_generation() {
        let key = Key::c_major();
        let ts = TimeSignature::common_time();
        let range = OctaveRange::centered_on_middle_c(2);
        
        let generator = MusicalPhraseGenerator::new(key, ts, range);
        let measure = generator.generate_measure(1);
        
        // Should have some notes
        assert!(!measure.is_empty());
    }

    #[test]
    fn test_phrase_notes_belong_to_the_key() {
        let key = Key::new(67, crate::domain::KeyMode::Major);
        let ts = TimeSignature::common_time();
        let range = OctaveRange::centered_on_middle_c(2);
        let generator = MusicalPhraseGenerator::new(key, ts, range);
        for _ in 0..30 {
            for note in generator.generate_measure(1) {
                assert!(key.contains_note(note.midi_number));
            }
        }
    }
}

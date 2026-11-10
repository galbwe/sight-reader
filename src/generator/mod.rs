//! Generator module with re-exports

pub mod config;
pub mod melody;
pub mod rules;
pub mod musical_phrase;

pub use config::{NoteDurationSelection, IntervalSelection};
pub use melody::Melody;
pub use rules::GenerationRules;

use crate::domain::{Note, NoteDuration, TimeSignature, Key, Measure};

/// Configuration for the exercise generator
pub struct GeneratorConfig {
    pub key: Key,
    pub time_signature: TimeSignature,
    pub range: OctaveRange,
    pub interval_selection: IntervalSelection,
    pub duration_selection: NoteDurationSelection,
}

impl GeneratorConfig {
    /// Create a default configuration
    pub fn default() -> Self {
        GeneratorConfig {
            key: Key::c_major(),
            time_signature: TimeSignature::common_time(),
            range: OctaveRange::new(60, 72), // C4 to C5
            interval_selection: IntervalSelection::default(),
            duration_selection: NoteDurationSelection::default(),
        }
    }
}

/// Represents the octave range for the exercise
#[derive(Debug, Clone, Copy)]
pub struct OctaveRange {
    pub min_octave: i32,
    pub max_octave: i32,
    pub center_octave: i32,
}

impl OctaveRange {
    /// Create a new octave range
    pub fn new(min_midi: u8, max_midi: u8) -> Self {
        let min_octave = (min_midi / 12) as i32;
        let max_octave = (max_midi / 12) as i32;
        let center_octave = (min_octave + max_octave) / 2;
        
        OctaveRange {
            min_octave,
            max_octave,
            center_octave,
        }
    }

    /// Create a range centered around middle C with a given span
    pub fn centered_on_middle_c(upper_octaves: u8) -> Self {
        let upper_midi = 60 + (upper_octaves as i32 * 12) as u8;
        OctaveRange::new(60, upper_midi)
    }

    /// Get the minimum MIDI number in this range
    pub fn min_midi(&self) -> u8 {
        (self.min_octave as u8) * 12
    }

    /// Get the maximum MIDI number in this range
    pub fn max_midi(&self) -> u8 {
        (self.max_octave as u8) * 12 + 11
    }

    /// Get the MIDI note in the middle of the range
    pub fn center_note(&self) -> u8 {
        ((self.min_midi() as u16 + self.max_midi() as u16) / 2) as u8
    }
}

/// Generates sight-reading exercises based on configuration
pub struct ExerciseGenerator {
    config: GeneratorConfig,
}

impl ExerciseGenerator {
    /// Create a new generator with the given configuration
    pub fn new(config: GeneratorConfig) -> Self {
        ExerciseGenerator { config }
    }

    /// Generate a complete exercise with the specified number of measures
    pub fn generate_exercise(&self, num_measures: u32) -> Vec<Measure> {
        let time_signature = self.config.time_signature;
        let key_signature = self.config.key.signature();
        
        let range = self.config.range;
        let rules = GenerationRules::new(&self.config.interval_selection);
        let melody = Melody::new(&self.config.key, range.min_midi(), range.max_midi(), rules);

        let mut measures = Vec::new();
        // Every pitch so far, so each measure carries on from the end of the one before
        let mut history: Vec<u8> = Vec::new();

        for measure_num in 1..=num_measures {
            let mut measure = Measure::new(measure_num, time_signature, key_signature);

            // Fill the measure with notes from the key's scale
            while measure.has_space() && !measure.is_full() {
                let duration = self.select_duration(measure.remaining_space());
                let midi = melody.next_note(&history);
                let note = Note::new(midi, duration);
                if measure.add_note(note) {
                    measure.notes.push(note);
                    history.push(midi);
                } else {
                    break;
                }
            }

            measures.push(measure);
        }

        measures
    }

    /// Select a note duration that fits within the remaining space
    fn select_duration(&self, remaining_space: f64) -> NoteDuration {
        // Filter durations that fit in the remaining space
        let valid_durations: Vec<NoteDuration> = self.config.duration_selection.get_valid_durations(remaining_space);
        
        if valid_durations.is_empty() {
            return NoteDuration::Quarter; // fallback
        }
        
        // Use weighted selection based on user preferences
        self.weighted_duration_selection(&valid_durations)
    }

    /// Weighted selection of duration based on user preferences
    fn weighted_duration_selection(&self, durations: &[NoteDuration]) -> NoteDuration {
        let preferences = &self.config.duration_selection;
        
        // Calculate weights based on user preferences
        let total_weight: u32 = durations.iter()
            .map(|d| preferences.weight(*d))
            .sum();
        
        if total_weight == 0 {
            return durations[0]; // fallback
        }
        
        // Simple weighted random selection
        let mut rng = rand::random::<u32>() % total_weight;
        for duration in durations {
            let weight = preferences.weight(*duration);
            if rng < weight {
                return *duration;
            }
            rng -= weight;
        }
        
        durations[0] // fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::KeyMode;

    const TREBLE_RANGE: (u8, u8) = (60, 83);

    fn config_in(root: u8, mode: KeyMode) -> GeneratorConfig {
        GeneratorConfig {
            key: Key::new(root, mode),
            time_signature: TimeSignature::common_time(),
            range: OctaveRange::new(TREBLE_RANGE.0, TREBLE_RANGE.1),
            interval_selection: IntervalSelection::default(),
            duration_selection: NoteDurationSelection::default(),
        }
    }

    fn exercise(config: GeneratorConfig, measures: u32) -> Vec<Measure> {
        ExerciseGenerator::new(config).generate_exercise(measures)
    }

    fn all_pitches(measures: &[Measure]) -> Vec<u8> {
        measures.iter().flat_map(|m| m.notes.iter().map(|n| n.midi_number)).collect()
    }

    #[test]
    fn test_every_note_belongs_to_the_key() {
        // The CLI can produce roots from 47 to 71, sharp and flat keys alike
        for root in 47u8..=71 {
            let key = Key::new(root, KeyMode::Major);
            for pitch in all_pitches(&exercise(config_in(root, KeyMode::Major), 24)) {
                assert!(key.contains_note(pitch), "root {root}: {pitch} is not in the key");
            }
        }
    }

    #[test]
    fn test_every_note_belongs_to_a_minor_key() {
        let key = Key::new(57, KeyMode::Minor);
        for pitch in all_pitches(&exercise(config_in(57, KeyMode::Minor), 24)) {
            assert!(key.contains_note(pitch));
        }
    }

    #[test]
    fn test_notes_stay_inside_the_range() {
        for pitch in all_pitches(&exercise(config_in(60, KeyMode::Major), 40)) {
            assert!((TREBLE_RANGE.0..=TREBLE_RANGE.1).contains(&pitch));
        }
    }

    #[test]
    fn test_exercise_starts_on_the_tonic() {
        for root in [60u8, 67, 62, 58, 53] {
            let measures = exercise(config_in(root, KeyMode::Major), 4);
            let first = measures[0].notes[0].midi_number;
            assert_eq!(first % 12, root % 12, "root {root}");
        }
    }

    #[test]
    fn test_measures_carry_the_key_signature_and_are_filled() {
        let measures = exercise(config_in(67, KeyMode::Major), 12);
        assert_eq!(measures.len(), 12);
        for measure in &measures {
            assert_eq!(measure.key_signature, Key::new(67, KeyMode::Major).signature());
            assert!(!measure.notes.is_empty());
            assert!(
                (measure.total_duration() - measure.time_signature.total_beats()).abs() < 1e-9,
                "measure {} holds {} beats",
                measure.number,
                measure.total_duration()
            );
        }
    }

    #[test]
    fn test_melody_flows_across_barlines() {
        // Measures no longer all begin on the same note: the next measure continues from the last
        let measures = exercise(config_in(60, KeyMode::Major), 40);
        let starts: std::collections::HashSet<u8> =
            measures.iter().map(|m| m.notes[0].midi_number).collect();
        assert!(starts.len() >= 4, "measures start on only {} different notes", starts.len());
    }

    #[test]
    fn test_moves_between_measures_follow_the_interval_selection() {
        let mut config = config_in(60, KeyMode::Major);
        config.interval_selection = IntervalSelection::steps_only();
        let measures = exercise(config, 30);
        let scale = melody::scale_notes(&Key::c_major(), TREBLE_RANGE.0, TREBLE_RANGE.1);
        let pitches = all_pitches(&measures);
        for pair in pitches.windows(2) {
            let a = scale.iter().position(|&n| n == pair[0]).unwrap() as i32;
            let b = scale.iter().position(|&n| n == pair[1]).unwrap() as i32;
            assert!((a - b).abs() <= 1, "{} to {} is more than a step", pair[0], pair[1]);
        }
    }

    #[test]
    fn test_melody_does_not_only_climb() {
        let pitches = all_pitches(&exercise(config_in(60, KeyMode::Major), 40));
        let rises = pitches.windows(2).filter(|w| w[1] > w[0]).count();
        let falls = pitches.windows(2).filter(|w| w[1] < w[0]).count();
        assert!(rises > 20 && falls > 20, "{rises} rises, {falls} falls");
    }

    #[test]
    fn test_default_configuration_uses_a_treble_range() {
        let config = GeneratorConfig::default();
        assert_eq!((config.range.min_midi(), config.range.max_midi()), (60, 83));
        for pitch in all_pitches(&exercise(config, 10)) {
            assert!(Key::c_major().contains_note(pitch));
        }
    }

    #[test]
    fn test_center_note_is_the_middle_of_the_range() {
        let range = OctaveRange::new(60, 83);
        assert_eq!(range.center_note(), 71);
        let one_octave = OctaveRange::new(60, 71);
        assert_eq!(one_octave.center_note(), 65);
    }

    #[test]
    fn test_zero_measures() {
        assert!(exercise(config_in(60, KeyMode::Major), 0).is_empty());
    }
}

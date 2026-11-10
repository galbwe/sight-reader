//! Melody generation: pitches are chosen from the notes of the key's scale
//!
//! Every note a `Melody` produces belongs to the key and lies inside the requested range, so an
//! exercise in G major contains F sharps and never F naturals. Moves go up and down the scale by
//! steps, skips and leaps according to `GenerationRules`.

use crate::domain::Key;
use crate::generator::rules::GenerationRules;

/// The notes of a key's scale between `min_midi` and `max_midi` (inclusive), lowest first
pub fn scale_notes(key: &Key, min_midi: u8, max_midi: u8) -> Vec<u8> {
    let pitch_classes = key.scale();
    (min_midi..=max_midi)
        .filter(|midi| pitch_classes.contains(&((midi % 12) as i32)))
        .collect()
}

/// Chooses the pitches of a melody, one note at a time
pub struct Melody {
    scale: Vec<u8>,
    tonic_pitch_class: u8,
    middle: u8,
    rules: GenerationRules,
}

impl Melody {
    /// Create a melody in `key` that stays between `min_midi` and `max_midi`
    pub fn new(key: &Key, min_midi: u8, max_midi: u8, rules: GenerationRules) -> Self {
        Melody {
            scale: scale_notes(key, min_midi, max_midi),
            tonic_pitch_class: key.tonic_pitch_class(),
            middle: ((min_midi as u16 + max_midi as u16) / 2) as u8,
            rules,
        }
    }

    /// The scale notes this melody can use, lowest first
    pub fn scale(&self) -> &[u8] {
        &self.scale
    }

    /// The first note of a melody: the tonic closest to the middle of the range
    pub fn first_note(&self) -> u8 {
        let tonics = self
            .scale
            .iter()
            .copied()
            .filter(|midi| midi % 12 == self.tonic_pitch_class);
        tonics
            .min_by_key(|midi| midi.abs_diff(self.middle))
            .or_else(|| self.nearest_scale_note(self.middle))
            .unwrap_or(self.middle)
    }

    /// Choose the next note given the notes so far (oldest first).
    ///
    /// The last two notes decide what counts as recovering from a leap. With no history the
    /// melody starts on the tonic.
    pub fn next_note(&self, history: &[u8]) -> u8 {
        let Some(&last) = history.last() else {
            return self.first_note();
        };
        let Some(current) = self.index_of(last) else {
            return last;
        };
        let previous_move = match history.len().checked_sub(2).map(|i| history[i]) {
            Some(previous) => self
                .index_of(previous)
                .map_or(0, |index| current as i32 - index as i32),
            None => 0,
        };

        let moves = self.rules.candidate_moves(&self.scale, current, previous_move);
        match GenerationRules::pick_move(&moves, rand::random::<u32>()) {
            Some(chosen) => self.scale[(current as i32 + chosen.steps) as usize],
            None => last,
        }
    }

    /// Index of a note in the scale, or of the nearest scale note if it is not in the key
    fn index_of(&self, midi: u8) -> Option<usize> {
        self.scale
            .iter()
            .position(|&note| note == midi)
            .or_else(|| {
                (0..self.scale.len()).min_by_key(|&index| self.scale[index].abs_diff(midi))
            })
    }

    fn nearest_scale_note(&self, midi: u8) -> Option<u8> {
        self.scale.iter().copied().min_by_key(|note| note.abs_diff(midi))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::KeyMode;
    use crate::generator::config::IntervalSelection;

    fn melody_in(root: u8, mode: KeyMode, min: u8, max: u8, selection: &IntervalSelection) -> Melody {
        Melody::new(&Key::new(root, mode), min, max, GenerationRules::new(selection))
    }

    /// A long random melody
    fn walk(melody: &Melody, length: usize) -> Vec<u8> {
        let mut notes = Vec::new();
        for _ in 0..length {
            let next = melody.next_note(&notes);
            notes.push(next);
        }
        notes
    }

    #[test]
    fn test_scale_notes_c_major() {
        let key = Key::c_major();
        assert_eq!(scale_notes(&key, 60, 72), vec![60, 62, 64, 65, 67, 69, 71, 72]);
    }

    #[test]
    fn test_scale_notes_in_other_keys() {
        // G major from G4 to G5
        assert_eq!(
            scale_notes(&Key::new(67, KeyMode::Major), 67, 79),
            vec![67, 69, 71, 72, 74, 76, 78, 79]
        );
        // Bb major has Bb and Eb and no B or E
        let notes = scale_notes(&Key::new(58, KeyMode::Major), 60, 83);
        assert!(notes.contains(&70) && notes.contains(&75)); // Bb4 and Eb5
        assert!(!notes.contains(&71) && !notes.contains(&64));
    }

    #[test]
    fn test_seven_notes_per_octave() {
        let notes = scale_notes(&Key::new(62, KeyMode::Major), 60, 83);
        assert_eq!(notes.len(), 14);
    }

    #[test]
    fn test_first_note_is_the_tonic_nearest_the_middle() {
        let all = IntervalSelection::default();
        // Range C4 to B5, middle B4 (71): the nearest C is C5
        let c = melody_in(60, KeyMode::Major, 60, 83, &all);
        assert_eq!(c.first_note(), 72);
        // G major: nearest G to 71 is G4 (67), 4 away, against G5 (79), 8 away
        let g = melody_in(67, KeyMode::Major, 60, 83, &all);
        assert_eq!(g.first_note(), 67);
        // Bb major: Bb4 is 70
        let b_flat = melody_in(58, KeyMode::Major, 60, 83, &all);
        assert_eq!(b_flat.first_note(), 70);
        assert_eq!(c.next_note(&[]), c.first_note());
    }

    #[test]
    fn test_melody_stays_in_the_key_in_every_major_key() {
        let all = IntervalSelection::default();
        for root in 48u8..72 {
            let key = Key::new(root, KeyMode::Major);
            let melody = melody_in(root, KeyMode::Major, 60, 83, &all);
            for note in walk(&melody, 300) {
                assert!(key.contains_note(note), "root {root}: note {note} is not in the key");
            }
        }
    }

    #[test]
    fn test_melody_stays_in_the_key_in_minor() {
        let all = IntervalSelection::default();
        let key = Key::new(57, KeyMode::Minor);
        let melody = melody_in(57, KeyMode::Minor, 57, 81, &all);
        for note in walk(&melody, 300) {
            assert!(key.contains_note(note));
        }
    }

    #[test]
    fn test_melody_stays_inside_the_range() {
        let all = IntervalSelection::default();
        for (min, max) in [(60u8, 83u8), (60, 71), (48, 95)] {
            let melody = melody_in(60, KeyMode::Major, min, max, &all);
            for note in walk(&melody, 300) {
                assert!(note >= min && note <= max, "{note} outside {min}..={max}");
            }
        }
    }

    #[test]
    fn test_melody_goes_up_and_down() {
        let all = IntervalSelection::default();
        let melody = melody_in(60, KeyMode::Major, 60, 83, &all);
        let notes = walk(&melody, 400);
        let rises = notes.windows(2).filter(|w| w[1] > w[0]).count();
        let falls = notes.windows(2).filter(|w| w[1] < w[0]).count();
        assert!(rises > 80, "only {rises} rising moves");
        assert!(falls > 80, "only {falls} falling moves");
    }

    #[test]
    fn test_melody_uses_much_of_the_range() {
        let all = IntervalSelection::default();
        let melody = melody_in(60, KeyMode::Major, 60, 83, &all);
        let notes = walk(&melody, 600);
        let lowest = *notes.iter().min().unwrap();
        let highest = *notes.iter().max().unwrap();
        assert!(highest - lowest >= 14, "range used was only {lowest}..{highest}");
    }

    #[test]
    fn test_steps_only_never_leaps() {
        let steps = IntervalSelection::steps_only();
        let melody = melody_in(67, KeyMode::Major, 60, 83, &steps);
        let scale = melody.scale().to_vec();
        let notes = walk(&melody, 400);
        for pair in notes.windows(2) {
            let a = scale.iter().position(|&n| n == pair[0]).unwrap() as i32;
            let b = scale.iter().position(|&n| n == pair[1]).unwrap() as i32;
            assert!((a - b).abs() <= 1, "moved {} scale steps", (a - b).abs());
        }
    }

    #[test]
    fn test_steps_and_skips_never_exceed_a_fourth() {
        let selection = IntervalSelection::steps_and_skips();
        let melody = melody_in(60, KeyMode::Major, 60, 83, &selection);
        let scale = melody.scale().to_vec();
        for pair in walk(&melody, 400).windows(2) {
            let a = scale.iter().position(|&n| n == pair[0]).unwrap() as i32;
            let b = scale.iter().position(|&n| n == pair[1]).unwrap() as i32;
            assert!((a - b).abs() <= 3, "moved {} scale steps", (a - b).abs());
        }
    }

    #[test]
    fn test_default_selection_makes_mostly_small_moves() {
        let all = IntervalSelection::default();
        let melody = melody_in(60, KeyMode::Major, 60, 83, &all);
        let scale = melody.scale().to_vec();
        let notes = walk(&melody, 1000);
        let steps = notes
            .windows(2)
            .filter(|pair| {
                let a = scale.iter().position(|&n| n == pair[0]).unwrap() as i32;
                let b = scale.iter().position(|&n| n == pair[1]).unwrap() as i32;
                (a - b).abs() <= 2
            })
            .count();
        assert!(steps * 100 / 999 >= 60, "only {steps} of 999 moves were steps or thirds");
    }

    #[test]
    fn test_repeated_notes_only_when_unisons_are_enabled() {
        let mut selection = IntervalSelection::steps_only();
        selection.unisons = false;
        let melody = melody_in(60, KeyMode::Major, 60, 83, &selection);
        for pair in walk(&melody, 300).windows(2) {
            assert_ne!(pair[0], pair[1]);
        }
    }

    #[test]
    fn test_narrow_range_still_works() {
        let all = IntervalSelection::default();
        // Exactly one octave of C major: 8 notes
        let melody = melody_in(60, KeyMode::Major, 60, 72, &all);
        let key = Key::c_major();
        for note in walk(&melody, 200) {
            assert!((60..=72).contains(&note) && key.contains_note(note));
        }
    }

    #[test]
    fn test_empty_scale_does_not_panic() {
        // A range with no scale notes: 61 to 61 is C# in C major
        let all = IntervalSelection::default();
        let melody = melody_in(60, KeyMode::Major, 61, 61, &all);
        assert!(melody.scale().is_empty());
        assert_eq!(melody.next_note(&[]), 61);
        assert_eq!(melody.next_note(&[61]), 61);
    }

    #[test]
    fn test_unknown_last_note_is_left_alone() {
        let all = IntervalSelection::default();
        let melody = melody_in(60, KeyMode::Major, 60, 83, &all);
        // If the history holds a note outside the key it falls back to the nearest scale note
        let next = melody.next_note(&[61]);
        assert!(Key::c_major().contains_note(next));
    }
}

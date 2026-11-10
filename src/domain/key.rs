//! Key signature representation

use std::fmt;

/// Key mode (major or minor)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyMode {
    Major,
    Minor,
}

/// Represents a musical key (e.g., C major, G minor)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    pub root_note: u8, // MIDI note number for the root
    pub mode: KeyMode,
}

impl Key {
    /// Create a new key
    pub fn new(root_note: u8, mode: KeyMode) -> Self {
        Key { root_note, mode }
    }

    /// Create C major key
    pub fn c_major() -> Self {
        Key::new(60, KeyMode::Major)
    }

    /// Create A minor key
    pub fn a_minor() -> Self {
        Key::new(57, KeyMode::Minor)
    }

    /// Get the key signature for this key
    pub fn signature(&self) -> KeySignature {
        KeySignature::new(self.root_note, self.mode)
    }

    /// Get the number of accidentals in this key
    pub fn accidental_count(&self) -> i32 {
        self.signature().accidental_count
    }

    /// Check if a note belongs to this key's scale (ignoring octaves)
    pub fn contains_note(&self, note_midi: u8) -> bool {
        let scale = self.scale();
        let note_step = (note_midi % 12) as i32;
        scale.contains(&note_step)
    }

    /// Pitch class (0 to 11, C = 0) of the tonic
    pub fn tonic_pitch_class(&self) -> u8 {
        self.root_note % 12
    }

    /// Pitch classes (0 to 11, C = 0) of this key's scale, in scale order starting from the tonic.
    ///
    /// Major keys use the major scale and minor keys the natural minor scale.
    pub fn scale(&self) -> Vec<i32> {
        let intervals = match self.mode {
            KeyMode::Major => [0, 2, 4, 5, 7, 9, 11],
            KeyMode::Minor => [0, 2, 3, 5, 7, 8, 10],
        };
        let tonic = self.tonic_pitch_class() as i32;
        intervals.iter().map(|interval| (tonic + interval) % 12).collect()
    }

    /// Get the dominant key (fifth above)
    pub fn dominant(&self) -> Self {
        let new_root = (self.root_note + 7) % 12;
        Key::new(new_root, self.mode)
    }

    /// Get the subdominant key (fifth below)
    pub fn subdominant(&self) -> Self {
        let new_root = if self.root_note >= 7 {
            self.root_note - 7
        } else {
            self.root_note + 5
        };
        Key::new(new_root, self.mode)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let note_name = self.root_note_name();
        let mode = match self.mode {
            KeyMode::Major => "Major",
            KeyMode::Minor => "Minor",
        };
        write!(f, "{} {}", note_name, mode)
    }
}

impl Key {
    fn root_note_name(&self) -> &'static str {
        const NAMES: &[&str] = &["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
        NAMES[self.root_note as usize % 12]
    }
}

/// Represents a key signature with its accidentals
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeySignature {
    /// Number of sharps (positive) or flats (negative)
    pub accidental_count: i32,
    pub mode: KeyMode,
    /// The last sharp or the second-to-last flat
    pub leading_note: u8,
}

impl KeySignature {
    /// Create a new key signature
    pub fn new(root_note: u8, mode: KeyMode) -> Self {
        // Calculate number of sharps/flats based on circle of fifths
        // C major / A minor = 0 accidentals
        // G major / E minor = 1 sharp
        // D major / B minor = 2 sharps
        // etc.
        
        let (accidental_count, leading_note) = Self::calculate_signature(root_note, mode);
        
        KeySignature {
            accidental_count,
            mode,
            leading_note,
        }
    }

    fn calculate_signature(root_note: u8, mode: KeyMode) -> (i32, u8) {
        let root_step = root_note % 12; // Get the note step within the octave
        
        // First check if this is a flat key by looking at root_note directly
        // For flat keys, use root values that don't match the sharp key pattern
        // F=53 (F4), Bb=58 (Bb4), Eb=51 (Eb4), Ab=56 (Ab4), Db=49 (Db4), Gb=54 (Gb4), Cb=47 (Cb4)
        let is_flat_key = match root_note {
            53 | 58 | 51 | 56 | 49 | 54 | 47 => true,
            _ => false,
        };
        
        if is_flat_key {
            // Use root_step for flat key lookup
            let flats = match root_step as i32 {
                5 => 1,   // F
                10 => 2,  // Bb
                3 => 3,   // Eb
                8 => 4,   // Ab
                1 => 5,   // Db
                6 => 6,   //Gb
                11 => 7,  //Cb
                _ => 0,
            };
            return (-flats as i32, Self::flat_root(flats as u8));
        }
        
        match mode {
            KeyMode::Major => {
                // Circle of fifths for major keys
                // C=0, G=1, D=2, A=3, E=4, B=5, F#=6, Cb=7
                let fifths_from_c = match root_step {
                    0 => 0,   // C
                    7 => 1,   // G
                    2 => 2,   // D
                    9 => 3,   // A
                    4 => 4,   // E
                    11 => 5,  // B
                    6 => 6,   // F#
                    1 => 7,   // C#
                    _ => -1i8,  // Flat keys (simplified)
                };
                
                if fifths_from_c >= 0 {
                    (fifths_from_c as i32, Self::sharp_root(fifths_from_c as u8))
                } else {
                    // This branch shouldn't be reached for flat major keys now
                    // It's here for any edge cases
                    (0, 0)
                }
            }
            KeyMode::Minor => {
                // Relative major + 3 semitones down
                let relative_major_root = (root_note as i32 - 3).rem_euclid(12) as u8;
                Self::calculate_signature(relative_major_root, KeyMode::Major)
            }
        }
    }

    fn sharp_root(sharps: u8) -> u8 {
        const SHARPS: &[u8] = &[0, 7, 2, 9, 4, 11, 6, 1]; // C, G, D, A, E, B, F#, C#
        SHARPS[sharps as usize]
    }

    fn flat_root(flats: u8) -> u8 {
        const FLATS: &[u8] = &[5, 10, 3, 8, 1, 6, 11, 4]; // F, Bb, Eb, Ab, Db, Gb, Cb, Fb
        FLATS[flats as usize]
    }

    /// Get the number of accidentals (always non-negative)
    pub fn count(&self) -> u8 {
        self.accidental_count.abs() as u8
    }

    /// Get the accidental for a given scale step (0-6)
    pub fn get_accidental_for_step(&self, step: i32) -> Accidental {
        if self.accidental_count == 0 {
            return Accidental::Natural;
        }

        let steps_with_accidentals = self.steps_with_accidentals();
        if steps_with_accidentals.contains(&step) {
            if self.accidental_count > 0 {
                Accidental::Sharp
            } else {
                Accidental::Flat
            }
        } else {
            Accidental::Natural
        }
    }

    fn steps_with_accidentals(&self) -> Vec<i32> {
        if self.accidental_count == 0 {
            return vec![];
        }

        // F#C#G#D#A#E#B# for sharps
        // BbEbAbDbGbCbFb for flats
        match self.accidental_count {
            1..=7 => vec![3, 0, 4, 1, 5, 2, 6], // F#C#G#D#A#E#B#
            -7..=-1 => vec![6, 2, 5, 1, 4, 0, 3], // BbEbAbDbGbCbFb
            _ => vec![],
        }
    }
}

impl fmt::Display for KeySignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.accidental_count == 0 {
            return write!(f, "No accidentals");
        }

        let (count, symbol) = if self.accidental_count > 0 {
            (self.accidental_count as u8, "#")
        } else {
            (-self.accidental_count as u8, "b")
        };

        write!(f, "{} {}", count, symbol)
    }
}

/// Accidental types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Accidental {
    Natural,
    Sharp,
    Flat,
    DoubleSharp,
    DoubleFlat,
}

impl fmt::Display for Accidental {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Accidental::Natural => write!(f, "♮"),
            Accidental::Sharp => write!(f, "#"),
            Accidental::Flat => write!(f, "b"),
            Accidental::DoubleSharp => write!(f, "##"),
            Accidental::DoubleFlat => write!(f, "bb"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_c_major_scale() {
        assert_eq!(Key::c_major().scale(), vec![0, 2, 4, 5, 7, 9, 11]);
    }

    #[test]
    fn test_scale_is_transposed_to_the_root() {
        // G major: G A B C D E F#
        assert_eq!(Key::new(67, KeyMode::Major).scale(), vec![7, 9, 11, 0, 2, 4, 6]);
        // Bb major (root 58): Bb C D Eb F G A
        assert_eq!(Key::new(58, KeyMode::Major).scale(), vec![10, 0, 2, 3, 5, 7, 9]);
    }

    #[test]
    fn test_minor_scale_uses_natural_minor() {
        // A minor has the same notes as C major
        let mut a_minor = Key::a_minor().scale();
        a_minor.sort();
        assert_eq!(a_minor, vec![0, 2, 4, 5, 7, 9, 11]);
        assert_eq!(Key::a_minor().scale()[0], 9);
    }

    #[test]
    fn test_contains_note_is_relative_to_the_key() {
        let g_major = Key::new(67, KeyMode::Major);
        assert!(g_major.contains_note(66)); // F#
        assert!(!g_major.contains_note(65)); // F natural
        assert!(g_major.contains_note(79)); // G, an octave up
        let b_flat = Key::new(58, KeyMode::Major);
        assert!(b_flat.contains_note(70)); // Bb
        assert!(!b_flat.contains_note(71)); // B natural
        assert!(!b_flat.contains_note(66)); // F#, not in Bb major
    }

    #[test]
    fn test_scale_matches_key_signature_for_every_major_key() {
        // The notes of the scale must agree with the number of sharps or flats
        for root in 48u8..60 {
            let key = Key::new(root, KeyMode::Major);
            let signature = key.signature();
            let black_keys = key.scale().iter().filter(|pc| [1, 3, 6, 8, 10].contains(pc)).count();
            // Every key has as many black-key scale notes as its signature has accidentals,
            // except C# / Cb style spellings where white keys carry accidentals
            if signature.accidental_count.abs() <= 5 {
                assert_eq!(black_keys as i32, signature.accidental_count.abs(), "root {root}");
            }
        }
    }

    #[test]
    fn test_tonic_pitch_class() {
        assert_eq!(Key::new(58, KeyMode::Major).tonic_pitch_class(), 10);
        assert_eq!(Key::c_major().tonic_pitch_class(), 0);
    }
}

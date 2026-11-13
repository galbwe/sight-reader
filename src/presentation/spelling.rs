//! Note spelling and accidentals: how a MIDI pitch is written on a staff.
//!
//! A MIDI number does not say whether a black key is a sharp or a flat. Spelling picks a letter
//! name and an alteration for each pitch, given the key signature. Accidentals then decide which
//! notes need a sharp, flat or natural sign in front of them, using the common practice rule that
//! a sign holds for the rest of the bar, for that pitch in that octave only.

use std::collections::HashMap;

use crate::domain::KeySignature;

/// Pitch class (0 to 11) of each natural letter name, indexed C D E F G A B
const NATURAL_PITCH_CLASS: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
/// Letters in the order sharps are added to a key signature: F C G D A E B
const SHARP_ORDER: [usize; 7] = [3, 0, 4, 1, 5, 2, 6];
/// Letters in the order flats are added to a key signature: B E A D G C F
const FLAT_ORDER: [usize; 7] = [6, 2, 5, 1, 4, 0, 3];

/// An accidental sign written in front of a note
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccidentalSign {
    Flat,
    Natural,
    Sharp,
}

impl AccidentalSign {
    fn for_alteration(alteration: i32) -> Self {
        match alteration {
            a if a > 0 => AccidentalSign::Sharp,
            a if a < 0 => AccidentalSign::Flat,
            _ => AccidentalSign::Natural,
        }
    }
}

/// A pitch written as a letter name, an alteration and an octave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpelledPitch {
    /// Letter name: 0 = C, 1 = D, 2 = E, 3 = F, 4 = G, 5 = A, 6 = B
    pub letter: usize,
    /// Semitones above (positive) or below (negative) the natural note: -1, 0 or 1
    pub alteration: i32,
    /// Written octave in scientific pitch notation (middle C is C4). Note that B#3 sounds in
    /// octave 4 and Cb4 sounds in octave 3, because the octave follows the letter.
    pub octave: i32,
}

impl SpelledPitch {
    /// Position on the diatonic scale, counting letters upward from C0. Adjacent lines and spaces
    /// of a staff are adjacent diatonic indexes.
    pub fn diatonic_index(&self) -> i32 {
        self.octave * 7 + self.letter as i32
    }
}

/// A note ready to draw: where it sits and which accidental sign (if any) precedes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpelledNote {
    pub pitch: SpelledPitch,
    pub accidental: Option<AccidentalSign>,
}

/// The alteration the key signature applies to each letter, indexed C D E F G A B
pub fn key_alterations(key: &KeySignature) -> [i32; 7] {
    let mut alterations = [0; 7];
    let count = (key.accidental_count.unsigned_abs() as usize).min(7);
    let (order, alteration) = if key.accidental_count > 0 {
        (SHARP_ORDER, 1)
    } else {
        (FLAT_ORDER, -1)
    };
    for &letter in order.iter().take(count) {
        alterations[letter] = alteration;
    }
    alterations
}

fn pitch(letter: usize, alteration: i32, midi: i32) -> SpelledPitch {
    // The written octave follows the letter, so remove the alteration before dividing
    let octave = (midi - alteration - NATURAL_PITCH_CLASS[letter]).div_euclid(12) - 1;
    SpelledPitch {
        letter,
        alteration,
        octave,
    }
}

/// Choose the letter name and alteration for a MIDI note in a key.
///
/// 1. A pitch that is in the key's scale is spelled the way the key signature says (so C# in D
///    major, Bb in Bb major, and E# in C# major).
/// 2. Another white key is spelled as the natural note.
/// 3. A remaining black key is a sharp in sharp keys and a flat in flat keys. In a key without
///    accidentals, the usual convention applies: sharps when rising, flats when falling.
pub fn spell_pitch(midi: u8, key: &KeySignature, descending: bool) -> SpelledPitch {
    let midi = midi as i32;
    let pitch_class = midi.rem_euclid(12);
    let alterations = key_alterations(key);

    if let Some(letter) =
        (0..7).find(|&l| (NATURAL_PITCH_CLASS[l] + alterations[l]).rem_euclid(12) == pitch_class)
    {
        return pitch(letter, alterations[letter], midi);
    }
    if let Some(letter) = (0..7).find(|&l| NATURAL_PITCH_CLASS[l] == pitch_class) {
        return pitch(letter, 0, midi);
    }

    let use_flats = key.accidental_count < 0 || (key.accidental_count == 0 && descending);
    let (alteration, from_natural) = if use_flats { (-1, -1) } else { (1, 1) };
    let letter = (0..7)
        .find(|&l| (NATURAL_PITCH_CLASS[l] + from_natural).rem_euclid(12) == pitch_class)
        .expect("every black key is one semitone from a white key");
    pitch(letter, alteration, midi)
}

/// Spell the notes of one measure and decide which need an accidental sign.
///
/// A sign is needed when a note's alteration differs from what currently applies to its letter
/// and octave. That is the key signature at first, and afterwards whatever the last sign in this
/// measure said. Returning to the key's pitch after an altered note therefore gets a sign too.
/// The barline cancels all signs, so every measure starts fresh from the key signature.
pub fn spell_measure(midi_notes: &[u8], key: &KeySignature) -> Vec<SpelledNote> {
    let alterations = key_alterations(key);
    let mut in_force: HashMap<(usize, i32), i32> = HashMap::new();
    let mut previous: Option<u8> = None;

    midi_notes
        .iter()
        .map(|&midi| {
            let descending = previous.is_some_and(|p| midi < p);
            previous = Some(midi);

            let pitch = spell_pitch(midi, key, descending);
            let current = in_force
                .get(&(pitch.letter, pitch.octave))
                .copied()
                .unwrap_or(alterations[pitch.letter]);
            let accidental = if pitch.alteration == current {
                None
            } else {
                in_force.insert((pitch.letter, pitch.octave), pitch.alteration);
                Some(AccidentalSign::for_alteration(pitch.alteration))
            };
            SpelledNote { pitch, accidental }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::KeyMode;

    const C_MAJOR: u8 = 60;
    const G_MAJOR: u8 = 67; // 1 sharp
    const D_MAJOR: u8 = 62; // 2 sharps
    const F_MAJOR: u8 = 53; // 1 flat
    const B_FLAT_MAJOR: u8 = 58; // 2 flats
    const C_SHARP_MAJOR: u8 = 61; // 7 sharps
    const C_FLAT_MAJOR: u8 = 47; // 7 flats

    const C: usize = 0;
    const D: usize = 1;
    const E: usize = 2;
    const F: usize = 3;
    const G: usize = 4;
    const A: usize = 5;
    const B: usize = 6;

    fn key(root: u8) -> KeySignature {
        KeySignature::new(root, KeyMode::Major)
    }

    fn spelled(midi: u8, root: u8, descending: bool) -> (usize, i32, i32) {
        let p = spell_pitch(midi, &key(root), descending);
        (p.letter, p.alteration, p.octave)
    }

    #[test]
    fn test_key_alterations() {
        assert_eq!(key_alterations(&key(C_MAJOR)), [0; 7]);
        let g = key_alterations(&key(G_MAJOR));
        assert_eq!(g[F], 1);
        assert_eq!(g.iter().filter(|&&a| a != 0).count(), 1);
        let b_flat = key_alterations(&key(B_FLAT_MAJOR));
        assert_eq!((b_flat[B], b_flat[E]), (-1, -1));
        assert_eq!(b_flat.iter().filter(|&&a| a != 0).count(), 2);
        assert_eq!(key_alterations(&key(C_SHARP_MAJOR)), [1; 7]);
        assert_eq!(key_alterations(&key(C_FLAT_MAJOR)), [-1; 7]);
    }

    #[test]
    fn test_white_keys_in_c_major() {
        assert_eq!(spelled(60, C_MAJOR, false), (C, 0, 4));
        assert_eq!(spelled(64, C_MAJOR, false), (E, 0, 4));
        assert_eq!(spelled(71, C_MAJOR, false), (B, 0, 4));
        assert_eq!(spelled(72, C_MAJOR, false), (C, 0, 5));
        assert_eq!(spelled(48, C_MAJOR, false), (C, 0, 3));
    }

    #[test]
    fn test_diatonic_index_anchors_the_staff() {
        let e4 = spell_pitch(64, &key(C_MAJOR), false);
        assert_eq!(
            e4.diatonic_index(),
            crate::domain::Clef::Treble.bottom_line_diatonic_index()
        );
        let f4 = spell_pitch(65, &key(C_MAJOR), false);
        assert_eq!(f4.diatonic_index(), e4.diatonic_index() + 1);
    }

    #[test]
    fn test_scale_notes_follow_the_key_signature() {
        // D major: F# and C# are in the key, so they are spelled that way and need no sign
        assert_eq!(spelled(66, D_MAJOR, false), (F, 1, 4));
        assert_eq!(spelled(61, D_MAJOR, false), (C, 1, 4));
        // Bb major: Bb is in the key, never A#
        assert_eq!(spelled(70, B_FLAT_MAJOR, false), (B, -1, 4));
        assert_eq!(spelled(63, B_FLAT_MAJOR, false), (E, -1, 4));
        // F major
        assert_eq!(spelled(70, F_MAJOR, false), (B, -1, 4));
    }

    #[test]
    fn test_chromatic_notes_use_the_key_direction() {
        // Sharp key: D# is a sharp; flat key: Db is a flat
        assert_eq!(spelled(63, G_MAJOR, false), (D, 1, 4));
        assert_eq!(spelled(63, G_MAJOR, true), (D, 1, 4));
        assert_eq!(spelled(61, F_MAJOR, false), (D, -1, 4));
        assert_eq!(spelled(61, F_MAJOR, true), (D, -1, 4));
    }

    #[test]
    fn test_chromatic_notes_in_c_major_follow_melodic_direction() {
        assert_eq!(spelled(61, C_MAJOR, false), (C, 1, 4)); // rising: C#
        assert_eq!(spelled(61, C_MAJOR, true), (D, -1, 4)); // falling: Db
        assert_eq!(spelled(70, C_MAJOR, false), (A, 1, 4));
        assert_eq!(spelled(70, C_MAJOR, true), (B, -1, 4));
    }

    #[test]
    fn test_white_key_outside_the_key_is_a_natural() {
        // F natural in G major, B natural in F major
        assert_eq!(spelled(65, G_MAJOR, false), (F, 0, 4));
        assert_eq!(spelled(71, F_MAJOR, false), (B, 0, 4));
    }

    #[test]
    fn test_extreme_keys_use_e_sharp_b_sharp_c_flat_f_flat() {
        // C# major has every note sharp: E# is the note a semitone above E, B# is above B
        assert_eq!(spelled(65, C_SHARP_MAJOR, false), (E, 1, 4));
        assert_eq!(spelled(72, C_SHARP_MAJOR, false), (B, 1, 4)); // B#4 sounds as C5
        assert_eq!(spelled(60, C_SHARP_MAJOR, false), (B, 1, 3)); // B#3 sounds as C4
                                                                  // Cb major has every note flat
        assert_eq!(spelled(59, C_FLAT_MAJOR, false), (C, -1, 4)); // Cb4 sounds as B3
        assert_eq!(spelled(71, C_FLAT_MAJOR, false), (C, -1, 5));
        assert_eq!(spelled(64, C_FLAT_MAJOR, false), (F, -1, 4)); // Fb4 sounds as E4
    }

    #[test]
    fn test_spelling_sounds_the_same_pitch() {
        // letter + alteration + octave must always give back the original MIDI number
        for root in [
            C_MAJOR,
            G_MAJOR,
            D_MAJOR,
            F_MAJOR,
            B_FLAT_MAJOR,
            C_SHARP_MAJOR,
            C_FLAT_MAJOR,
        ] {
            for midi in 24u8..=108 {
                for descending in [false, true] {
                    let p = spell_pitch(midi, &key(root), descending);
                    let sounding =
                        (p.octave + 1) * 12 + NATURAL_PITCH_CLASS[p.letter] + p.alteration;
                    assert_eq!(sounding, midi as i32, "key {root}, midi {midi}");
                    assert!(p.alteration.abs() <= 1);
                }
            }
        }
    }

    fn signs(midis: &[u8], root: u8) -> Vec<Option<AccidentalSign>> {
        spell_measure(midis, &key(root))
            .iter()
            .map(|n| n.accidental)
            .collect()
    }

    #[test]
    fn test_notes_in_the_key_need_no_accidental() {
        assert_eq!(signs(&[60, 62, 64, 65, 67], C_MAJOR), vec![None; 5]);
        // G major: F# (66) is in the key
        assert_eq!(signs(&[66, 67, 66], G_MAJOR), vec![None; 3]);
        // Bb major: Bb (70) is in the key
        assert_eq!(signs(&[70, 69, 70], B_FLAT_MAJOR), vec![None; 3]);
        assert_eq!(signs(&[65, 72], C_SHARP_MAJOR), vec![None; 2]);
    }

    #[test]
    fn test_chromatic_note_gets_a_sign_and_repeats_do_not() {
        use AccidentalSign::*;
        assert_eq!(signs(&[61, 61, 61], C_MAJOR), vec![Some(Sharp), None, None]);
        assert_eq!(signs(&[61], F_MAJOR), vec![Some(Flat)]);
    }

    #[test]
    fn test_natural_cancels_the_key_signature() {
        use AccidentalSign::*;
        // G major: F natural needs a natural sign, and a following F# needs its sharp back
        assert_eq!(
            signs(&[65, 65, 66], G_MAJOR),
            vec![Some(Natural), None, Some(Sharp)]
        );
        // F major: B natural, then B flat again
        assert_eq!(signs(&[71, 70], F_MAJOR), vec![Some(Natural), Some(Flat)]);
    }

    #[test]
    fn test_returning_to_the_natural_needs_a_natural_sign() {
        use AccidentalSign::*;
        // C#, then C natural, then C# again
        assert_eq!(
            signs(&[61, 60, 61], C_MAJOR),
            vec![Some(Sharp), Some(Natural), Some(Sharp)]
        );
    }

    #[test]
    fn test_accidentals_apply_to_one_octave_only() {
        use AccidentalSign::*;
        // C#4 then C#5 each need their own sharp. C natural in another octave needs nothing.
        assert_eq!(signs(&[61, 73], C_MAJOR), vec![Some(Sharp), Some(Sharp)]);
        assert_eq!(signs(&[61, 72], C_MAJOR), vec![Some(Sharp), None]);
    }

    #[test]
    fn test_accidentals_do_not_carry_to_the_next_measure() {
        use AccidentalSign::*;
        let c_sharp = [61u8];
        let first = spell_measure(&c_sharp, &key(C_MAJOR));
        let second = spell_measure(&c_sharp, &key(C_MAJOR));
        assert_eq!(first[0].accidental, Some(Sharp));
        assert_eq!(second[0].accidental, Some(Sharp));
        // C natural right after: no sign, because the key signature applies again
        assert_eq!(spell_measure(&[60], &key(C_MAJOR))[0].accidental, None);
    }

    #[test]
    fn test_falling_chromatic_note_in_c_major_uses_a_flat() {
        use AccidentalSign::*;
        let notes = spell_measure(&[62, 61], &key(C_MAJOR));
        assert_eq!(notes[0].accidental, None);
        assert_eq!(notes[1].pitch.letter, D);
        assert_eq!(notes[1].accidental, Some(Flat));
    }

    #[test]
    fn test_respelling_the_same_pitch_changes_the_sign() {
        use AccidentalSign::*;
        // C major: 61 rising is C#, then 62, then 61 falling is Db. Different letters, so both get signs.
        let notes = spell_measure(&[61, 62, 61], &key(C_MAJOR));
        assert_eq!(notes[0].accidental, Some(Sharp));
        assert_eq!(notes[1].accidental, None);
        assert_eq!(notes[2].accidental, Some(Flat));
        assert_eq!(notes[2].pitch.letter, D);
    }

    #[test]
    fn test_empty_measure_has_no_notes() {
        assert!(spell_measure(&[], &key(C_MAJOR)).is_empty());
    }
}

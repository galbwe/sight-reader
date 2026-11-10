//! Beaming: which notes are beamed together, and how the beams and stems are laid out.
//!
//! Grouping works on exact integer time (`TICKS_PER_QUARTER` ticks per quarter note). Geometry
//! works in staff spaces: y = 0 is the bottom staff line and y grows upward, so the middle line
//! is at y = 2 and the top line at y = 4. The PDF renderer converts to points.
//!
//! Sources: Gould, *Behind Bars* (Beams, pp. 17 to 23, and Beaming according to the metre,
//! p. 153), Ted Ross, and Dorico's documentation of beam slants.

use crate::domain::{Note, NoteDuration, TimeSignature};

/// Resolution of the beaming clock. Divides evenly into every note value down to a 64th, with
/// dots and triplets.
pub const TICKS_PER_QUARTER: u32 = 96;
/// Thickness of a beam in staff spaces, measured vertically
pub const BEAM_THICKNESS: f32 = 0.5;
/// Gap between two beams in staff spaces
pub const BEAM_GAP: f32 = 0.25;
/// Shortest stem under a beam, from the notehead centre to the outer edge of the beam. Gould: 3.5
/// for a note in a space and 3.25 for a note on a line, so that the beam can sit or hang on a line.
const MIN_BEAMED_STEM: f32 = 3.25;
/// The innermost beam's centre must be at least this far from the notehead centre. This puts the
/// semiquaver beam in its usual place and makes the stem longer for 3 or more beams.
const INNERMOST_BEAM_CENTER: f32 = 2.25;
const MIDDLE_LINE_Y: f32 = 2.0;
const MIDDLE_LINE_STEPS: i32 = 4;
/// Beam slant per step of the interval between the first and last note (Dorico: 0.25 for a second)
const SLANT_PER_STEP: f32 = 0.25;
/// Largest slant of a beam, in staff spaces. Gould: beams usually cross no more than one line.
const MAX_SLANT: f32 = 1.0;
/// Groups of four or more notes may slant a little more
const MAX_SLANT_LONG_GROUP: f32 = 1.25;
/// Notes closer together than this (in spaces) only take a slight slant. Gould: 3 spaces.
const CLOSE_NOTES_SPACING: f32 = 3.0;
const CLOSE_NOTES_MAX_SLANT: f32 = 0.5;
/// Cost of a slant that differs from the wanted one, against the cost of stem length
const SLANT_WEIGHT: f32 = 3.0;

/// Number of beams (or flags) a note value carries: 0 for quarter notes and longer
pub fn flag_count(duration: NoteDuration) -> u8 {
    match duration {
        NoteDuration::Eighth => 1,
        NoteDuration::Sixteenth => 2,
        NoteDuration::ThirtySecond => 3,
        NoteDuration::SixtyFourth => 4,
        _ => 0,
    }
}

/// Length of a note in ticks
pub fn duration_ticks(note: &Note) -> u32 {
    let mut ticks = note.duration.beats_in_4_4() * TICKS_PER_QUARTER as f64;
    if note.dotted {
        ticks *= 1.5;
    }
    if note.is_triplet {
        ticks *= 2.0 / 3.0;
    }
    ticks.round() as u32
}

/// Start of each note in ticks from the start of the measure
pub fn note_onsets(notes: &[Note]) -> Vec<u32> {
    let mut onset = 0u32;
    notes
        .iter()
        .map(|note| {
            let start = onset;
            onset += duration_ticks(note);
            start
        })
        .collect()
}

/// Length of one beat in ticks.
///
/// The beat is the note value of the denominator, except in compound time (3/8, 6/8, 9/8, 12/8,
/// 6/16 and so on) where it is three of them.
pub fn beat_ticks(time: &TimeSignature) -> u32 {
    let unit = match time.denominator {
        1 => 4 * TICKS_PER_QUARTER,
        2 => 2 * TICKS_PER_QUARTER,
        4 => TICKS_PER_QUARTER,
        8 => TICKS_PER_QUARTER / 2,
        16 => TICKS_PER_QUARTER / 4,
        32 => TICKS_PER_QUARTER / 8,
        _ => TICKS_PER_QUARTER,
    };
    let compound = time.denominator >= 8 && time.numerator % 3 == 0;
    if compound {
        unit * 3
    } else {
        unit
    }
}

/// How many beats a run of eighth notes (and nothing shorter) may span when beamed together:
/// half a bar in 4/4, a whole bar in 2/4. Other time signatures beam each beat on its own.
fn eighth_span_in_beats(time: &TimeSignature) -> Option<u32> {
    match (time.numerator, time.denominator) {
        (4, 4) | (2, 4) => Some(2),
        _ => None,
    }
}

/// A run of consecutive notes of a measure that share beams: `start..end` indexes into the notes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeamGroup {
    pub start: usize,
    /// One past the last note
    pub end: usize,
}

impl BeamGroup {
    pub fn len(&self) -> usize {
        self.end - self.start
    }
}

/// Decide which notes of a measure are beamed together.
///
/// - Notes shorter than a quarter are beamed with the notes of the same beat, in all metres.
/// - A beam never crosses a barline, because each measure is grouped on its own, and never
///   crosses the middle of a 4/4 bar.
/// - A run of eighths only, with nothing shorter, may continue across beats within half a bar of
///   4/4, or the whole of a 2/4 bar.
/// - A note that is alone in its beat keeps its flag.
pub fn beam_groups(notes: &[Note], time: &TimeSignature) -> Vec<BeamGroup> {
    let beat = beat_ticks(time);

    let info: Vec<(u32, u8)> = note_onsets(notes)
        .into_iter()
        .zip(notes)
        .map(|(onset, note)| (onset, flag_count(note.duration)))
        .collect();

    // Runs of consecutive beamable notes that start in the same beat
    struct Run {
        start: usize,
        end: usize,
        first_beat: u32,
        last_beat: u32,
    }
    let mut runs: Vec<Run> = Vec::new();
    let mut i = 0;
    while i < notes.len() {
        if info[i].1 == 0 {
            i += 1;
            continue;
        }
        let beat_index = info[i].0 / beat;
        let mut j = i + 1;
        while j < notes.len() && info[j].1 > 0 && info[j].0 / beat == beat_index {
            j += 1;
        }
        runs.push(Run { start: i, end: j, first_beat: beat_index, last_beat: beat_index });
        i = j;
    }

    if let Some(span) = eighth_span_in_beats(time) {
        let mut merged: Vec<Run> = Vec::new();
        for run in runs {
            let joins_previous = merged.last().is_some_and(|previous| {
                previous.end == run.start
                    && previous.first_beat / span == run.first_beat / span
                    && info[previous.start..run.end].iter().all(|&(_, flags)| flags == 1)
            });
            if joins_previous {
                let previous = merged.last_mut().expect("checked above");
                previous.end = run.end;
                previous.last_beat = run.last_beat;
            } else {
                merged.push(run);
            }
        }
        runs = merged;
    }

    runs.into_iter()
        .filter(|run| run.end - run.start >= 2)
        .map(|run| BeamGroup { start: run.start, end: run.end })
        .collect()
}

/// Decide the stem direction of a beamed group from the staff positions of its notes (steps above
/// the bottom line; the middle line is 4). The majority of the notes above or below the middle
/// line decides. If they are level, the note furthest from the middle line decides, and if that
/// is a tie too the stems go down. `true` means stems up.
pub fn group_stems_up(positions: &[i32]) -> bool {
    let above = positions.iter().filter(|&&p| p > MIDDLE_LINE_STEPS).count();
    let below = positions.iter().filter(|&&p| p < MIDDLE_LINE_STEPS).count();
    if below != above {
        return below > above;
    }
    let furthest_below = positions.iter().filter(|&&p| p < MIDDLE_LINE_STEPS).map(|p| MIDDLE_LINE_STEPS - p).max();
    let furthest_above = positions.iter().filter(|&&p| p > MIDDLE_LINE_STEPS).map(|p| p - MIDDLE_LINE_STEPS).max();
    furthest_below.unwrap_or(0) > furthest_above.unwrap_or(0)
}

/// Whether a beam stub (a fractional beam on a note whose neighbours have fewer beams) points
/// right. At beam level `level` the note's unit is `TICKS_PER_QUARTER >> level` ticks long. A
/// note on the first half of its parent unit points right, toward its partner, and one on the
/// second half points left. A sixteenth after a dotted eighth therefore points back at it.
pub fn stub_points_right(onset: u32, level: u8) -> bool {
    let unit = (TICKS_PER_QUARTER >> level.min(6)).max(1);
    (onset / unit) % 2 == 0
}

/// One note of a beamed group, in staff spaces
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeamNote {
    /// Notehead centre
    pub x: f32,
    /// Notehead centre, above the bottom staff line
    pub y: f32,
    /// Staff position in steps above the bottom line
    pub position: i32,
    /// Number of beams this note carries (1 for eighths, 2 for sixteenths)
    pub beams: u8,
    /// Start of the note in ticks from the start of the measure
    pub onset: u32,
}

/// Measurements of the noteheads and stems, in staff spaces
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeamMetrics {
    /// Half the width of a notehead where the stem attaches
    pub notehead_half_width: f32,
    /// How far above or below the notehead centre the stem starts
    pub stem_anchor_dy: f32,
    pub stem_width: f32,
}

/// A stem of a beamed note
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeamedStem {
    pub x: f32,
    pub start_y: f32,
    pub end_y: f32,
}

/// One beam (or stub), drawn as a parallelogram with vertical edges.
///
/// `(x1, y1)` and `(x2, y2)` are the points on the edge nearest the end of the stems.
/// The beam is `BEAM_THICKNESS` thick, toward the noteheads (`toward_heads` is -1 for stems up and
/// 1 for stems down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Beam {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub toward_heads: f32,
    /// 1 for the outer beam, 2 for the next one in, and so on
    pub level: u8,
}

/// The stems and beams of a beamed group
#[derive(Debug, Clone, PartialEq)]
pub struct BeamedGroup {
    pub up: bool,
    pub stems: Vec<BeamedStem>,
    pub beams: Vec<Beam>,
}

/// Shortest stem a note with `beams` beams may have, measured from its notehead centre to the
/// outer edge of the outer beam. Every extra beam beyond two needs another beam's worth of room.
pub fn min_stem_length(beams: u8) -> f32 {
    let inner = INNERMOST_BEAM_CENTER + BEAM_THICKNESS / 2.0 + (beams.max(1) - 1) as f32 * (BEAM_THICKNESS + BEAM_GAP);
    MIN_BEAMED_STEM.max(inner)
}

/// Wanted slant of the beam in staff spaces (positive rises), from the notes' contour.
///
/// The beam follows the interval between the first and last note, a quarter space per step up to
/// a limit. It is flat when those two notes are the same, when an inner note is closer to the beam
/// than both outer notes (a concave group), or when most of the notes move against the outer
/// interval.
fn target_slant(notes: &[BeamNote], stem_x: &[f32], up: bool) -> f32 {
    let first = notes.first().expect("a group has notes");
    let last = notes.last().expect("a group has notes");
    let steps = last.position - first.position;
    if steps == 0 {
        return 0.0;
    }

    let direction = if up { 1.0 } else { -1.0 };
    let closer_to_beam = |note: &BeamNote| direction * note.y;
    let outer_best = closer_to_beam(first).max(closer_to_beam(last));
    let concave = notes[1..notes.len() - 1].iter().any(|note| closer_to_beam(note) > outer_best + 1e-4);
    if concave {
        return 0.0;
    }

    let rises = notes.windows(2).filter(|w| w[1].position > w[0].position).count();
    let falls = notes.windows(2).filter(|w| w[1].position < w[0].position).count();
    if (steps > 0 && falls > rises) || (steps < 0 && rises > falls) {
        return 0.0;
    }

    let mut limit = if notes.len() >= 4 { MAX_SLANT_LONG_GROUP } else { MAX_SLANT };
    let average_gap = (stem_x[stem_x.len() - 1] - stem_x[0]) / (notes.len() - 1) as f32;
    if average_gap < CLOSE_NOTES_SPACING {
        limit = limit.min(CLOSE_NOTES_MAX_SLANT);
    }
    let magnitude = (SLANT_PER_STEP * steps.abs() as f32).min(limit);
    let quantized = (magnitude / 0.25).round() * 0.25;
    quantized * steps.signum() as f32
}

/// Lay out the stems and beams of a group of two or more notes.
///
/// The stem direction follows `group_stems_up`. The beam line is chosen so that
/// - each end of the outer beam sits on a staff line, hangs from one, or is centred on one
///   (Ross's sit, hang or straddle rule, which keeps beams from ending inside a space),
/// - every stem is at least as long as `min_stem_length` allows and reaches the middle line,
/// - the slant is as close as possible to the contour of the notes, and
/// - the stems are no longer than they need to be.
pub fn layout_group(notes: &[BeamNote], metrics: &BeamMetrics) -> BeamedGroup {
    assert!(notes.len() >= 2, "a beamed group needs at least two notes");

    let positions: Vec<i32> = notes.iter().map(|note| note.position).collect();
    let up = group_stems_up(&positions);
    let d = if up { 1.0f32 } else { -1.0 };

    // Everything below is in "stem coordinates": u = d * y, so a larger u is further out along
    // the stems
    let attach = metrics.notehead_half_width - metrics.stem_width / 2.0;
    let stem_x: Vec<f32> = notes.iter().map(|note| note.x + d * attach).collect();
    let required: Vec<f32> = notes
        .iter()
        .map(|note| (d * note.y + min_stem_length(note.beams)).max(d * MIDDLE_LINE_Y))
        .collect();

    let x_first = stem_x[0];
    let span = stem_x[stem_x.len() - 1] - x_first;
    let fraction: Vec<f32> = stem_x
        .iter()
        .map(|&x| if span > 0.0 { (x - x_first) / span } else { 0.0 })
        .collect();

    let target_y = target_slant(notes, &stem_x, up);
    let max_slant = (target_y.abs() + 0.75).min(1.5);

    // Beam ends may be where the outer edge is on a line or a quarter space further out
    let low = required.iter().cloned().fold(f32::MAX, f32::min) - 1.0;
    let high = required.iter().cloned().fold(f32::MIN, f32::max) + 4.0;
    let mut allowed: Vec<f32> = Vec::new();
    let mut line = low.floor();
    while line <= high {
        for offset in [0.0, 0.25] {
            let value = line + offset;
            if value >= low && value <= high {
                allowed.push(value);
            }
        }
        line += 1.0;
    }

    let mut best: Option<(f32, f32, f32)> = None;
    for &first_tip in &allowed {
        for &last_tip in &allowed {
            let slant_u = last_tip - first_tip;
            let slant_y = d * slant_u;
            if target_y == 0.0 {
                if slant_u.abs() > 1e-6 {
                    continue;
                }
            } else if slant_y * target_y < -1e-6 || slant_y.abs() > max_slant {
                continue;
            }

            let mut extra = 0.0;
            let mut fits = true;
            for (i, &needed) in required.iter().enumerate() {
                let tip = first_tip + slant_u * fraction[i];
                if tip < needed - 1e-4 {
                    fits = false;
                    break;
                }
                extra += tip - needed;
            }
            if !fits {
                continue;
            }

            let cost = SLANT_WEIGHT * (slant_y - target_y).abs() + extra / notes.len() as f32;
            if best.map_or(true, |(best_cost, _, _)| cost < best_cost - 1e-6) {
                best = Some((cost, first_tip, last_tip));
            }
        }
    }
    let (first_tip, last_tip) = match best {
        Some((_, first_tip, last_tip)) => (first_tip, last_tip),
        None => {
            // Cannot happen with the slants allowed above, but never fail to draw
            let level = required.iter().cloned().fold(f32::MIN, f32::max);
            (level, level)
        }
    };

    // Position of the outer edge of the outer beam at any x
    let tip_y_at = |x: f32| -> f32 {
        let t = if span > 0.0 { (x - x_first) / span } else { 0.0 };
        d * (first_tip + (last_tip - first_tip) * t)
    };

    let stems: Vec<BeamedStem> = notes
        .iter()
        .zip(&stem_x)
        .map(|(note, &x)| BeamedStem {
            x,
            start_y: note.y + d * metrics.stem_anchor_dy,
            end_y: tip_y_at(x),
        })
        .collect();

    let half_stem = metrics.stem_width / 2.0;
    let toward_heads = -d;
    let mut beams = vec![Beam {
        x1: x_first - half_stem,
        y1: tip_y_at(x_first - half_stem),
        x2: stem_x[stem_x.len() - 1] + half_stem,
        y2: tip_y_at(stem_x[stem_x.len() - 1] + half_stem),
        toward_heads,
        level: 1,
    }];

    // Inner beams: one for every further level, offset toward the noteheads
    let deepest = notes.iter().map(|note| note.beams).max().unwrap_or(1);
    let stub_length = metrics.notehead_half_width * 2.0;
    for level in 2..=deepest {
        let offset = toward_heads * (level - 1) as f32 * (BEAM_THICKNESS + BEAM_GAP);
        let mut i = 0;
        while i < notes.len() {
            if notes[i].beams < level {
                i += 1;
                continue;
            }
            let mut j = i;
            while j + 1 < notes.len() && notes[j + 1].beams >= level {
                j += 1;
            }

            let (x1, x2) = if j > i {
                (stem_x[i] - half_stem, stem_x[j] + half_stem)
            } else {
                // A single note with this many beams gets a stub, pointing toward its partner
                let mut right = stub_points_right(notes[i].onset, level);
                if right && i == notes.len() - 1 {
                    right = false;
                } else if !right && i == 0 {
                    right = true;
                }
                if right {
                    (stem_x[i] - half_stem, stem_x[i] + stub_length)
                } else {
                    (stem_x[i] - stub_length, stem_x[i] + half_stem)
                }
            };
            beams.push(Beam {
                x1,
                y1: tip_y_at(x1) + offset,
                x2,
                y2: tip_y_at(x2) + offset,
                toward_heads,
                level,
            });
            i = j + 1;
        }
    }

    BeamedGroup { up, stems, beams }
}

#[cfg(test)]
mod tests {
    use super::*;
    use NoteDuration::*;

    fn note(duration: NoteDuration) -> Note {
        Note::new(60, duration)
    }

    fn dotted(duration: NoteDuration) -> Note {
        Note::new(60, duration).dotted()
    }

    fn groups(notes: &[Note], numerator: u8, denominator: u8) -> Vec<(usize, usize)> {
        beam_groups(notes, &TimeSignature::new(numerator, denominator))
            .iter()
            .map(|g| (g.start, g.end))
            .collect()
    }

    fn eighths(count: usize) -> Vec<Note> {
        vec![note(Eighth); count]
    }

    // ----- grouping -----

    #[test]
    fn test_flag_counts() {
        assert_eq!(flag_count(Whole), 0);
        assert_eq!(flag_count(Quarter), 0);
        assert_eq!(flag_count(Eighth), 1);
        assert_eq!(flag_count(Sixteenth), 2);
        assert_eq!(flag_count(ThirtySecond), 3);
        assert_eq!(flag_count(SixtyFourth), 4);
    }

    #[test]
    fn test_duration_ticks() {
        assert_eq!(duration_ticks(&note(Quarter)), 96);
        assert_eq!(duration_ticks(&note(Eighth)), 48);
        assert_eq!(duration_ticks(&note(Sixteenth)), 24);
        assert_eq!(duration_ticks(&note(SixtyFourth)), 6);
        assert_eq!(duration_ticks(&note(Whole)), 384);
        assert_eq!(duration_ticks(&dotted(Eighth)), 72);
        assert_eq!(duration_ticks(&dotted(Quarter)), 144);
        assert_eq!(duration_ticks(&note(Eighth).triplet()), 32);
    }

    #[test]
    fn test_beat_lengths() {
        let q = TICKS_PER_QUARTER;
        assert_eq!(beat_ticks(&TimeSignature::new(4, 4)), q);
        assert_eq!(beat_ticks(&TimeSignature::new(3, 4)), q);
        assert_eq!(beat_ticks(&TimeSignature::new(2, 2)), 2 * q);
        assert_eq!(beat_ticks(&TimeSignature::new(6, 8)), 3 * q / 2); // dotted quarter
        assert_eq!(beat_ticks(&TimeSignature::new(9, 8)), 3 * q / 2);
        assert_eq!(beat_ticks(&TimeSignature::new(12, 8)), 3 * q / 2);
        assert_eq!(beat_ticks(&TimeSignature::new(3, 8)), 3 * q / 2);
        assert_eq!(beat_ticks(&TimeSignature::new(5, 8)), q / 2);
        assert_eq!(beat_ticks(&TimeSignature::new(6, 16)), 3 * q / 4);
    }

    #[test]
    fn test_four_eighths_in_two_four_form_one_group() {
        assert_eq!(groups(&eighths(4), 2, 4), vec![(0, 4)]);
    }

    #[test]
    fn test_eight_eighths_in_four_four_beam_in_half_bars() {
        assert_eq!(groups(&eighths(8), 4, 4), vec![(0, 4), (4, 8)]);
    }

    #[test]
    fn test_eighths_never_beam_across_the_middle_of_a_four_four_bar() {
        // Eighth on beat 2 and eighth on beat 3 stay apart
        let notes = vec![note(Quarter), note(Eighth), note(Eighth), note(Eighth), note(Eighth), note(Quarter)];
        let found = groups(&notes, 4, 4);
        assert_eq!(found, vec![(1, 3), (3, 5)]);
    }

    #[test]
    fn test_three_four_eighths_beam_by_the_beat() {
        assert_eq!(groups(&eighths(6), 3, 4), vec![(0, 2), (2, 4), (4, 6)]);
    }

    #[test]
    fn test_six_eight_beams_in_threes() {
        assert_eq!(groups(&eighths(6), 6, 8), vec![(0, 3), (3, 6)]);
        assert_eq!(groups(&eighths(9), 9, 8), vec![(0, 3), (3, 6), (6, 9)]);
        assert_eq!(groups(&eighths(12), 12, 8).len(), 4);
    }

    #[test]
    fn test_three_eight_beams_the_whole_bar() {
        assert_eq!(groups(&eighths(3), 3, 8), vec![(0, 3)]);
    }

    #[test]
    fn test_two_two_beams_four_eighths_to_the_beat() {
        assert_eq!(groups(&eighths(8), 2, 2), vec![(0, 4), (4, 8)]);
    }

    #[test]
    fn test_sixteenths_beam_by_the_beat_not_by_the_half_bar() {
        let notes = vec![note(Sixteenth); 8];
        assert_eq!(groups(&notes, 4, 4), vec![(0, 4), (4, 8)]);
        // Eight sixteenths over four quarter beats: still never merged across beats
        let mut mixed = vec![note(Sixteenth); 4];
        mixed.extend(vec![note(Eighth); 2]);
        assert_eq!(groups(&mixed, 4, 4), vec![(0, 4), (4, 6)]);
    }

    #[test]
    fn test_a_shorter_note_stops_eighths_from_merging_across_beats() {
        // Beat 1: two eighths. Beat 2: eighth plus two sixteenths. They stay in separate groups.
        let notes = vec![note(Eighth), note(Eighth), note(Eighth), note(Sixteenth), note(Sixteenth)];
        assert_eq!(groups(&notes, 4, 4), vec![(0, 2), (2, 5)]);
    }

    #[test]
    fn test_longer_notes_split_groups() {
        // 8th 8th | quarter | 8th 8th | quarter
        let notes = vec![note(Eighth), note(Eighth), note(Quarter), note(Eighth), note(Eighth), note(Quarter)];
        assert_eq!(groups(&notes, 4, 4), vec![(0, 2), (3, 5)]);
    }

    #[test]
    fn test_a_lone_eighth_keeps_its_flag() {
        let notes = vec![note(Quarter), note(Eighth), note(Quarter), note(Quarter), note(Eighth)];
        // The eighth at the start of beat 2 is alone, and the last eighth falls in beat 5
        assert!(groups(&notes, 4, 4).is_empty());
    }

    #[test]
    fn test_quarters_and_longer_notes_are_never_beamed() {
        let notes = vec![note(Quarter), note(Half), note(Quarter)];
        assert!(groups(&notes, 4, 4).is_empty());
        assert!(groups(&[note(Whole)], 4, 4).is_empty());
    }

    #[test]
    fn test_notes_starting_off_the_beat_do_not_join_the_next_beat() {
        // Dotted quarter, then eighth (off the beat), then eighth on beat 3
        let notes = vec![dotted(Quarter), note(Eighth), note(Eighth), note(Quarter)];
        let found = groups(&notes, 4, 4);
        // The eighth at 1.5 is in beat 2 and the next at 2.0 in beat 3: different halves of the bar
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn test_dotted_eighth_and_sixteenth_beam_together() {
        let notes = vec![dotted(Eighth), note(Sixteenth), dotted(Eighth), note(Sixteenth)];
        assert_eq!(groups(&notes, 4, 4), vec![(0, 2), (2, 4)]);
    }

    #[test]
    fn test_sixteenth_before_a_dotted_eighth() {
        let notes = vec![note(Sixteenth), dotted(Eighth), note(Sixteenth), dotted(Eighth)];
        // 16th + dotted eighth fill a beat, and so do the next two, but a dotted eighth ends at
        // 1.0 only if started at 0.25: 0.25 + 0.75 = 1.0
        assert_eq!(groups(&notes, 4, 4), vec![(0, 2), (2, 4)]);
    }

    #[test]
    fn test_no_notes() {
        assert!(groups(&[], 4, 4).is_empty());
    }

    // ----- stem direction -----

    #[test]
    fn test_majority_decides_stem_direction() {
        // Positions 0 to 8, middle line is 4
        assert!(group_stems_up(&[0, 1, 6])); // two below, one above
        assert!(!group_stems_up(&[6, 7, 0])); // two above, one below
    }

    #[test]
    fn test_furthest_note_decides_when_level() {
        assert!(group_stems_up(&[-2, 6])); // C4 is 6 away, 6 is 2 away
        assert!(!group_stems_up(&[2, 9])); // G5 is 5 away, 2 is 2 away
    }

    #[test]
    fn test_stems_go_down_on_a_complete_tie() {
        assert!(!group_stems_up(&[2, 6])); // both 2 from the middle
        assert!(!group_stems_up(&[4, 4]));
        assert!(!group_stems_up(&[4, 4, 4, 4]));
    }

    #[test]
    fn test_middle_line_notes_do_not_vote() {
        assert!(group_stems_up(&[4, 3])); // one below, none above
        assert!(!group_stems_up(&[4, 5]));
    }

    // ----- stubs -----

    #[test]
    fn test_stub_direction_follows_the_beat() {
        let s = TICKS_PER_QUARTER / 4; // a sixteenth
        // First sixteenth of an eighth (before a dotted eighth): points right
        assert!(stub_points_right(0, 2));
        // Sixteenth on the second half of its eighth (after a dotted eighth): points left
        assert!(!stub_points_right(3 * s, 2));
        // The same pattern shifted to the second half of the beat
        assert!(stub_points_right(2 * s, 2));
        assert!(!stub_points_right(5 * s, 2));
        // Thirty-seconds work the same way one level down
        assert!(stub_points_right(0, 3));
        assert!(!stub_points_right(TICKS_PER_QUARTER / 8, 3));
    }

    // ----- geometry -----

    const METRICS: BeamMetrics = BeamMetrics { notehead_half_width: 0.63, stem_anchor_dy: 0.126, stem_width: 0.12 };

    fn beam_note(index: usize, position: i32, beams: u8) -> BeamNote {
        BeamNote {
            x: 3.0 + index as f32 * 4.0,
            y: position as f32 / 2.0,
            position,
            beams,
            onset: index as u32 * 48,
        }
    }

    fn eighth_group(positions: &[i32]) -> Vec<BeamNote> {
        positions.iter().enumerate().map(|(i, &p)| beam_note(i, p, 1)).collect()
    }

    fn outer_beam(group: &BeamedGroup) -> Beam {
        *group.beams.iter().find(|b| b.level == 1).unwrap()
    }

    /// The beam edge in stem coordinates at the stems: a multiple of 0.25 on or beside a line
    fn is_snapped(value: f32) -> bool {
        let fractional = value.rem_euclid(1.0);
        fractional < 1e-3 || (fractional - 0.25).abs() < 1e-3 || (fractional - 1.0).abs() < 1e-3
    }

    #[test]
    fn test_level_pair_makes_a_flat_beam_and_equal_stems() {
        let group = layout_group(&eighth_group(&[2, 2]), &METRICS);
        let beam = outer_beam(&group);
        assert_eq!(beam.y1, beam.y2);
        assert!((group.stems[0].end_y - group.stems[1].end_y).abs() < 1e-6);
        assert!(group.up);
    }

    #[test]
    fn test_stems_end_on_the_beam() {
        for positions in [vec![0, 1, 2, 3], vec![8, 7, 6], vec![2, 5, 1, 4], vec![-2, 0]] {
            let group = layout_group(&eighth_group(&positions), &METRICS);
            let beam = outer_beam(&group);
            for stem in &group.stems {
                let t = (stem.x - beam.x1) / (beam.x2 - beam.x1);
                let beam_y = beam.y1 + (beam.y2 - beam.y1) * t;
                assert!((stem.end_y - beam_y).abs() < 1e-3, "{positions:?}: stem ends at {} beam at {beam_y}", stem.end_y);
            }
        }
    }

    #[test]
    fn test_every_stem_is_long_enough() {
        for positions in [vec![0, 1, 2, 3], vec![8, 7, 6, 5], vec![2, 5, 1, 4], vec![-4, -3], vec![11, 12, 10]] {
            let notes = eighth_group(&positions);
            let group = layout_group(&notes, &METRICS);
            for (note, stem) in notes.iter().zip(&group.stems) {
                let length = (stem.end_y - note.y).abs();
                assert!(length >= 3.25 - 1e-3, "{positions:?}: stem of {length} spaces");
            }
        }
    }

    #[test]
    fn test_stems_reach_the_middle_line_for_far_away_notes() {
        // Two ledger lines below the staff: the stems must at least reach the middle line
        let group = layout_group(&eighth_group(&[-4, -3]), &METRICS);
        assert!(group.up);
        for stem in &group.stems {
            assert!(stem.end_y >= MIDDLE_LINE_Y - 1e-4);
        }
        // And far above
        let group = layout_group(&eighth_group(&[12, 11]), &METRICS);
        assert!(!group.up);
        for stem in &group.stems {
            assert!(stem.end_y <= MIDDLE_LINE_Y + 1e-4);
        }
    }

    #[test]
    fn test_beam_ends_sit_hang_or_straddle() {
        for positions in [vec![0, 0], vec![1, 1], vec![1, 3], vec![2, 4, 5], vec![7, 6], vec![6, 5, 4, 3]] {
            let group = layout_group(&eighth_group(&positions), &METRICS);
            let d = if group.up { 1.0 } else { -1.0 };
            let beam = outer_beam(&group);
            // Stem coordinates of the stem ends at the first and last stem
            let first = d * group.stems[0].end_y;
            let last = d * group.stems[group.stems.len() - 1].end_y;
            assert!(is_snapped(first), "{positions:?}: first end at {first}");
            assert!(is_snapped(last), "{positions:?}: last end at {last}");
            assert!(beam.x2 > beam.x1);
        }
    }

    #[test]
    fn test_rising_notes_slope_up_and_falling_notes_slope_down() {
        let rising = layout_group(&eighth_group(&[0, 1, 2, 3]), &METRICS);
        let beam = outer_beam(&rising);
        assert!(beam.y2 > beam.y1 + 0.2, "slant {}", beam.y2 - beam.y1);

        let falling = layout_group(&eighth_group(&[3, 2, 1, 0]), &METRICS);
        let beam = outer_beam(&falling);
        assert!(beam.y2 < beam.y1 - 0.2);
    }

    #[test]
    fn test_slant_follows_the_outer_interval_not_the_middle() {
        // Outer notes rise by 2 steps while the middle dips and stays below: still slopes up
        let group = layout_group(&eighth_group(&[2, 1, 4]), &METRICS);
        let beam = outer_beam(&group);
        assert!(beam.y2 > beam.y1);
    }

    #[test]
    fn test_beam_is_flat_when_first_and_last_notes_match() {
        let group = layout_group(&eighth_group(&[1, 3, 2, 1]), &METRICS);
        let beam = outer_beam(&group);
        assert!((beam.y1 - beam.y2).abs() < 1e-5);
    }

    #[test]
    fn test_beam_is_flat_for_a_concave_group() {
        // Stems up: the middle notes are higher (closer to the beam) than both outer notes
        let group = layout_group(&eighth_group(&[0, 3, 3, 1]), &METRICS);
        assert!(group.up);
        let beam = outer_beam(&group);
        assert!((beam.y1 - beam.y2).abs() < 1e-5);
    }

    #[test]
    fn test_beam_is_flat_when_most_notes_move_against_the_outer_interval() {
        // First to last rises by one step but three moves fall
        let group = layout_group(&eighth_group(&[3, 2, 1, 0, 4]), &METRICS);
        let beam = outer_beam(&group);
        assert!((beam.y1 - beam.y2).abs() < 1e-5);
    }

    #[test]
    fn test_slant_is_limited() {
        // A two-octave leap must not give a steep beam
        let group = layout_group(&eighth_group(&[-2, 12]), &METRICS);
        let beam = outer_beam(&group);
        assert!((beam.y2 - beam.y1).abs() <= 1.5 + 1e-4);
        let group = layout_group(&eighth_group(&[0, 5]), &METRICS);
        let beam = outer_beam(&group);
        assert!((beam.y2 - beam.y1).abs() <= 1.25 + 1e-4);
    }

    #[test]
    fn test_close_notes_take_only_a_slight_slant() {
        let mut notes = eighth_group(&[0, 6]);
        notes[1].x = notes[0].x + 2.0; // closer than three spaces
        let group = layout_group(&notes, &METRICS);
        let beam = outer_beam(&group);
        assert!((beam.y2 - beam.y1).abs() <= 0.5 + 1e-4);
    }

    #[test]
    fn test_down_stems_mirror_up_stems() {
        // The same shape reflected about the middle line (position p becomes 8 - p)
        let up = layout_group(&eighth_group(&[0, 1, 2, 3]), &METRICS);
        let mirrored: Vec<i32> = [0, 1, 2, 3].iter().map(|p| 8 - p).collect();
        let down = layout_group(&eighth_group(&mirrored), &METRICS);
        assert!(up.up && !down.up);
        for (a, b) in up.stems.iter().zip(&down.stems) {
            // end_y reflects around y = 2
            assert!(((a.end_y - 2.0) + (b.end_y - 2.0)).abs() < 1e-4, "{} vs {}", a.end_y, b.end_y);
        }
    }

    #[test]
    fn test_stems_attach_to_the_correct_side() {
        let up = layout_group(&eighth_group(&[0, 1]), &METRICS);
        let down = layout_group(&eighth_group(&[7, 8]), &METRICS);
        for (note, stem) in eighth_group(&[0, 1]).iter().zip(&up.stems) {
            assert!(stem.x > note.x, "up-stems belong on the right");
        }
        for (note, stem) in eighth_group(&[7, 8]).iter().zip(&down.stems) {
            assert!(stem.x < note.x, "down-stems belong on the left");
        }
    }

    #[test]
    fn test_stems_start_at_the_notehead_edge_and_go_toward_the_beam() {
        let notes = eighth_group(&[0, 1]);
        let group = layout_group(&notes, &METRICS);
        for (note, stem) in notes.iter().zip(&group.stems) {
            assert!(stem.start_y > note.y - 1e-6 && stem.end_y > stem.start_y);
        }
        let notes = eighth_group(&[7, 8]);
        let group = layout_group(&notes, &METRICS);
        for (note, stem) in notes.iter().zip(&group.stems) {
            assert!(stem.start_y < note.y + 1e-6 && stem.end_y < stem.start_y);
        }
    }

    #[test]
    fn test_beam_spans_from_the_first_to_the_last_stem() {
        let group = layout_group(&eighth_group(&[0, 1, 2]), &METRICS);
        let beam = outer_beam(&group);
        let first = group.stems.first().unwrap().x;
        let last = group.stems.last().unwrap().x;
        assert!((beam.x1 - (first - 0.06)).abs() < 1e-5);
        assert!((beam.x2 - (last + 0.06)).abs() < 1e-5);
        assert_eq!(beam.toward_heads, -1.0); // stems up: the beam thickens downward
    }

    fn sixteenth_group(positions: &[i32]) -> Vec<BeamNote> {
        positions
            .iter()
            .enumerate()
            .map(|(i, &p)| BeamNote { onset: i as u32 * 24, ..beam_note(i, p, 2) })
            .collect()
    }

    #[test]
    fn test_two_beams_stack_toward_the_noteheads() {
        let group = layout_group(&sixteenth_group(&[0, 1, 2, 3]), &METRICS);
        assert_eq!(group.beams.len(), 2);
        let outer = group.beams.iter().find(|b| b.level == 1).unwrap();
        let inner = group.beams.iter().find(|b| b.level == 2).unwrap();
        // Same span, one beam thickness plus a gap closer to the heads
        assert!((outer.x1 - inner.x1).abs() < 1e-5 && (outer.x2 - inner.x2).abs() < 1e-5);
        let step = BEAM_THICKNESS + BEAM_GAP;
        assert!((outer.y1 - inner.y1 - step).abs() < 1e-4);
        assert!((outer.y2 - inner.y2 - step).abs() < 1e-4);
    }

    #[test]
    fn test_down_stem_inner_beam_is_above_the_outer_beam() {
        let group = layout_group(&sixteenth_group(&[8, 7, 6, 5]), &METRICS);
        assert!(!group.up);
        let outer = group.beams.iter().find(|b| b.level == 1).unwrap();
        let inner = group.beams.iter().find(|b| b.level == 2).unwrap();
        assert!(inner.y1 > outer.y1);
        assert_eq!(outer.toward_heads, 1.0);
    }

    #[test]
    fn test_three_and_four_beams_need_longer_stems() {
        let two = layout_group(&sixteenth_group(&[2, 2]), &METRICS);
        let three: Vec<BeamNote> = (0..2).map(|i| beam_note(i, 2, 3)).collect();
        let four: Vec<BeamNote> = (0..2).map(|i| beam_note(i, 2, 4)).collect();
        let length = |g: &BeamedGroup| g.stems[0].end_y - g.stems[0].start_y;
        let l2 = length(&two);
        let l3 = length(&layout_group(&three, &METRICS));
        let l4 = length(&layout_group(&four, &METRICS));
        assert!(l3 > l2 + 0.5 && l4 > l3 + 0.5, "{l2} {l3} {l4}");
    }

    #[test]
    fn test_beams_never_get_closer_to_noteheads_than_allowed() {
        for beams in 1..=4u8 {
            let notes: Vec<BeamNote> = (0..3).map(|i| beam_note(i, [1, 2, 3][i], beams)).collect();
            let group = layout_group(&notes, &METRICS);
            let innermost = group.beams.iter().map(|b| b.level).max().unwrap();
            let inner = group.beams.iter().find(|b| b.level == innermost).unwrap();
            for (note, stem) in notes.iter().zip(&group.stems) {
                let t = (stem.x - inner.x1) / (inner.x2 - inner.x1);
                let edge = inner.y1 + (inner.y2 - inner.y1) * t;
                let centre = edge + inner.toward_heads * BEAM_THICKNESS / 2.0;
                assert!((centre - note.y).abs() >= INNERMOST_BEAM_CENTER - 1e-3, "{beams} beams: centre {centre}");
            }
        }
    }

    #[test]
    fn test_sixteenth_after_dotted_eighth_gets_a_stub_pointing_back() {
        // Dotted eighth (1 beam), sixteenth (2 beams) at tick 72
        let notes = vec![
            BeamNote { onset: 0, ..beam_note(0, 2, 1) },
            BeamNote { onset: 72, ..beam_note(1, 2, 2) },
        ];
        let group = layout_group(&notes, &METRICS);
        let stub = group.beams.iter().find(|b| b.level == 2).expect("a stub");
        let stem = group.stems[1];
        assert!(stub.x1 < stem.x && stub.x2 > stem.x - 1e-6, "stub should point left of the stem");
        assert!(stub.x2 - stub.x1 > 0.9 && stub.x2 - stub.x1 < 1.6, "stub is about a notehead long");
        assert!(stub.x1 >= group.stems[0].x, "stub stays inside the group");
    }

    #[test]
    fn test_sixteenth_before_dotted_eighth_gets_a_stub_pointing_forward() {
        let notes = vec![
            BeamNote { onset: 0, ..beam_note(0, 2, 2) },
            BeamNote { onset: 24, ..beam_note(1, 2, 1) },
        ];
        let group = layout_group(&notes, &METRICS);
        let stub = group.beams.iter().find(|b| b.level == 2).expect("a stub");
        let stem = group.stems[0];
        assert!(stub.x2 > stem.x + 0.9);
        assert!(stub.x1 <= stem.x);
        assert!(stub.x2 <= group.stems[1].x + 1e-3, "stub stays inside the group");
    }

    #[test]
    fn test_stubs_flip_inward_at_the_ends_of_a_group() {
        // A lone sixteenth that would point out of the group on the second half of a beat,
        // as the last note, would point left; as the first note it must point right
        let first_note = vec![
            BeamNote { onset: 24, ..beam_note(0, 2, 2) },
            BeamNote { onset: 48, ..beam_note(1, 2, 1) },
        ];
        let group = layout_group(&first_note, &METRICS);
        let stub = group.beams.iter().find(|b| b.level == 2).unwrap();
        assert!(stub.x2 > group.stems[0].x + 0.9, "first note: stub must point into the group");

        let last_note = vec![
            BeamNote { onset: 0, ..beam_note(0, 2, 1) },
            BeamNote { onset: 0, ..beam_note(1, 2, 2) },
        ];
        let group = layout_group(&last_note, &METRICS);
        let stub = group.beams.iter().find(|b| b.level == 2).unwrap();
        assert!(stub.x1 < group.stems[1].x - 0.9, "last note: stub must point back into the group");
    }

    #[test]
    fn test_two_sixteenths_among_eighths_share_a_beam() {
        // 8th, 16th, 16th, 8th: the inner beam joins only the two sixteenths
        let notes = vec![
            beam_note(0, 2, 1),
            BeamNote { onset: 48, ..beam_note(1, 3, 2) },
            BeamNote { onset: 72, ..beam_note(2, 3, 2) },
            BeamNote { onset: 96, ..beam_note(3, 2, 1) },
        ];
        let group = layout_group(&notes, &METRICS);
        let inner: Vec<&Beam> = group.beams.iter().filter(|b| b.level == 2).collect();
        assert_eq!(inner.len(), 1);
        assert!(inner[0].x1 <= group.stems[1].x && inner[0].x2 >= group.stems[2].x);
        assert!(inner[0].x2 < group.stems[3].x, "the last eighth has no second beam");
        assert!(inner[0].x1 > group.stems[0].x);
    }

    #[test]
    fn test_layout_is_deterministic() {
        let notes = eighth_group(&[1, 3, 2, 4]);
        assert_eq!(layout_group(&notes, &METRICS), layout_group(&notes, &METRICS));
    }

    #[test]
    fn test_every_group_shape_gets_a_layout_that_satisfies_the_rules() {
        // Sweep many shapes: all stems are long enough and end on the beam line
        let mut count = 0;
        for a in -3..=11 {
            for b in -3..=11 {
                for c in [None, Some(0), Some(4), Some(8)] {
                    let mut positions = vec![a, b];
                    if let Some(extra) = c {
                        positions.insert(1, extra);
                    }
                    let notes = eighth_group(&positions);
                    let group = layout_group(&notes, &METRICS);
                    let beam = outer_beam(&group);
                    for (note, stem) in notes.iter().zip(&group.stems) {
                        let length = (stem.end_y - note.y).abs();
                        assert!(length >= 3.25 - 1e-3, "{positions:?}: stem {length}");
                        let t = (stem.x - beam.x1) / (beam.x2 - beam.x1);
                        let beam_y = beam.y1 + (beam.y2 - beam.y1) * t;
                        assert!((stem.end_y - beam_y).abs() < 1e-3);
                    }
                    assert!((beam.y2 - beam.y1).abs() <= 1.5 + 1e-4, "{positions:?}");
                    count += 1;
                }
            }
        }
        assert!(count > 800);
    }

    #[test]
    #[should_panic]
    fn test_a_single_note_is_not_a_group() {
        layout_group(&eighth_group(&[2]), &METRICS);
    }
}

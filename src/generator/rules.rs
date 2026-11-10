//! Rules for guided musical phrase generation
//!
//! Melodies move through the notes of the key's scale. A move is a number of scale steps, up or
//! down: 1 step is a second, 2 steps a third, and so on up to 7 steps for an octave. The rules
//! give each possible move a weight. Steps are the most likely, big leaps are rare, a leap is
//! usually followed by a step back the other way, and the melody is gently pulled back toward the
//! middle of the range so that it wanders up and down instead of climbing to one end.

use crate::domain::{Interval, Note};
use crate::generator::config::{DirectionBias, IntervalSelection};

/// Largest move considered, in scale steps (7 steps is an octave)
const MAX_MOVE_STEPS: i32 = 7;
/// A move of this many scale steps or more (a fourth and up) counts as a leap
const LEAP_STEPS: i32 = 3;
/// Base weight of a move by its size in scale steps: unison, second, third, fourth, fifth,
/// sixth, seventh, octave
const BASE_WEIGHTS: [u32; 8] = [2, 16, 8, 4, 3, 2, 1, 2];
/// How strongly the middle of the range attracts the melody. Added to the weight factor so that
/// the edge of the range is still reachable.
const MIDDLE_PULL_BASE: i32 = 4;

/// One possible move through the scale: a signed number of scale steps and its likelihood
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    /// Positive moves up the scale, negative moves down, 0 repeats the note
    pub steps: i32,
    pub weight: u32,
}

/// Rules for generating musically sound exercises
pub struct GenerationRules {
    interval_selection: IntervalSelection,
    max_leap_semitones: i32,
    prefer_stepwise_motion: bool,
    avoid_large_leaps_in_succession: bool,
}

impl GenerationRules {
    /// Create new generation rules based on interval selection
    pub fn new(interval_selection: &IntervalSelection) -> Self {
        GenerationRules {
            interval_selection: interval_selection.clone(),
            max_leap_semitones: 12, // an octave
            prefer_stepwise_motion: true,
            avoid_large_leaps_in_succession: true,
        }
    }

    /// Whether the user's interval selection allows a move of `distance` scale steps
    fn is_distance_enabled(&self, distance: i32) -> bool {
        match distance {
            0 => self.interval_selection.unisons,
            1 => self.interval_selection.steps,
            2 | 3 => self.interval_selection.skips,
            4..=6 => self.interval_selection.leaps,
            7 => self.interval_selection.octave,
            _ => false,
        }
    }

    /// All moves the melody could make from `scale[current]`, with their weights.
    ///
    /// `scale` holds the notes of the key in the available range, lowest first, so a move of N
    /// steps lands N entries further along. `previous_move` is the move that led to the current
    /// note (0 at the start). Moves that leave the range, are not enabled, or span more than an
    /// octave are left out. The list is empty only when no enabled move fits.
    pub fn candidate_moves(&self, scale: &[u8], current: usize, previous_move: i32) -> Vec<Move> {
        let len = scale.len() as i32;
        let middle = (len - 1) / 2;
        let half = len / 2;
        let mut moves = Vec::new();

        for distance in 0..=MAX_MOVE_STEPS {
            if !self.is_distance_enabled(distance) {
                continue;
            }
            for direction in [1, -1] {
                if distance == 0 && direction == -1 {
                    continue;
                }
                let steps = distance * direction;
                let target = current as i32 + steps;
                if target < 0 || target >= len {
                    continue;
                }
                let semitones = (scale[target as usize] as i32 - scale[current] as i32).abs();
                if semitones > self.max_leap_semitones {
                    continue;
                }

                let mut weight = if self.prefer_stepwise_motion {
                    BASE_WEIGHTS[distance as usize]
                } else {
                    1
                };

                match self.interval_selection.direction_bias {
                    DirectionBias::LeaningUp if steps > 0 => weight *= 2,
                    DirectionBias::LeaningDown if steps < 0 => weight *= 2,
                    _ => {}
                }

                // After a leap, recover: a step back the other way is likely, another leap or
                // carrying on in the same direction is not
                if self.avoid_large_leaps_in_succession && previous_move.abs() >= LEAP_STEPS {
                    if distance >= LEAP_STEPS {
                        weight = (weight / 4).max(1);
                    } else if distance > 0 && steps.signum() == previous_move.signum() {
                        weight = (weight / 2).max(1);
                    } else if distance == 1 && steps.signum() == -previous_move.signum() {
                        weight *= 2;
                    }
                }

                // Pull toward the middle of the range, so melodies do not stick to one end
                let from_middle = (target - middle).abs();
                weight *= (MIDDLE_PULL_BASE + (half - from_middle).max(0)) as u32;

                moves.push(Move { steps, weight });
            }
        }
        moves
    }

    /// Pick a move at random, each in proportion to its weight. `roll` is any random number.
    pub fn pick_move(moves: &[Move], roll: u32) -> Option<Move> {
        let total: u32 = moves.iter().map(|m| m.weight).sum();
        if total == 0 {
            return None;
        }
        let mut remaining = roll % total;
        for candidate in moves {
            if remaining < candidate.weight {
                return Some(*candidate);
            }
            remaining -= candidate.weight;
        }
        None
    }

    /// Check if two successive notes would create an awkward leap
    pub fn would_awkward_leap(&self, first: &Note, second: &Note) -> bool {
        if !self.avoid_large_leaps_in_succession {
            return false;
        }

        let interval = (second.midi_number as i32 - first.midi_number as i32).abs();
        interval > 12 // More than an octave
    }

    /// Get a list of recommended intervals for a given difficulty level
    pub fn recommended_intervals(&self, difficulty: u8) -> Vec<Interval> {
        match difficulty {
            1..=2 => vec![
                Interval::unison(),
                Interval::major_second(),
                Interval::minor_third(),
                Interval::major_third(),
            ],
            3..=4 => vec![
                Interval::unison(),
                Interval::major_second(),
                Interval::minor_third(),
                Interval::major_third(),
                Interval::perfect_fourth(),
            ],
            5..=7 => vec![
                Interval::unison(),
                Interval::major_second(),
                Interval::minor_third(),
                Interval::major_third(),
                Interval::perfect_fourth(),
                Interval::perfect_fifth(),
            ],
            8..=10 => vec![
                Interval::unison(),
                Interval::major_second(),
                Interval::minor_third(),
                Interval::major_third(),
                Interval::perfect_fourth(),
                Interval::tritone(),
                Interval::perfect_fifth(),
                Interval::minor_sixth(),
                Interval::major_sixth(),
                Interval::minor_seventh(),
                Interval::major_seventh(),
                Interval::octave(),
            ],
            _ => vec![Interval::octave()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// C4 to B5 in C major: 14 notes
    fn scale() -> Vec<u8> {
        crate::generator::melody::scale_notes(&crate::domain::Key::c_major(), 60, 83)
    }

    fn rules(selection: IntervalSelection) -> GenerationRules {
        GenerationRules::new(&selection)
    }

    fn weight_of(moves: &[Move], steps: i32) -> u32 {
        moves.iter().find(|m| m.steps == steps).map_or(0, |m| m.weight)
    }

    #[test]
    fn test_moves_stay_inside_the_scale() {
        let rules = rules(IntervalSelection::default());
        let scale = scale();
        for current in 0..scale.len() {
            for m in rules.candidate_moves(&scale, current, 0) {
                let target = current as i32 + m.steps;
                assert!(target >= 0 && (target as usize) < scale.len());
                assert!(m.weight > 0);
            }
        }
    }

    #[test]
    fn test_no_downward_moves_from_the_lowest_note_and_no_upward_from_the_highest() {
        let rules = rules(IntervalSelection::default());
        let scale = scale();
        assert!(rules.candidate_moves(&scale, 0, 0).iter().all(|m| m.steps >= 0));
        let top = scale.len() - 1;
        assert!(rules.candidate_moves(&scale, top, 0).iter().all(|m| m.steps <= 0));
    }

    #[test]
    fn test_steps_are_more_likely_than_leaps() {
        let rules = rules(IntervalSelection::default());
        let scale = scale();
        let moves = rules.candidate_moves(&scale, 6, 0);
        assert!(weight_of(&moves, 1) > weight_of(&moves, 2));
        assert!(weight_of(&moves, 2) > weight_of(&moves, 3));
        assert!(weight_of(&moves, 3) > weight_of(&moves, 5));
        assert!(weight_of(&moves, -1) > weight_of(&moves, -4));
    }

    #[test]
    fn test_steps_only_selection_offers_only_steps_and_repeats() {
        let rules = rules(IntervalSelection::steps_only());
        let scale = scale();
        let sizes: Vec<i32> = rules.candidate_moves(&scale, 6, 0).iter().map(|m| m.steps.abs()).collect();
        assert!(sizes.iter().all(|&s| s <= 1));
        assert!(sizes.contains(&1) && sizes.contains(&0));
    }

    #[test]
    fn test_skips_cover_thirds_and_fourths_only() {
        let mut selection = IntervalSelection::steps_only();
        selection.skips = true;
        let rules = rules(selection);
        let scale = scale();
        let sizes: Vec<i32> = rules.candidate_moves(&scale, 6, 0).iter().map(|m| m.steps.abs()).collect();
        assert!(sizes.iter().all(|&s| s <= 3));
        assert!(sizes.contains(&2) && sizes.contains(&3));
    }

    #[test]
    fn test_octave_is_only_offered_when_enabled() {
        let scale = scale();
        let mut selection = IntervalSelection::default();
        selection.octave = false;
        let without = rules(selection).candidate_moves(&scale, 3, 0);
        assert!(without.iter().all(|m| m.steps.abs() < 7));
        let with = rules(IntervalSelection::default()).candidate_moves(&scale, 3, 0);
        assert!(with.iter().any(|m| m.steps == 7));
        // An octave is exactly 12 semitones in a diatonic scale, which is allowed
        assert_eq!(scale[10] - scale[3], 12);
    }

    #[test]
    fn test_nothing_enabled_gives_no_moves() {
        let selection = IntervalSelection {
            steps: false,
            skips: false,
            leaps: false,
            unisons: false,
            octave: false,
            direction_bias: DirectionBias::Balanced,
        };
        assert!(rules(selection).candidate_moves(&scale(), 5, 0).is_empty());
    }

    #[test]
    fn test_after_a_leap_up_a_step_down_is_favoured() {
        let rules = rules(IntervalSelection::default());
        let scale = scale();
        let fresh = rules.candidate_moves(&scale, 7, 0);
        let after_leap = rules.candidate_moves(&scale, 7, 4);
        // Stepping back down is boosted, carrying on upward is held back, another leap is rare
        assert!(weight_of(&after_leap, -1) > weight_of(&fresh, -1));
        assert!(weight_of(&after_leap, 1) < weight_of(&fresh, 1));
        assert!(weight_of(&after_leap, 5) < weight_of(&fresh, 5));
        assert!(weight_of(&after_leap, -5) < weight_of(&fresh, -5));
    }

    #[test]
    fn test_after_a_leap_down_a_step_up_is_favoured() {
        let rules = rules(IntervalSelection::default());
        let scale = scale();
        let fresh = rules.candidate_moves(&scale, 7, 0);
        let after_leap = rules.candidate_moves(&scale, 7, -4);
        assert!(weight_of(&after_leap, 1) > weight_of(&fresh, 1));
        assert!(weight_of(&after_leap, -1) < weight_of(&fresh, -1));
    }

    #[test]
    fn test_a_small_move_changes_nothing() {
        let rules = rules(IntervalSelection::default());
        let scale = scale();
        assert_eq!(rules.candidate_moves(&scale, 7, 0), rules.candidate_moves(&scale, 7, 1));
        assert_eq!(rules.candidate_moves(&scale, 7, 0), rules.candidate_moves(&scale, 7, -2));
    }

    #[test]
    fn test_direction_bias() {
        let scale = scale();
        let mut selection = IntervalSelection::default();
        selection.direction_bias = DirectionBias::LeaningUp;
        let up = rules(selection.clone()).candidate_moves(&scale, 6, 0);
        let balanced = rules(IntervalSelection::default()).candidate_moves(&scale, 6, 0);
        assert!(weight_of(&up, 1) > weight_of(&balanced, 1));
        assert_eq!(weight_of(&up, -1), weight_of(&balanced, -1));
        selection.direction_bias = DirectionBias::LeaningDown;
        let down = rules(selection).candidate_moves(&scale, 6, 0);
        assert!(weight_of(&down, -1) > weight_of(&balanced, -1));
        assert_eq!(weight_of(&down, 1), weight_of(&balanced, 1));
    }

    #[test]
    fn test_melody_is_pulled_back_toward_the_middle() {
        let rules = rules(IntervalSelection::default());
        let scale = scale();
        // Near the bottom, a step up (toward the middle) is likelier than a step down
        let low = rules.candidate_moves(&scale, 2, 0);
        assert!(weight_of(&low, 1) > weight_of(&low, -1));
        // Near the top, a step down is likelier than a step up
        let high = rules.candidate_moves(&scale, scale.len() - 3, 0);
        assert!(weight_of(&high, -1) > weight_of(&high, 1));
        // In the middle there is no preference between equal steps
        let middle = rules.candidate_moves(&scale, 6, 0);
        let mid_up = weight_of(&middle, 1);
        let mid_down = weight_of(&middle, -1);
        assert!(mid_up.abs_diff(mid_down) <= mid_up / 4);
    }

    #[test]
    fn test_the_edges_stay_reachable() {
        let rules = rules(IntervalSelection::default());
        let scale = scale();
        let moves = rules.candidate_moves(&scale, 12, 0);
        assert!(weight_of(&moves, 1) > 0);
    }

    #[test]
    fn test_pick_move_is_proportional_to_weight() {
        let moves = [Move { steps: 1, weight: 1 }, Move { steps: -1, weight: 3 }];
        assert_eq!(GenerationRules::pick_move(&moves, 0).unwrap().steps, 1);
        for roll in 1..4 {
            assert_eq!(GenerationRules::pick_move(&moves, roll).unwrap().steps, -1);
        }
        assert_eq!(GenerationRules::pick_move(&moves, 4).unwrap().steps, 1); // wraps around
        assert!(GenerationRules::pick_move(&[], 7).is_none());
    }

    #[test]
    fn test_recommended_intervals() {
        let config = crate::generator::IntervalSelection::default();
        let rules = GenerationRules::new(&config);
        
        let easy = rules.recommended_intervals(2);
        assert!(easy.len() <= 4);
        
        let hard = rules.recommended_intervals(10);
        assert!(hard.len() > 4);
    }
}

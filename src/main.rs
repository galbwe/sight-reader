//! Sight Reading Trainer - A Rust application for generating sight reading exercises

mod domain;
mod generator;
mod presentation;

use domain::{Clef, Key, KeyMode, TimeSignature};
use generator::{ExerciseGenerator, GeneratorConfig, OctaveRange};
use presentation::get_renderer;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let options = match parse_args(&args) {
        Ok(Command::Run(options)) => options,
        Ok(Command::Help) => {
            print_help();
            return;
        }
        Err(message) => {
            eprintln!("Error: {}\n", message);
            eprintln!("Run with --help to see the options.");
            std::process::exit(1);
        }
    };
    let clef = options.clef;
    let num_measures = options.num_measures;
    let output_format = options.output_format;
    let output_file = options.output_file;

    // Create configuration
    let config = GeneratorConfig {
        key: options.key,
        time_signature: TimeSignature::common_time(),
        range: options.range,
        duration_selection: options.duration_selection,
        interval_selection: options.interval_selection,
    };
    
    // println!("DEBUG: Selected key: {}", config.key);
    // println!("DEBUG: Key signature: {}", config.key.signature());

    // Create generator
    let generator = ExerciseGenerator::new(config);
    
    // Generate exercise
    let measures = generator.generate_exercise(num_measures);
    
    // Get renderer and output
    match output_format.as_str() {
        "ascii" => {
            println!("Sight Reading Trainer");
            println!("====================\n");
            
            let renderer = get_renderer("ascii");
            let output = renderer.render_measures(&measures);
            
            println!("{}", output);
        }
        "pdf" => {
            let output_path = output_file.unwrap_or_else(|| "exercise.pdf".to_string());
            let pdf_renderer = presentation::PdfNotationRenderer::new().with_clef(clef);
            pdf_renderer.save_to_file(&measures, &output_path)
                .expect(&format!("Failed to save PDF to {}", output_path));
            
            println!("PDF saved to: {}", output_path);
        }
        _ => {
            eprintln!("Unknown output format: {}", output_format);
            std::process::exit(1);
        }
    }
}

/// Everything the command line can ask for
#[derive(Debug)]
struct Options {
    num_measures: u32,
    key: Key,
    clef: Clef,
    range: OctaveRange,
    duration_selection: generator::config::NoteDurationSelection,
    interval_selection: generator::config::IntervalSelection,
    output_format: String,
    output_file: Option<String>,
}

/// What the command line asked the program to do
#[derive(Debug)]
enum Command {
    /// Generate an exercise
    Run(Options),
    /// Print the help text
    Help,
}

/// The value after a flag, or an error saying which flag is missing its value
fn flag_value<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    args.get(index + 1)
        .map(String::as_str)
        .ok_or_else(|| format!("{} requires a value", flag))
}

/// Read the number of octaves given to `--octaves` (`-o`).
///
/// A value that is not a whole number from 1 to `MAX_OCTAVES` is an error. If it looks like a
/// file name, the message points at `--output`, because `-o` used to be a tempting way to name
/// the output file.
fn parse_octaves(value: &str) -> Result<u8, String> {
    match value.trim().parse::<u8>() {
        Ok(count) if (1..=MAX_OCTAVES).contains(&count) => Ok(count),
        _ => {
            let hint = if value.chars().any(|c| c.is_alphabetic()) && !value.starts_with('-') {
                format!(" To name the output file, use --output {}.", value)
            } else {
                String::new()
            };
            Err(format!(
                "--octaves (-o) needs a whole number of octaves from 1 to {}, but got '{}'.{}",
                MAX_OCTAVES, value, hint
            ))
        }
    }
}

fn parse_args(args: &[String]) -> Result<Command, String> {
    let mut num_measures = 4u32;
    let mut clef = Clef::Treble;
    let mut key_root = 60u8; // C
    let key_mode = KeyMode::Major;
    let mut octave_count = DEFAULT_OCTAVES;
    let mut duration_selection = generator::config::NoteDurationSelection::default();
    let mut interval_selection = generator::config::IntervalSelection::default();
    let mut output_format = "ascii".to_string();
    let mut output_file: Option<String> = None;

    // The first argument is the program name
    let mut i = 1;
    while i < args.len() {
        let flag = args[i].as_str();
        match flag {
            "--help" | "-h" => return Ok(Command::Help),
            "--measures" | "-m" => {
                num_measures = flag_value(args, i, "--measures")?.parse().unwrap_or(4);
            }
            "--key" | "-k" => key_root = parse_key(flag_value(args, i, "--key")?),
            "--octaves" | "-o" => octave_count = parse_octaves(flag_value(args, i, "--octaves")?)?,
            "--clef" | "-c" => {
                let name = flag_value(args, i, "--clef")?;
                clef = Clef::from_name(name)
                    .ok_or_else(|| format!("unknown clef '{}' (use treble or bass)", name))?;
            }
            "--durations" | "-d" => duration_selection = parse_durations(flag_value(args, i, "--durations")?),
            "--intervals" | "-i" => interval_selection = parse_intervals(flag_value(args, i, "--intervals")?),
            "--format" | "-f" => output_format = flag_value(args, i, "--format")?.to_string(),
            "--output" => output_file = Some(flag_value(args, i, "--output")?.to_string()),
            _ => return Err(format!("unknown argument '{}'", flag)),
        }
        i += 2;
    }

    let key = Key::new(key_root, key_mode);
    let (min_midi, max_midi) = octave_midi_range(octave_count, clef);
    let range = OctaveRange::new(min_midi, max_midi);

    Ok(Command::Run(Options {
        num_measures,
        key,
        clef,
        range,
        duration_selection,
        interval_selection,
        output_format,
        output_file,
    }))
}

/// Number of octaves an exercise covers when `--octaves` is not given
const DEFAULT_OCTAVES: u8 = 2;

/// Most octaves `--octaves` accepts. MIDI spans a little over ten and a half octaves.
const MAX_OCTAVES: u8 = 10;

/// The lowest and highest MIDI note for an exercise covering `count` octaves in a clef.
///
/// Ranges start on a C and are whole octaves. Octaves use scientific pitch notation, where
/// octave 4 starts at middle C (MIDI 60). The range is the one whose centre is closest to the
/// middle line of the staff, so the melody sits on the staff with a few ledger lines. For the
/// default two octaves that is C4 to B5 in treble clef and C2 to B3 in bass clef.
fn octave_midi_range(count: u8, clef: Clef) -> (u8, u8) {
    let count = count.max(1) as i32;
    let middle_line = clef.middle_line_midi() as f32;
    let centre = |lowest_octave: i32| ((lowest_octave + 1) * 12) as f32 + (count * 12) as f32 / 2.0 - 0.5;

    // On a tie the lower range wins
    let lowest_octave = (0..=9)
        .min_by(|&a, &b| {
            (centre(a) - middle_line)
                .abs()
                .partial_cmp(&(centre(b) - middle_line).abs())
                .expect("distances are numbers")
        })
        .expect("the range is not empty");
    let highest_octave = lowest_octave + count - 1;
    let min_midi = ((lowest_octave + 1) * 12).clamp(0, 127);
    let max_midi = ((highest_octave + 1) * 12 + 11).clamp(0, 127);
    (min_midi as u8, max_midi as u8)
}

fn parse_key(s: &str) -> u8 {
    const NOTES: &[&str] = &["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    const FLATS: &[&str] = &["Db", "Eb", "Gb", "Ab", "Bb"];
    
    let s_lower = s.to_lowercase();
    
    for (i, &flat) in FLATS.iter().enumerate() {
        if s_lower == flat.to_lowercase() {
            // For flat keys, use specific root values that will be recognized
            // by the is_flat_key check in calculate_signature
            let root_note = match flat {
                "Db" => 49,  // 5 flats (Db4 = 4*12 + 1 = 49)
                "Eb" => 51,  // 3 flats (Eb4 = 4*12 + 3 = 51)
                "Gb" => 54,  // 6 flats (Gb4 = 4*12 + 6 = 54)
                "Ab" => 56,  // 4 flats (Ab4 = 4*12 + 8 = 56)
                "Bb" => 58,  // 2 flats (Bb4 = 4*12 + 10 = 58)
                _ => 53,     // F4 as default
            };
            return root_note;
        }
    }
    
    // Check for sharps and naturals
    let s_upper = s.to_uppercase();
    for (i, &note) in NOTES.iter().enumerate() {
        if s_upper == note {
            let octave = 4i32; // Default octave
            return (octave as u8) * 12 + i as u8;
        }
    }
    
    60u8 // Default to C4
}

fn parse_durations(s: &str) -> generator::config::NoteDurationSelection {
    match s.to_lowercase().as_str() {
        "simple" => generator::config::NoteDurationSelection::simple_durations(),
        "quarter" => {
            let mut sel = generator::config::NoteDurationSelection::default();
            sel.whole = generator::config::DurationWeight::Disabled;
            sel.half = generator::config::DurationWeight::Disabled;
            sel.dotted_quarter = false;
            sel.dotted_eighth = false;
            sel.triplet_quarter = false;
            sel.triplet_eighth = false;
            sel
        }
        "dotted" => {
            let mut sel = generator::config::NoteDurationSelection::default();
            sel.whole = generator::config::DurationWeight::Disabled;
            sel.triplet_quarter = false;
            sel.triplet_eighth = false;
            sel
        }
        _ => generator::config::NoteDurationSelection::default(),
    }
}

fn parse_intervals(s: &str) -> generator::config::IntervalSelection {
    match s.to_lowercase().as_str() {
        "steps" => generator::config::IntervalSelection::steps_only(),
        "steps-skips" => generator::config::IntervalSelection::steps_and_skips(),
        "large" => generator::config::IntervalSelection::larger_intervals(),
        _ => generator::config::IntervalSelection::default(),
    }
}

fn print_help() {
    println!("Sight Reading Trainer - Generate sight reading exercises");
    println!("\nUsage: sight-reading [OPTIONS]");
    println!("\nOptions:");
    println!("  -m, --measures <COUNT>   Number of measures to generate (default: 4)");
    println!("  -k, --key <KEY>          Key (C, C#, D, D#, E, F, F#, G, G#, A, A#, B)");
    println!("  -c, --clef <CLEF>        Clef: treble, bass (default: treble). Sets the range of notes");
    println!("  -o, --octaves <COUNT>    Number of octaves to cover, 1 to 10 (default: 2)");
    println!("  -d, --durations <TYPE>   Duration types: simple, quarter, dotted (default: all)");
    println!("  -i, --intervals <TYPE>   Interval types: steps, steps-skips, large (default: all)");
    println!("  -f, --format <FORMAT>    Output format: ascii, pdf (default: ascii)");
    println!("      --output <FILE>      Output file path (for PDF; default: exercise.pdf)");
    println!("  -h, --help               Show this help message");
    println!("\nExamples:");
    println!("  sight-reading -m 8 -k G -o 3");
    println!("  sight-reading --measures 16 --key C# --durations quarter");
    println!("  sight-reading --format pdf --output exercise.pdf -k A -m 4");
    println!("  sight-reading --format pdf --output bass.pdf --clef bass -k F -m 8");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_range_is_c4_to_b5_in_treble() {
        assert_eq!(octave_midi_range(DEFAULT_OCTAVES, Clef::Treble), (60, 83));
    }

    #[test]
    fn test_default_range_is_c2_to_b3_in_bass() {
        assert_eq!(octave_midi_range(DEFAULT_OCTAVES, Clef::Bass), (36, 59));
    }

    #[test]
    fn test_treble_octave_counts() {
        assert_eq!(octave_midi_range(1, Clef::Treble), (60, 71)); // C4 to B4
        assert_eq!(octave_midi_range(2, Clef::Treble), (60, 83)); // C4 to B5
        assert_eq!(octave_midi_range(3, Clef::Treble), (48, 83)); // C3 to B5
        assert_eq!(octave_midi_range(4, Clef::Treble), (48, 95)); // C3 to B6
    }

    #[test]
    fn test_bass_octave_counts() {
        assert_eq!(octave_midi_range(1, Clef::Bass), (48, 59)); // C3 to B3
        assert_eq!(octave_midi_range(2, Clef::Bass), (36, 59)); // C2 to B3
        assert_eq!(octave_midi_range(3, Clef::Bass), (36, 71)); // C2 to B4
        assert_eq!(octave_midi_range(4, Clef::Bass), (24, 71)); // C1 to B4
    }

    #[test]
    fn test_octave_range_spans_the_requested_number_of_octaves() {
        for clef in [Clef::Treble, Clef::Bass] {
            for count in 1u8..=8 {
                let (min, max) = octave_midi_range(count, clef);
                assert_eq!(max as i32 - min as i32 + 1, count as i32 * 12, "{clef} count {count}");
                assert_eq!(min % 12, 0);
            }
        }
    }

    #[test]
    fn test_octave_range_is_centred_on_the_middle_line() {
        // Within half an octave of the middle line, so the melody sits on the staff
        for clef in [Clef::Treble, Clef::Bass] {
            for count in 1u8..=6 {
                let (min, max) = octave_midi_range(count, clef);
                let centre = (min as f32 + max as f32) / 2.0;
                assert!((centre - clef.middle_line_midi() as f32).abs() <= 6.0, "{clef} count {count}");
            }
        }
    }

    #[test]
    fn test_octave_range_stays_inside_midi_limits() {
        for clef in [Clef::Treble, Clef::Bass] {
            for count in 0u8..=255 {
                let (min, max) = octave_midi_range(count, clef);
                assert!(min <= max && max <= 127);
            }
            assert_eq!(octave_midi_range(0, clef), octave_midi_range(1, clef));
        }
    }

    // ----- command line parsing -----

    fn args(arguments: &[&str]) -> Vec<String> {
        std::iter::once("sight-reading").chain(arguments.iter().copied()).map(String::from).collect()
    }

    fn run(arguments: &[&str]) -> Options {
        match parse_args(&args(arguments)) {
            Ok(Command::Run(options)) => options,
            other => panic!("expected options, got {other:?}"),
        }
    }

    fn error(arguments: &[&str]) -> String {
        match parse_args(&args(arguments)) {
            Err(message) => message,
            other => panic!("expected an error, got {other:?}"),
        }
    }

    #[test]
    fn test_defaults() {
        let options = run(&[]);
        assert_eq!(options.num_measures, 4);
        assert_eq!(options.clef, Clef::Treble);
        assert_eq!((options.range.min_midi(), options.range.max_midi()), (60, 83));
        assert_eq!(options.output_format, "ascii");
        assert_eq!(options.output_file, None);
    }

    #[test]
    fn test_short_o_sets_the_octaves() {
        // The README has always shown -o as the octave count
        let options = run(&["-m", "16", "-k", "G", "-o", "3"]);
        assert_eq!((options.range.min_midi(), options.range.max_midi()), (48, 83));
        assert_eq!(options.num_measures, 16);
    }

    #[test]
    fn test_long_octaves_flag() {
        let options = run(&["--octaves", "1"]);
        assert_eq!((options.range.min_midi(), options.range.max_midi()), (60, 71));
    }

    #[test]
    fn test_octaves_follow_the_clef_in_any_order() {
        let bass_first = run(&["--clef", "bass", "-o", "1"]);
        let bass_last = run(&["-o", "1", "--clef", "bass"]);
        assert_eq!((bass_first.range.min_midi(), bass_first.range.max_midi()), (48, 59));
        assert_eq!((bass_last.range.min_midi(), bass_last.range.max_midi()), (48, 59));
    }

    #[test]
    fn test_output_file_needs_the_long_flag() {
        let options = run(&["--format", "pdf", "--output", "bass.pdf"]);
        assert_eq!(options.output_file.as_deref(), Some("bass.pdf"));
        assert_eq!(options.output_format, "pdf");
    }

    #[test]
    fn test_octaves_and_output_can_be_given_together() {
        let options = run(&["-o", "3", "--output", "x.pdf", "-f", "pdf"]);
        assert_eq!((options.range.min_midi(), options.range.max_midi()), (48, 83));
        assert_eq!(options.output_file.as_deref(), Some("x.pdf"));
    }

    #[test]
    fn test_a_file_name_after_short_o_is_an_error_that_points_at_output() {
        let message = error(&["-o", "exercise.pdf"]);
        assert!(message.contains("--octaves"), "{message}");
        assert!(message.contains("--output exercise.pdf"), "{message}");
        let message = error(&["--octaves", "bass.pdf"]);
        assert!(message.contains("--output bass.pdf"), "{message}");
    }

    #[test]
    fn test_octave_counts_outside_the_range_are_errors() {
        for bad in ["0", "11", "100", "255", "256", "-1", "2.5", ""] {
            let message = error(&["-o", bad]);
            assert!(message.contains("1 to 10"), "{bad}: {message}");
        }
        for good in ["1", "2", "5", "10"] {
            run(&["-o", good]);
        }
    }

    #[test]
    fn test_a_number_does_not_get_the_filename_hint() {
        assert!(!error(&["-o", "11"]).contains("--output"));
        assert!(!error(&["-o", "2.5"]).contains("--output"));
    }

    #[test]
    fn test_a_flag_in_place_of_the_value_does_not_get_a_nonsense_hint() {
        let message = error(&["-o", "--output", "x.pdf"]);
        assert!(message.contains("1 to 10"), "{message}");
        assert!(!message.contains("use --output --output"), "{message}");
    }

    #[test]
    fn test_missing_values() {
        for flag in ["-o", "--octaves", "-k", "--key", "-c", "--clef", "-d", "-i", "-f", "-m", "--output"] {
            let message = error(&[flag]);
            assert!(message.contains("requires a value"), "{flag}: {message}");
        }
    }

    #[test]
    fn test_a_missing_value_names_the_long_flag() {
        assert!(error(&["-o"]).starts_with("--octaves"));
        assert!(error(&["--output"]).starts_with("--output"));
    }

    #[test]
    fn test_unknown_arguments_and_clefs() {
        assert!(error(&["--nope"]).contains("unknown argument '--nope'"));
        assert!(error(&["-x"]).contains("unknown argument '-x'"));
        assert!(error(&["--clef", "alto"]).contains("unknown clef 'alto'"));
    }

    #[test]
    fn test_help() {
        assert!(matches!(parse_args(&args(&["--help"])), Ok(Command::Help)));
        assert!(matches!(parse_args(&args(&["-h"])), Ok(Command::Help)));
        assert!(matches!(parse_args(&args(&["-m", "8", "--help"])), Ok(Command::Help)));
    }

    #[test]
    fn test_the_whole_example_from_the_help_text() {
        let options = run(&["--format", "pdf", "--output", "bass.pdf", "--clef", "bass", "-k", "F", "-m", "8"]);
        assert_eq!(options.clef, Clef::Bass);
        assert_eq!(options.num_measures, 8);
        assert_eq!(options.output_file.as_deref(), Some("bass.pdf"));
        assert_eq!((options.range.min_midi(), options.range.max_midi()), (36, 59));
    }

    #[test]
    fn test_help_text_gives_o_to_octaves_only() {
        // Guard against the old clash coming back: no two options may share a short flag
        let source = include_str!("main.rs");
        let help = source.split("fn print_help()").nth(1).unwrap().split("#[cfg(test)]").next().unwrap();
        let mut shorts: Vec<&str> = help
            .lines()
            .filter_map(|line| line.split("println!(\"").nth(1))
            .filter_map(|line| {
                let line = line.trim_start();
                line.strip_prefix('-').filter(|rest| !rest.starts_with('-')).map(|rest| &rest[..1])
            })
            .collect();
        let total = shorts.len();
        shorts.sort();
        shorts.dedup();
        assert_eq!(shorts.len(), total, "two options share a short flag");
        assert!(total >= 8);
    }
}

//! PDF notation renderer using lopdf library

use super::beaming::{
    beam_groups, flag_count, layout_group, note_onsets, Beam, BeamMetrics, BeamNote, BeamedStem, BEAM_THICKNESS,
};
use super::music_glyphs::{
    Glyph, PathOp, BASS_CLEF, FLAT, HALF_NOTEHEAD, NATURAL, SHARP, TIME_SIG_DIGITS, TREBLE_CLEF, WHOLE_NOTEHEAD,
};
use super::spelling::{spell_measure, AccidentalSign, SpelledNote, SpelledPitch};
use crate::domain::{Clef, KeySignature, Measure, Note, NoteDuration, TimeSignature};
use lopdf::{Document, Object, Stream, Dictionary, ObjectId};
use lopdf::content::{Content, Operation};
use lopdf::dictionary;

/// A4 page size in points
const PAGE_WIDTH: f32 = 595.0;
const PAGE_HEIGHT: f32 = 842.0;
/// Left and right page margin in points
const PAGE_MARGIN_X: f32 = 40.0;
/// Space above the first staff line of a page
const PAGE_MARGIN_TOP: f32 = 60.0;
/// Space below the lowest staff on a page
const PAGE_MARGIN_BOTTOM: f32 = 60.0;
/// Thin barline thickness (Bravura's engraving default is 0.16 staff spaces)
const BARLINE_WIDTH: f32 = STAFF_LINE_GAP * 0.16;
/// Vertical distance between two adjacent staff lines in points
const STAFF_LINE_GAP: f32 = 5.0;
/// Vertical distance between a line and the adjacent space
const STAFF_STEP: f32 = STAFF_LINE_GAP / 2.0;
/// Horizontal padding inside the staff before the first / after the last note
const NOTE_AREA_PADDING: f32 = 15.0;
/// Notehead horizontal radius
const NOTEHEAD_RX: f32 = STAFF_LINE_GAP * 0.65;
/// Notehead vertical radius
const NOTEHEAD_RY: f32 = STAFF_LINE_GAP * 0.42;
/// Notehead tilt in degrees (counter-clockwise)
const NOTEHEAD_TILT_DEG: f32 = 20.0;
/// Control point factor for approximating a quarter ellipse with a cubic Bezier
const BEZIER_CIRCLE_KAPPA: f32 = 0.552_284_8;
/// Standard stem length in staff spaces (one octave), measured from the notehead center
const STEM_LENGTH_SPACES: f32 = 3.5;
/// Stem thickness (Bravura's engraving default is 0.12 staff spaces)
const STEM_LINE_WIDTH: f32 = STAFF_LINE_GAP * 0.12;
/// Length of an eighth-note flag in staff spaces (Gould: 3 to 3.25 is the norm)
const FLAG_TAIL_LENGTH_SPACES: f32 = 3.0;
/// How far a flag sticks out to the right of the stem, in staff spaces
const FLAG_WIDTH_SPACES: f32 = 1.1;
/// Distance between consecutive flags along the stem (Gould: "a little less than a space")
const FLAG_SPACING_SPACES: f32 = 0.8;
/// How much further from the stem end each additional flag reaches (Gould: 1/4 to 1/2 space)
const FLAG_EXTRA_DEPTH_SPACES: f32 = 0.35;
/// Flags that fit on a standard-length stem. Each further flag lengthens the stem.
const FLAGS_ON_STANDARD_STEM: u8 = 2;
/// Outline of a flag in unit coordinates: x is a fraction of the flag width to the right of the
/// stem's left edge, y is a fraction of the flag length measured from the stem end toward the notehead.
/// Starts at (0, 0), then two cubic Beziers: the outer edge down to the tip, then the inner edge back to the stem.
const FLAG_OUTLINE: [[f32; 6]; 2] = [
    [0.45, 0.25, 1.15, 0.40, 0.95, 1.00],
    [0.98, 0.62, 0.50, 0.32, 0.00, 0.17],
];
/// Space before the first symbol of a row, in staff spaces. An estimate.
const HEADER_LEFT_PAD_SPACES: f32 = 1.0;
/// Gap between the clef, key signature and time signature, in staff spaces (Gould: 1 to 1.5)
const SYMBOL_GAP_SPACES: f32 = 1.25;
/// Gap between the accidentals of a key signature, in staff spaces. An estimate.
const KEY_ACCIDENTAL_GAP_SPACES: f32 = 0.2;
/// Staff positions of the key signature sharps in treble clef: F C G D A E B
const TREBLE_SHARP_POSITIONS: [i32; 7] = [8, 5, 9, 6, 3, 7, 4];
/// Staff positions of the key signature flats in treble clef: B E A D G C F
const TREBLE_FLAT_POSITIONS: [i32; 7] = [4, 7, 3, 6, 2, 5, 1];
/// The same patterns in bass clef, a third (two steps) lower. F sharp is on the fourth line and B flat
/// on the second. The last flat, F flat, is in the space just below the staff.
const BASS_SHARP_POSITIONS: [i32; 7] = [6, 3, 7, 4, 1, 5, 2];
const BASS_FLAT_POSITIONS: [i32; 7] = [2, 5, 1, 4, 0, 3, -1];
/// Staff positions on which the numerator and denominator digits are centered.
/// Each digit is 2 spaces tall, so together they fill the staff.
const TIME_NUMERATOR_STEPS: i32 = 6;
const TIME_DENOMINATOR_STEPS: i32 = 2;
/// Gap between an accidental sign and its notehead, in staff spaces. An estimate: Gould says only
/// that the sign closes up as far as it can without colliding.
const ACCIDENTAL_GAP_SPACES: f32 = 0.25;
/// Augmentation dot diameter in staff spaces (Bravura: 2/5 space)
const DOT_DIAMETER_SPACES: f32 = 0.4;
/// Gap between the notehead (or flags) and the augmentation dot in staff spaces.
/// This is an estimate. The sources consulted do not give a number.
const DOT_GAP_SPACES: f32 = 0.5;
/// Number of staff steps from the bottom line to the middle line
const MIDDLE_LINE_STEPS: f32 = 4.0;
/// Number of staff steps from the bottom line to the top line
const TOP_LINE_STEPS: i32 = 8;
/// Staff line thickness (Bravura's engraving default is 0.13 staff spaces)
const STAFF_LINE_WIDTH: f32 = STAFF_LINE_GAP * 0.13;
/// Ledger line thickness: about twice a staff line, so the count is readable at a glance
const LEDGER_LINE_WIDTH: f32 = STAFF_LINE_GAP * 0.26;
/// How far a ledger line extends beyond each side of the notehead (Bravura: 0.4 staff spaces)
const LEDGER_LINE_EXTENSION: f32 = STAFF_LINE_GAP * 0.4;

/// Renderer for PDF notation output
pub struct PdfNotationRenderer {
    staff_lines: u8,
    clef: Clef,
    show_clef: bool,
    show_key_signature: bool,
    show_time_signature: bool,
    /// Preferred measure width in points. Rows are filled with as many measures as fit,
    /// then stretched to span the full width between the page margins.
    measure_width: f32,
    /// Vertical distance between the bottom lines of two consecutive systems (rows) in points
    system_spacing: f32,
    note_spacing: f32,
}

impl PdfNotationRenderer {
    pub fn new() -> Self {
        PdfNotationRenderer {
            staff_lines: 5,
            clef: Clef::Treble,
            show_clef: true,
            show_key_signature: true,
            show_time_signature: true,
            measure_width: 200.0,
            system_spacing: 60.0,
            note_spacing: 15.0,
        }
    }

    /// Write the exercise in this clef. Notes keep their pitch, and move on the staff to match.
    pub fn with_clef(self, clef: Clef) -> Self {
        Self { clef, ..self }
    }

    pub fn with_measure_width(self, width: f32) -> Self {
        Self { measure_width: width, ..self }
    }

    pub fn with_system_spacing(self, spacing: f32) -> Self {
        Self { system_spacing: spacing, ..self }
    }

    pub fn with_note_spacing(self, spacing: f32) -> Self {
        Self { note_spacing: spacing, ..self }
    }

    pub fn without_clef(self) -> Self {
        Self { show_clef: false, ..self }
    }

    pub fn without_key_signature(self) -> Self {
        Self { show_key_signature: false, ..self }
    }

    pub fn without_time_signature(self) -> Self {
        Self { show_time_signature: false, ..self }
    }
}

impl Default for PdfNotationRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Horizontal positions of the clef, key signature and time signature at the start of a row.
///
/// All x values are in points, measured from the left edge of the row. `width` is the total
/// space the header takes, so the first measure of the row starts at `width`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct HeaderLayout {
    clef_x: Option<f32>,
    key_x: Option<f32>,
    time_x: Option<f32>,
    width: f32,
}

/// Where a single measure goes in the document.
#[derive(Debug, Clone, Copy, PartialEq)]
struct MeasureSlot {
    /// Zero-based page index
    page: usize,
    /// Zero-based index of the system (row) over the whole document
    system_index: usize,
    /// Left edge of the measure
    x: f32,
    /// y coordinate of the bottom staff line
    staff_bottom_y: f32,
    width: f32,
    /// Zero-based position of the measure within its system (row)
    index_in_system: usize,
    /// Number of measures in the system this measure belongs to
    system_len: usize,
    /// Header (clef, key and time signature) of this measure's system
    header: HeaderLayout,
}

impl PdfNotationRenderer {
    /// Create a PDF document from measures and save to file
    pub fn save_to_file(&self, measures: &[Measure], filepath: &str) -> Result<(), String> {
        let pages = self.render_pages(measures)?;
        let mut doc = Document::with_version("1.5");

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });
        let font_resources_id = doc.add_object(Object::Dictionary(dictionary! {
            "Font" => dictionary! {
                "F1" => font_id,
            },
        }));

        // Reserve the Pages object id first so every page can point at its parent
        let pages_id = doc.new_object_id();
        let mut page_ids: Vec<ObjectId> = Vec::with_capacity(pages.len());
        for operations in pages {
            page_ids.push(self.create_page(&mut doc, pages_id, font_resources_id, operations)?);
        }

        doc.objects.insert(pages_id, Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => page_ids.iter().map(|id| Object::Reference(*id)).collect::<Vec<_>>(),
            "Count" => page_ids.len() as u32,
        }));

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        doc.save(filepath).map(|_| ()).map_err(|e| format!("Failed to save PDF: {}", e))
    }

    /// Lay out all measures and return one list of drawing operations per page.
    ///
    /// Each page's operations are collected in a single list and written as one content
    /// stream, so measures on the same page never overwrite each other.
    fn render_pages(&self, measures: &[Measure]) -> Result<Vec<Vec<Operation>>, String> {
        let slots = self.layout(measures);
        let page_count = slots.last().map_or(1, |slot| slot.page + 1);
        let mut pages: Vec<Vec<Operation>> = vec![Vec::new(); page_count];

        for (measure, slot) in measures.iter().zip(&slots) {
            self.draw_measure(&mut pages[slot.page], measure, slot)?;
        }
        Ok(pages)
    }

    /// y coordinate of the bottom staff line of the first system on a page
    fn first_staff_bottom_y(&self) -> f32 {
        PAGE_HEIGHT - PAGE_MARGIN_TOP - self.staff_height()
    }

    /// Distance from the bottom to the top staff line
    fn staff_height(&self) -> f32 {
        (self.staff_lines as f32 - 1.0) * STAFF_LINE_GAP
    }

    fn usable_width() -> f32 {
        PAGE_WIDTH - 2.0 * PAGE_MARGIN_X
    }

    fn systems_per_page(&self) -> usize {
        let room = (self.first_staff_bottom_y() - PAGE_MARGIN_BOTTOM).max(0.0);
        (room / self.system_spacing).floor() as usize + 1
    }

    /// Place the clef, key signature and time signature at the start of a row.
    ///
    /// They appear in that order, separated by `SYMBOL_GAP_SPACES`. The clef and key signature
    /// start every row. The time signature is passed in only for the first row of the piece.
    fn header_layout(&self, key: &KeySignature, time: Option<&TimeSignature>) -> HeaderLayout {
        let mut layout = HeaderLayout::default();
        let mut cursor = 0.0f32;
        let mut placed_any = false;
        let mut place = |width_spaces: f32| -> f32 {
            cursor += if placed_any { SYMBOL_GAP_SPACES } else { HEADER_LEFT_PAD_SPACES };
            let x = cursor * STAFF_LINE_GAP;
            cursor += width_spaces;
            placed_any = true;
            x
        };

        if self.show_clef {
            layout.clef_x = Some(place(clef_glyph(self.clef).width));
        }
        if self.show_key_signature && key.accidental_count != 0 {
            layout.key_x = Some(place(key_signature_width(key, self.clef)));
        }
        if let (true, Some(time)) = (self.show_time_signature, time) {
            layout.time_x = Some(place(time_signature_width(time)));
        }
        layout.width = if placed_any { cursor * STAFF_LINE_GAP } else { 0.0 };
        layout
    }

    /// Assign every measure a page, row and horizontal position.
    ///
    /// Rows start with a header (clef and key signature, plus the time signature on the first
    /// row). The measures of a row fill the rest of the width between the margins, so rows with a
    /// smaller header get slightly wider measures. The last row keeps the same measure width as the
    /// others instead of being stretched. Rows go from the top of the page downward, and a new page
    /// starts when a row would cross the bottom margin.
    fn layout(&self, measures: &[Measure]) -> Vec<MeasureSlot> {
        let Some(first) = measures.first() else {
            return Vec::new();
        };
        // The first row has the widest header, so it decides how many measures fit in a row
        let widest_header = self.header_layout(&first.key_signature, Some(&first.time_signature));
        let per_system =
            (((Self::usable_width() - widest_header.width) / self.measure_width).floor() as usize).max(1);
        let systems_per_page = self.systems_per_page();

        (0..measures.len())
            .map(|i| {
                let system = i / per_system;
                let index_in_system = i % per_system;
                let row = system % systems_per_page;
                let row_first = &measures[system * per_system];
                let time = (system == 0).then_some(&row_first.time_signature);
                let header = self.header_layout(&row_first.key_signature, time);
                let width = (Self::usable_width() - header.width) / per_system as f32;
                MeasureSlot {
                    page: system / systems_per_page,
                    system_index: system,
                    x: PAGE_MARGIN_X + header.width + index_in_system as f32 * width,
                    staff_bottom_y: self.first_staff_bottom_y() - row as f32 * self.system_spacing,
                    width,
                    index_in_system,
                    system_len: per_system.min(measures.len() - system * per_system),
                    header,
                }
            })
            .collect()
    }

    /// Create a page object holding `operations` as its single content stream
    fn create_page(
        &self,
        doc: &mut Document,
        parent_id: ObjectId,
        font_resources_id: ObjectId,
        operations: Vec<Operation>,
    ) -> Result<ObjectId, String> {
        let encoded = Content { operations }.encode().map_err(|e| e.to_string())?;
        let content_id = doc.add_object(Stream::new(Dictionary::new(), encoded));

        Ok(doc.add_object(Object::Dictionary(dictionary! {
            "Type" => "Page",
            "Parent" => parent_id,
            "Contents" => content_id,
            "Resources" => font_resources_id,
            "MediaBox" => vec![0.into(), 0.into(), (PAGE_WIDTH as i64).into(), (PAGE_HEIGHT as i64).into()],
        })))
    }

    /// Draw one measure into `operations` at the position given by `slot`.
    ///
    /// The staff lines and header (clef, key and time signature) of a whole system are drawn
    /// together by its first measure, so the lines run unbroken across barlines.
    /// Every measure ends with a thin barline.
    fn draw_measure(
        &self,
        operations: &mut Vec<Operation>,
        measure: &Measure,
        slot: &MeasureSlot,
    ) -> Result<(), String> {
        if slot.index_in_system == 0 {
            let system_width = slot.header.width + slot.width * slot.system_len as f32;
            self.push_staff_lines(operations, PAGE_MARGIN_X, system_width, slot.staff_bottom_y);
            self.draw_header(operations, measure, slot);
        }
        push_barline(operations, slot.x + slot.width, slot.staff_bottom_y, self.staff_height());

        // Spread the notes evenly across the measure, away from the barlines
        let note_area_start = slot.x + NOTE_AREA_PADDING;
        let note_area_width = slot.width - 2.0 * NOTE_AREA_PADDING;
        let note_slot_width = note_area_width / measure.notes.len().max(1) as f32;
        let midi_notes: Vec<u8> = measure.notes.iter().map(|note| note.midi_number).collect();
        let spelled = spell_measure(&midi_notes, &measure.key_signature);
        let note_xs: Vec<f32> = (0..measure.notes.len())
            .map(|i| note_area_start + note_slot_width * (i as f32 + 0.5))
            .collect();

        // Notes shorter than a quarter that share a beat are joined by beams instead of flags
        let (beamed_stems, beams) = beamed_notes(measure, &spelled, &note_xs, slot.staff_bottom_y, self.clef);

        for (i, (note, spelled)) in measure.notes.iter().zip(&spelled).enumerate() {
            self.draw_note(operations, note, spelled, note_xs[i], slot.staff_bottom_y, beamed_stems[i])?;
        }
        for beam in &beams {
            push_beam(operations, beam, slot.staff_bottom_y);
        }
        Ok(())
    }

    /// Draw the clef, key signature and time signature at the start of a system.
    ///
    /// `measure` is the first measure of the system.
    fn draw_header(&self, operations: &mut Vec<Operation>, measure: &Measure, slot: &MeasureSlot) {
        let bottom = slot.staff_bottom_y;
        let at_step = |steps: i32| bottom + steps as f32 * STAFF_STEP;

        if let Some(clef_x) = slot.header.clef_x {
            push_glyph(operations, &clef_glyph(self.clef), PAGE_MARGIN_X + clef_x, at_step(self.clef.reference_line_steps()));
        }

        if let Some(key_x) = slot.header.key_x {
            for (glyph, offset, position) in key_signature_glyphs(&measure.key_signature, self.clef) {
                let x = PAGE_MARGIN_X + key_x + offset * STAFF_LINE_GAP;
                push_glyph(operations, &glyph, x, at_step(position));
            }
        }

        if let Some(time_x) = slot.header.time_x {
            let time = &measure.time_signature;
            let total_width = time_signature_width(time);
            for (value, steps) in [
                (time.numerator, TIME_NUMERATOR_STEPS),
                (time.denominator, TIME_DENOMINATOR_STEPS),
            ] {
                let digits = number_glyphs(value);
                let mut offset = (total_width - run_width(&digits)) / 2.0;
                for digit in digits {
                    push_glyph(operations, &digit, PAGE_MARGIN_X + time_x + offset * STAFF_LINE_GAP, at_step(steps));
                    offset += digit.width;
                }
            }
        }
    }

    /// Draw the horizontal staff lines of one system
    fn push_staff_lines(&self, operations: &mut Vec<Operation>, x: f32, width: f32, staff_bottom_y: f32) {
        operations.push(Operation::new("q", vec![]));
        operations.push(Operation::new("w", vec![STAFF_LINE_WIDTH.into()]));
        for i in 0..self.staff_lines as i32 {
            let line_y = staff_bottom_y + (i as f32) * STAFF_LINE_GAP;
            operations.push(Operation::new("m", vec![x.into(), line_y.into()]));
            operations.push(Operation::new("l", vec![(x + width).into(), line_y.into()]));
            operations.push(Operation::new("S", vec![]));
        }
        operations.push(Operation::new("Q", vec![]));
    }

    /// Draw a single note body (notehead) on a treble staff whose bottom line is at `staff_bottom_y`.
    ///
    /// Stems are drawn for half notes and shorter, with flags for eighth notes and shorter,
    /// and an augmentation dot for dotted notes. `spelled` says which line or space the note sits
    /// on and whether an accidental sign precedes it. A note in a beamed group arrives with its
    /// stem already laid out in `beamed_stem` and gets no flags. The group's beams are drawn by the
    /// caller.
    /// Returns the y coordinate of the notehead center.
    fn draw_note(
        &self,
        operations: &mut Vec<Operation>,
        note: &Note,
        spelled: &SpelledNote,
        x: f32,
        staff_bottom_y: f32,
        beamed_stem: Option<StemGeometry>,
    ) -> Result<f32, String> {
        let position = staff_position(&spelled.pitch, self.clef);
        let y = staff_bottom_y + (position as f32) * STAFF_STEP;

        let half_width = notehead_half_width(note.duration);

        // Ledger lines go first so the notehead is drawn on top of them
        push_ledger_lines(operations, x, staff_bottom_y, position, half_width);

        if let Some(sign) = spelled.accidental {
            let has_ledger_lines = !ledger_line_positions(position).is_empty();
            push_accidental(operations, sign, x, y, has_ledger_lines, half_width);
        }

        // Whole notes have their own wide head. Half notes are hollow, and shorter notes filled.
        match note.duration {
            NoteDuration::Whole => push_whole_notehead(operations, x, y),
            NoteDuration::Half => push_half_notehead(operations, x, y),
            _ => push_notehead(operations, x, y),
        }

        // Draw a stem for half notes and shorter (a whole note has none), with flags unless the
        // note is beamed
        let mut flags = flag_count(note.duration);
        let stem = if let Some(stem) = beamed_stem {
            push_stem(operations, &stem);
            flags = 0;
            Some(stem)
        } else if note.duration != NoteDuration::Whole {
            let staff_middle = staff_bottom_y + MIDDLE_LINE_STEPS * STAFF_STEP;
            let stem = stem_geometry(x, y, staff_middle, flags);
            push_stem(operations, &stem);
            push_flags(operations, &stem, flags);
            Some(stem)
        } else {
            None
        };

        if note.dotted {
            let (dot_x, dot_y) = dot_position(x, y, position, stem.as_ref(), flags, half_width);
            push_dot(operations, dot_x, dot_y);
        }

        Ok(y)
    }
}

/// Push operations that draw `text` with its baseline origin at absolute page position (x, y).
///
/// The text matrix operands are `a b c d e f`; `1 0 0 1` is the identity
/// (no scale/rotation) and `e f` is the translation.
fn push_text(operations: &mut Vec<Operation>, text: &str, font_size: f32, x: f32, y: f32) {
    operations.push(Operation::new("BT", vec![]));
    operations.push(Operation::new("Tf", vec![Object::Name(b"F1".to_vec()), font_size.into()]));
    operations.push(Operation::new(
        "Tm",
        vec![1.0.into(), 0.0.into(), 0.0.into(), 1.0.into(), x.into(), y.into()],
    ));
    operations.push(Operation::new("Tj", vec![Object::string_literal(text)]));
    operations.push(Operation::new("ET", vec![]));
}

/// Number of staff steps (lines and spaces) a written pitch sits above the bottom line of the staff.
/// The bottom line is E4 in treble clef and G2 in bass clef.
fn staff_position(pitch: &SpelledPitch, clef: Clef) -> i32 {
    pitch.diatonic_index() - clef.bottom_line_diatonic_index()
}

/// The glyph for a clef
fn clef_glyph(clef: Clef) -> Glyph {
    match clef {
        Clef::Treble => TREBLE_CLEF,
        Clef::Bass => BASS_CLEF,
    }
}

/// The glyph for an accidental sign
fn accidental_glyph(sign: AccidentalSign) -> Glyph {
    match sign {
        AccidentalSign::Sharp => SHARP,
        AccidentalSign::Flat => FLAT,
        AccidentalSign::Natural => NATURAL,
    }
}

/// x of the accidental glyph's origin (its left edge) for a note centered at `note_x`.
///
/// The sign sits directly left of the notehead, with its right edge `ACCIDENTAL_GAP_SPACES` away.
/// When the note has ledger lines the sign clears their left ends instead.
fn accidental_origin_x(note_x: f32, has_ledger_lines: bool, glyph: &Glyph, half_width: f32) -> f32 {
    let ledger_extension = if has_ledger_lines { LEDGER_LINE_EXTENSION } else { 0.0 };
    note_x - half_width - ledger_extension - (ACCIDENTAL_GAP_SPACES + glyph.width) * STAFF_LINE_GAP
}

/// Push operations that draw an accidental sign in front of the note at (note_x, note_y).
/// The glyphs are centered vertically on their origin, so they sit on the note's line or space.
fn push_accidental(
    operations: &mut Vec<Operation>,
    sign: AccidentalSign,
    note_x: f32,
    note_y: f32,
    has_ledger_lines: bool,
    half_width: f32,
) {
    let glyph = accidental_glyph(sign);
    push_glyph(operations, &glyph, accidental_origin_x(note_x, has_ledger_lines, &glyph, half_width), note_y);
}

/// Push operations that fill a glyph outline with its origin at (x, y).
///
/// Glyph coordinates are in staff spaces, so the transform scales them to points.
fn push_glyph(operations: &mut Vec<Operation>, glyph: &Glyph, x: f32, y: f32) {
    push_glyph_scaled(operations, glyph, x, y, 1.0, 1.0);
}

/// Like `push_glyph`, with the glyph stretched horizontally by `scale_x` and vertically by `scale_y`
fn push_glyph_scaled(operations: &mut Vec<Operation>, glyph: &Glyph, x: f32, y: f32, scale_x: f32, scale_y: f32) {
    operations.push(Operation::new("q", vec![]));
    operations.push(Operation::new(
        "cm",
        vec![
            (STAFF_LINE_GAP * scale_x).into(),
            0.0.into(),
            0.0.into(),
            (STAFF_LINE_GAP * scale_y).into(),
            x.into(),
            y.into(),
        ],
    ));
    for op in glyph.ops {
        operations.push(match *op {
            PathOp::Move(x, y) => Operation::new("m", vec![x.into(), y.into()]),
            PathOp::Line(x, y) => Operation::new("l", vec![x.into(), y.into()]),
            PathOp::Curve(c1x, c1y, c2x, c2y, x, y) => {
                Operation::new("c", vec![c1x.into(), c1y.into(), c2x.into(), c2y.into(), x.into(), y.into()])
            }
            PathOp::Close => Operation::new("h", vec![]),
        });
    }
    operations.push(Operation::new("f", vec![]));
    operations.push(Operation::new("Q", vec![]));
}

/// The accidentals of a key signature in a clef, left to right.
///
/// Each entry is the glyph, its x offset from the start of the key signature in staff spaces,
/// and its staff position in steps above the bottom line. Sharps follow the order F C G D A E B
/// and flats B E A D G C F, each in a fixed octave so that none needs a ledger line.
fn key_signature_glyphs(key: &KeySignature, clef: Clef) -> Vec<(Glyph, f32, i32)> {
    let count = (key.accidental_count.unsigned_abs() as usize).min(7);
    let (glyph, positions) = match (key.accidental_count > 0, clef) {
        (true, Clef::Treble) => (SHARP, TREBLE_SHARP_POSITIONS),
        (false, Clef::Treble) => (FLAT, TREBLE_FLAT_POSITIONS),
        (true, Clef::Bass) => (SHARP, BASS_SHARP_POSITIONS),
        (false, Clef::Bass) => (FLAT, BASS_FLAT_POSITIONS),
    };
    (0..count)
        .map(|i| (glyph, i as f32 * (glyph.width + KEY_ACCIDENTAL_GAP_SPACES), positions[i]))
        .collect()
}

/// Width of a key signature in staff spaces (0 when it has no accidentals)
fn key_signature_width(key: &KeySignature, clef: Clef) -> f32 {
    key_signature_glyphs(key, clef)
        .last()
        .map_or(0.0, |(glyph, offset, _)| offset + glyph.width)
}

/// The digit glyphs that spell `value`, left to right
fn number_glyphs(value: u8) -> Vec<Glyph> {
    value
        .to_string()
        .bytes()
        .map(|digit| TIME_SIG_DIGITS[(digit - b'0') as usize])
        .collect()
}

/// Total width of a row of glyphs in staff spaces
fn run_width(glyphs: &[Glyph]) -> f32 {
    glyphs.iter().map(|glyph| glyph.width).sum()
}

/// Width of a time signature in staff spaces: the wider of the numerator and denominator
fn time_signature_width(time: &TimeSignature) -> f32 {
    run_width(&number_glyphs(time.numerator)).max(run_width(&number_glyphs(time.denominator)))
}

/// Push a closed ellipse path centered at the current origin, made of four cubic Bezier segments.
/// The caller decides how to paint it and sets up the transform.
fn push_ellipse_path(operations: &mut Vec<Operation>, rx: f32, ry: f32) {
    let (kx, ky) = (rx * BEZIER_CIRCLE_KAPPA, ry * BEZIER_CIRCLE_KAPPA);
    operations.push(Operation::new("m", vec![rx.into(), 0.0.into()]));
    let curves: [[f32; 6]; 4] = [
        [rx, ky, kx, ry, 0.0, ry],
        [-kx, ry, -rx, ky, -rx, 0.0],
        [-rx, -ky, -kx, -ry, 0.0, -ry],
        [kx, -ry, rx, -ky, rx, 0.0],
    ];
    for curve in curves {
        operations.push(Operation::new("c", curve.iter().map(|v| (*v).into()).collect()));
    }
    operations.push(Operation::new("h", vec![]));
}

/// Horizontal extent of the flag outline to the right of the stem's left edge, in points.
///
/// Found by sampling the outline's Bezier curves, because the curves peak slightly short of
/// their control points.
fn flag_right_extent() -> f32 {
    const SAMPLES: usize = 64;
    let mut start_x = 0.0f32;
    let mut max_u = 0.0f32;
    for curve in FLAG_OUTLINE {
        let (c1x, c2x, end_x) = (curve[0], curve[2], curve[4]);
        for step in 0..=SAMPLES {
            let t = step as f32 / SAMPLES as f32;
            let m = 1.0 - t;
            let u = m.powi(3) * start_x + 3.0 * m * m * t * c1x + 3.0 * m * t * t * c2x + t.powi(3) * end_x;
            max_u = max_u.max(u);
        }
        start_x = end_x;
    }
    max_u * FLAG_WIDTH_SPACES * STAFF_LINE_GAP
}

/// Center of the augmentation dot for a note at (x, note_y), `position` steps above the bottom staff line.
///
/// - The dot goes to the right of the notehead, clear of the stem.
/// - It never sits on a staff or ledger line. A note on a line gets its dot in the space above.
/// - On an up-stem note with flags the dot moves to the right of the flags to avoid them.
fn dot_position(
    x: f32,
    note_y: f32,
    position: i32,
    stem: Option<&StemGeometry>,
    flags: u8,
    half_width: f32,
) -> (f32, f32) {
    let radius = DOT_DIAMETER_SPACES * STAFF_LINE_GAP / 2.0;
    let gap = DOT_GAP_SPACES * STAFF_LINE_GAP;

    let mut left = x + half_width + gap;
    if let Some(stem) = stem {
        if stem.up && flags > 0 {
            left = left.max(stem.x - STEM_LINE_WIDTH / 2.0 + flag_right_extent() + gap);
        }
    }

    let on_a_line = position.rem_euclid(2) == 0;
    let y = if on_a_line { note_y + STAFF_STEP } else { note_y };
    (left + radius, y)
}

/// Push operations that draw a filled augmentation dot centered at (x, y).
fn push_dot(operations: &mut Vec<Operation>, x: f32, y: f32) {
    let radius = DOT_DIAMETER_SPACES * STAFF_LINE_GAP / 2.0;
    operations.push(Operation::new("q", vec![]));
    operations.push(Operation::new("cm", vec![1.0.into(), 0.0.into(), 0.0.into(), 1.0.into(), x.into(), y.into()]));
    push_ellipse_path(operations, radius, radius);
    operations.push(Operation::new("f", vec![]));
    operations.push(Operation::new("Q", vec![]));
}

/// Half the width of a notehead, in points: where a stem attaches, and how far ledger lines, accidentals
/// and dots keep away from the head's centre. A whole note's head is wider than the others.
fn notehead_half_width(duration: NoteDuration) -> f32 {
    match duration {
        NoteDuration::Whole => WHOLE_NOTEHEAD.width * STAFF_LINE_GAP / 2.0,
        _ => notehead_stem_anchor().0,
    }
}

/// Push operations that draw a whole note's head, centered at (x, y).
///
/// It is Bravura's whole notehead: wider and rounder than the other heads, a thick ring around a
/// tilted hole. The outline holds two contours, which the non-zero fill turns into the ring.
fn push_whole_notehead(operations: &mut Vec<Operation>, x: f32, y: f32) {
    let left = x - notehead_half_width(NoteDuration::Whole);
    push_glyph(operations, &WHOLE_NOTEHEAD, left, y);
}

/// Push operations that draw a filled, tilted oval notehead centered at (x, y).
///
/// This is the head of quarter notes and shorter.
fn push_notehead(operations: &mut Vec<Operation>, x: f32, y: f32) {
    let angle = NOTEHEAD_TILT_DEG.to_radians();
    let (sin, cos) = angle.sin_cos();

    // Isolate the transform so it doesn't affect later drawing
    operations.push(Operation::new("q", vec![]));
    operations.push(Operation::new(
        "cm",
        vec![cos.into(), sin.into(), (-sin).into(), cos.into(), x.into(), y.into()],
    ));
    push_ellipse_path(operations, NOTEHEAD_RX, NOTEHEAD_RY);
    operations.push(Operation::new("f", vec![]));
    operations.push(Operation::new("Q", vec![]));
}

/// Half the height of a filled notehead, in points. The tilt makes this a little more than `NOTEHEAD_RY`.
fn notehead_half_height() -> f32 {
    let (sin, cos) = NOTEHEAD_TILT_DEG.to_radians().sin_cos();
    (NOTEHEAD_RX.powi(2) * sin.powi(2) + NOTEHEAD_RY.powi(2) * cos.powi(2)).sqrt()
}

/// Push operations that draw a half note's hollow head, centered at (x, y).
///
/// It is Bravura's half notehead (a thick ring around a tilted hole, like the whole note's head)
/// squeezed to exactly the width and height of the filled head. Both heads then take the same
/// stem, dot and accidental positions.
fn push_half_notehead(operations: &mut Vec<Operation>, x: f32, y: f32) {
    let width = 2.0 * notehead_stem_anchor().0;
    let height = 2.0 * notehead_half_height();
    let scale_x = width / (HALF_NOTEHEAD.width * STAFF_LINE_GAP);
    let scale_y = height / STAFF_LINE_GAP; // the glyph is exactly one space tall
    push_glyph_scaled(operations, &HALF_NOTEHEAD, x - width / 2.0, y, scale_x, scale_y);
}

/// Push operations that draw a thin barline spanning the staff at `x`.
fn push_barline(operations: &mut Vec<Operation>, x: f32, staff_bottom_y: f32, staff_height: f32) {
    operations.push(Operation::new("q", vec![]));
    operations.push(Operation::new("w", vec![BARLINE_WIDTH.into()]));
    operations.push(Operation::new("m", vec![x.into(), staff_bottom_y.into()]));
    operations.push(Operation::new("l", vec![x.into(), (staff_bottom_y + staff_height).into()]));
    operations.push(Operation::new("S", vec![]));
    operations.push(Operation::new("Q", vec![]));
}

/// Staff positions (in steps above the bottom line) of the ledger lines a note needs.
///
/// Only lines between the staff and the note are needed. A note in the first space beyond
/// the staff needs none. Positions are even because lines sit on even steps.
fn ledger_line_positions(position: i32) -> Vec<i32> {
    if position < 0 {
        (position..=-2).filter(|p| p % 2 == 0).rev().collect()
    } else if position > TOP_LINE_STEPS {
        (TOP_LINE_STEPS + 2..=position).filter(|p| p % 2 == 0).collect()
    } else {
        Vec::new()
    }
}

/// Horizontal extent of a ledger line for a notehead centered at x: the notehead plus an extension each side.
fn ledger_line_extent(x: f32, half_width: f32) -> (f32, f32) {
    let reach = half_width + LEDGER_LINE_EXTENSION;
    (x - reach, x + reach)
}

/// Push operations that draw the ledger lines for a note at `position` steps above the bottom staff line.
fn push_ledger_lines(operations: &mut Vec<Operation>, x: f32, staff_bottom_y: f32, position: i32, half_width: f32) {
    let lines = ledger_line_positions(position);
    if lines.is_empty() {
        return;
    }
    let (x_start, x_end) = ledger_line_extent(x, half_width);

    operations.push(Operation::new("q", vec![]));
    operations.push(Operation::new("w", vec![LEDGER_LINE_WIDTH.into()]));
    for line in lines {
        let line_y = staff_bottom_y + line as f32 * STAFF_STEP;
        operations.push(Operation::new("m", vec![x_start.into(), line_y.into()]));
        operations.push(Operation::new("l", vec![x_end.into(), line_y.into()]));
        operations.push(Operation::new("S", vec![]));
    }
    operations.push(Operation::new("Q", vec![]));
}

/// Geometry of a single stem in page coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
struct StemGeometry {
    x: f32,
    start_y: f32,
    end_y: f32,
    up: bool,
}

/// Offset from the notehead center to the point where the tilted ellipse is widest
/// (rightmost for up-stems; the mirror image is used for the leftmost point of down-stems).
fn notehead_stem_anchor() -> (f32, f32) {
    let (sin, cos) = NOTEHEAD_TILT_DEG.to_radians().sin_cos();
    let half_width = (NOTEHEAD_RX.powi(2) * cos.powi(2) + NOTEHEAD_RY.powi(2) * sin.powi(2)).sqrt();
    let dy = (NOTEHEAD_RX.powi(2) - NOTEHEAD_RY.powi(2)) * sin * cos / half_width;
    (half_width, dy)
}

/// Compute where a stem goes for a notehead centered at (x, note_y).
///
/// - Notes on the middle line or higher are stemmed down, attached to the left of the head.
/// - Notes below the middle line are stemmed up, attached to the right of the head.
/// - Stems are 3.5 staff spaces long, measured from the notehead center, and are extended
///   to reach the middle line when the note is far enough outside the staff (ledger lines).
/// - A stem carrying more than two flags is lengthened by one flag spacing per extra flag.
fn stem_geometry(x: f32, note_y: f32, staff_middle: f32, flags: u8) -> StemGeometry {
    let up = note_y < staff_middle;
    let (anchor_dx, anchor_dy) = notehead_stem_anchor();
    // Pull the line inward by half its width so the stroke's outer edge meets the notehead edge
    let inset = STEM_LINE_WIDTH / 2.0;
    let extra_flags = flags.saturating_sub(FLAGS_ON_STANDARD_STEM) as f32;
    let length = (STEM_LENGTH_SPACES + extra_flags * FLAG_SPACING_SPACES) * STAFF_LINE_GAP;

    if up {
        StemGeometry {
            x: x + anchor_dx - inset,
            start_y: note_y + anchor_dy,
            end_y: (note_y + length).max(staff_middle),
            up,
        }
    } else {
        StemGeometry {
            x: x - anchor_dx + inset,
            start_y: note_y - anchor_dy,
            end_y: (note_y - length).min(staff_middle),
            up,
        }
    }
}

/// Lay out the stems and beams of every beamed group of a measure, in points.
///
/// Returns, for each note, its stem if the note is beamed, and all the beams of the measure.
/// The beaming module works in staff spaces measured from the bottom staff line, so positions are
/// scaled on the way in and out.
fn beamed_notes(
    measure: &Measure,
    spelled: &[SpelledNote],
    note_xs: &[f32],
    staff_bottom_y: f32,
    clef: Clef,
) -> (Vec<Option<StemGeometry>>, Vec<Beam>) {
    let mut stems: Vec<Option<StemGeometry>> = vec![None; measure.notes.len()];
    let mut beams: Vec<Beam> = Vec::new();

    let onsets = note_onsets(&measure.notes);
    let (half_width, anchor_dy) = notehead_stem_anchor();
    let metrics = BeamMetrics {
        notehead_half_width: half_width / STAFF_LINE_GAP,
        stem_anchor_dy: anchor_dy / STAFF_LINE_GAP,
        stem_width: STEM_LINE_WIDTH / STAFF_LINE_GAP,
    };

    for group in beam_groups(&measure.notes, &measure.time_signature) {
        let notes: Vec<BeamNote> = (group.start..group.end)
            .map(|i| {
                let position = staff_position(&spelled[i].pitch, clef);
                BeamNote {
                    x: note_xs[i] / STAFF_LINE_GAP,
                    y: position as f32 / 2.0,
                    position,
                    beams: flag_count(measure.notes[i].duration),
                    onset: onsets[i],
                }
            })
            .collect();
        let layout = layout_group(&notes, &metrics);

        for (offset, stem) in layout.stems.iter().enumerate() {
            stems[group.start + offset] = Some(stem_in_points(stem, layout.up, staff_bottom_y));
        }
        beams.extend(layout.beams);
    }
    (stems, beams)
}

/// Convert a stem from staff spaces above the bottom line to points on the page
fn stem_in_points(stem: &BeamedStem, up: bool, staff_bottom_y: f32) -> StemGeometry {
    StemGeometry {
        x: stem.x * STAFF_LINE_GAP,
        start_y: staff_bottom_y + stem.start_y * STAFF_LINE_GAP,
        end_y: staff_bottom_y + stem.end_y * STAFF_LINE_GAP,
        up,
    }
}

/// Push operations that fill a beam, given in staff spaces above the bottom line.
///
/// A beam is a parallelogram whose vertical edges are `BEAM_THICKNESS` apart, so it is the same
/// thickness all along its length however steeply it slants.
fn push_beam(operations: &mut Vec<Operation>, beam: &Beam, staff_bottom_y: f32) {
    let to_x = |x: f32| x * STAFF_LINE_GAP;
    let to_y = |y: f32| staff_bottom_y + y * STAFF_LINE_GAP;
    let thickness = beam.toward_heads * BEAM_THICKNESS;

    operations.push(Operation::new("q", vec![]));
    operations.push(Operation::new("m", vec![to_x(beam.x1).into(), to_y(beam.y1).into()]));
    operations.push(Operation::new("l", vec![to_x(beam.x2).into(), to_y(beam.y2).into()]));
    operations.push(Operation::new("l", vec![to_x(beam.x2).into(), to_y(beam.y2 + thickness).into()]));
    operations.push(Operation::new("l", vec![to_x(beam.x1).into(), to_y(beam.y1 + thickness).into()]));
    operations.push(Operation::new("h", vec![]));
    operations.push(Operation::new("f", vec![]));
    operations.push(Operation::new("Q", vec![]));
}

/// Push operations that draw a stem.
fn push_stem(operations: &mut Vec<Operation>, stem: &StemGeometry) {
    operations.push(Operation::new("q", vec![]));
    operations.push(Operation::new("w", vec![STEM_LINE_WIDTH.into()]));
    operations.push(Operation::new("m", vec![stem.x.into(), stem.start_y.into()]));
    operations.push(Operation::new("l", vec![stem.x.into(), stem.end_y.into()]));
    operations.push(Operation::new("S", vec![]));
    operations.push(Operation::new("Q", vec![]));
}

/// Push operations that draw `count` flags hanging from the free end of `stem`.
///
/// Flags are always on the right of the stem, flush with its left edge, and point toward the
/// notehead. Each further flag starts one flag spacing closer to the notehead, ends slightly
/// further from the stem end than the one before, and is therefore a little shorter.
fn push_flags(operations: &mut Vec<Operation>, stem: &StemGeometry, count: u8) {
    // Positive y direction away from the stem end, toward the notehead
    let toward_head = if stem.up { -1.0 } else { 1.0 };
    let left = stem.x - STEM_LINE_WIDTH / 2.0;
    let width = FLAG_WIDTH_SPACES * STAFF_LINE_GAP;

    for i in 0..count {
        let offset = i as f32 * FLAG_SPACING_SPACES;
        let length = FLAG_TAIL_LENGTH_SPACES + i as f32 * (FLAG_EXTRA_DEPTH_SPACES - FLAG_SPACING_SPACES);
        let point = |u: f32, v: f32| -> (Object, Object) {
            let x = left + u * width;
            let y = stem.end_y + toward_head * (offset + v * length) * STAFF_LINE_GAP;
            (x.into(), y.into())
        };

        operations.push(Operation::new("q", vec![]));
        let (start_x, start_y) = point(0.0, 0.0);
        operations.push(Operation::new("m", vec![start_x, start_y]));
        for curve in FLAG_OUTLINE {
            let (c1x, c1y) = point(curve[0], curve[1]);
            let (c2x, c2y) = point(curve[2], curve[3]);
            let (ex, ey) = point(curve[4], curve[5]);
            operations.push(Operation::new("c", vec![c1x, c1y, c2x, c2y, ex, ey]));
        }
        operations.push(Operation::new("h", vec![]));
        operations.push(Operation::new("f", vec![]));
        operations.push(Operation::new("Q", vec![]));
    }
}

impl crate::presentation::NotationRenderer for PdfNotationRenderer {
    fn render_measure(&self, _measure: &Measure) -> String {
        String::new()
    }

    fn render_note(&self, _note: &Note, _key_sig: &KeySignature) -> String {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdf_renderer_creation() {
        let renderer = PdfNotationRenderer::new();
        assert_eq!(renderer.staff_lines, 5);
        assert!(renderer.show_clef);
        assert!(renderer.show_key_signature);
        assert!(renderer.show_time_signature);
    }

    /// Staff position of a MIDI note when written in a key (C major unless stated)
    fn position_in(midi: u8, key_root: u8) -> i32 {
        staff_position(&crate::presentation::spelling::spell_pitch(midi, &key(key_root), false), Clef::Treble)
    }

    #[test]
    fn test_staff_position_treble() {
        assert_eq!(position_in(64, 60), 0); // E4 bottom line
        assert_eq!(position_in(65, 60), 1); // F4 first space
        assert_eq!(position_in(71, 60), 4); // B4 middle line
        assert_eq!(position_in(77, 60), 8); // F5 top line
        assert_eq!(position_in(60, 60), -2); // C4 first ledger line below
        assert_eq!(position_in(66, G_MAJOR), 1); // F#4 in G major is written as F4, on the first space
    }

    #[test]
    fn test_flat_keys_put_notes_on_the_right_line() {
        // This was wrong before spelling: Bb4 was drawn on the A line
        assert_eq!(position_in(70, B_FLAT_MAJOR), 4); // Bb4 on the B line (middle line)
        assert_eq!(position_in(69, B_FLAT_MAJOR), 3); // A4 stays on the A space
        assert_eq!(position_in(63, B_FLAT_MAJOR), 0); // Eb4 on the bottom line (E)
        assert_eq!(position_in(75, B_FLAT_MAJOR), 7); // Eb5 in the fourth space (E)
    }

    #[test]
    fn test_enharmonic_pitches_sit_on_different_lines() {
        // The same sounding note is written on different lines depending on its spelling:
        // C#5 (sharp key) is on the C space, Db5 (flat key) is on the D line
        let c_sharp = position_in(73, A_MAJOR);
        let d_flat = position_in(73, B_FLAT_MAJOR);
        assert_eq!(d_flat, c_sharp + 1);
    }

    #[test]
    fn test_accidental_glyph_per_sign() {
        assert_eq!(accidental_glyph(AccidentalSign::Sharp).width, SHARP.width);
        assert_eq!(accidental_glyph(AccidentalSign::Flat).width, FLAT.width);
        assert_eq!(accidental_glyph(AccidentalSign::Natural).width, NATURAL.width);
    }

    #[test]
    fn test_accidental_sits_left_of_the_notehead_with_a_gap() {
        let (half_width, _) = notehead_stem_anchor();
        for sign in [AccidentalSign::Sharp, AccidentalSign::Flat, AccidentalSign::Natural] {
            let glyph = accidental_glyph(sign);
            let origin = accidental_origin_x(100.0, false, &glyph, notehead_stem_anchor().0);
            let right_edge = origin + glyph.width * STAFF_LINE_GAP;
            let notehead_left = 100.0 - half_width;
            assert!(right_edge < notehead_left, "{sign:?} overlaps the notehead");
            assert!(((notehead_left - right_edge) - ACCIDENTAL_GAP_SPACES * STAFF_LINE_GAP).abs() < 1e-4);
        }
    }

    #[test]
    fn test_accidental_clears_ledger_lines() {
        let glyph = SHARP;
        let plain = accidental_origin_x(100.0, false, &glyph, notehead_stem_anchor().0);
        let with_ledger = accidental_origin_x(100.0, true, &glyph, notehead_stem_anchor().0);
        assert!((plain - with_ledger - LEDGER_LINE_EXTENSION).abs() < 1e-4);
        // The sign's right edge must not reach the left end of the ledger line
        let (ledger_start, _) = ledger_line_extent(100.0, notehead_stem_anchor().0);
        assert!(with_ledger + glyph.width * STAFF_LINE_GAP < ledger_start);
    }

    #[test]
    fn test_push_accidental_is_vertically_centered_on_the_note() {
        let mut ops = vec![];
        push_accidental(&mut ops, AccidentalSign::Flat, 100.0, 77.5, false, notehead_stem_anchor().0);
        let cm = ops.iter().find(|op| op.operator == "cm").unwrap();
        let values: Vec<f32> = cm.operands.iter().map(|o| o.as_float().unwrap()).collect();
        assert_eq!(values[5], 77.5);
        assert_eq!(count_ops(&ops, "f"), 1);
    }

    #[test]
    fn test_chromatic_note_is_drawn_with_a_sign() {
        use crate::domain::NoteDuration::Quarter;
        // C major: C# gets one extra filled shape, the sharp, on top of the notehead
        let notes = |midis: &[u8]| {
            let mut m = Measure::new(1, TimeSignature::common_time(), key(60));
            for &midi in midis {
                m.notes.push(Note::new(midi, Quarter));
            }
            m
        };
        let fills = |measure: Measure| {
            let renderer = PdfNotationRenderer::new().without_clef().without_time_signature();
            count_ops(&renderer.render_pages(&[measure]).unwrap()[0], "f")
        };
        assert_eq!(fills(notes(&[60])), 1); // notehead
        assert_eq!(fills(notes(&[61])), 2); // notehead + sharp
        assert_eq!(fills(notes(&[61, 61])), 3); // second C# needs no sign again
        assert_eq!(fills(notes(&[61, 60])), 4); // C#: sharp + head, then C: natural sign + head
    }

    #[test]
    fn test_notes_in_the_key_draw_no_signs() {
        use crate::domain::NoteDuration::Quarter;
        // Bb major: Bb is in the key signature, so no flat sign in front of it
        let mut m = Measure::new(1, TimeSignature::common_time(), key(B_FLAT_MAJOR));
        m.notes.push(Note::new(70, Quarter));
        let renderer = PdfNotationRenderer::new().without_clef().without_key_signature().without_time_signature();
        assert_eq!(count_ops(&renderer.render_pages(&[m]).unwrap()[0], "f"), 1);
    }

    #[test]
    fn test_text_matrix_puts_position_last() {
        let mut ops = vec![];
        push_text(&mut ops, "x", 12.0, 150.0, 680.0);
        let tm = ops.iter().find(|op| op.operator == "Tm").unwrap();
        let values: Vec<f32> = tm.operands.iter().map(|o| o.as_float().unwrap()).collect();
        assert_eq!(values, vec![1.0, 0.0, 0.0, 1.0, 150.0, 680.0]);
    }

    fn measures_in(count: usize, key_root: u8, time: TimeSignature) -> Vec<Measure> {
        use crate::domain::KeyMode;
        (0..count)
            .map(|i| Measure::new(i as u32 + 1, time, KeySignature::new(key_root, KeyMode::Major)))
            .collect()
    }

    fn empty_measures(count: usize) -> Vec<Measure> {
        measures_in(count, 60, TimeSignature::common_time())
    }

    fn count_ops(page: &[Operation], operator: &str) -> usize {
        page.iter().filter(|op| op.operator == operator).count()
    }

    /// Measures per row for a renderer, read from the layout of a long piece
    fn measures_per_row(renderer: &PdfNotationRenderer) -> usize {
        renderer.layout(&empty_measures(100))[0].system_len
    }

    #[test]
    fn test_layout_of_nothing_is_empty() {
        assert!(PdfNotationRenderer::new().layout(&[]).is_empty());
    }

    #[test]
    fn test_layout_fills_rows_then_moves_down() {
        let renderer = PdfNotationRenderer::new();
        let slots = renderer.layout(&empty_measures(5));
        assert_eq!(measures_per_row(&renderer), 2); // about 450pt of measures / 200pt preferred

        // Same row: same y, increasing x
        assert_eq!(slots[0].staff_bottom_y, slots[1].staff_bottom_y);
        assert!(slots[1].x > slots[0].x);
        // Next row: lower on the page (smaller y), starting right after its header
        assert!(slots[2].staff_bottom_y < slots[0].staff_bottom_y);
        assert_eq!(slots[2].x, PAGE_MARGIN_X + slots[2].header.width);
        assert_eq!(slots[2].index_in_system, 0);
        // Last row only has one measure
        assert_eq!(slots[4].system_len, 1);
        assert_eq!(slots[0].system_len, 2);
    }

    #[test]
    fn test_layout_stays_inside_margins() {
        let renderer = PdfNotationRenderer::new();
        let top = PAGE_HEIGHT - PAGE_MARGIN_TOP;
        for slot in renderer.layout(&empty_measures(200)) {
            assert!(slot.x >= PAGE_MARGIN_X - 1e-3);
            assert!(slot.x + slot.width <= PAGE_WIDTH - PAGE_MARGIN_X + 1e-3);
            assert!(slot.staff_bottom_y >= PAGE_MARGIN_BOTTOM - 1e-3);
            assert!(slot.staff_bottom_y + renderer.staff_height() <= top + 1e-3);
        }
    }

    #[test]
    fn test_layout_starts_new_page_when_full() {
        let renderer = PdfNotationRenderer::new();
        let per_page = measures_per_row(&renderer) * renderer.systems_per_page();
        let slots = renderer.layout(&empty_measures(per_page + 1));
        assert_eq!(slots[per_page - 1].page, 0);
        assert_eq!(slots[per_page].page, 1);
        assert_eq!(slots[per_page].staff_bottom_y, renderer.first_staff_bottom_y());
        assert_eq!(slots[per_page].x, PAGE_MARGIN_X + slots[per_page].header.width);
    }

    #[test]
    fn test_measure_wider_than_page_still_gets_a_row_each() {
        let renderer = PdfNotationRenderer::new().with_measure_width(10_000.0);
        let slots = renderer.layout(&empty_measures(3));
        assert!(slots.iter().all(|slot| slot.index_in_system == 0));
        for slot in &slots {
            assert!((slot.header.width + slot.width - PdfNotationRenderer::usable_width()).abs() < 1e-3);
        }
    }

    #[test]
    fn test_every_measure_is_drawn() {
        // Regression test: each measure used to replace the page's content, so only the last survived
        let renderer = PdfNotationRenderer::new();
        let pages = renderer.render_pages(&empty_measures(5)).unwrap();
        assert_eq!(pages.len(), 1);
        // 5 measures in 3 systems: 3 * 5 staff lines + 5 barlines
        assert_eq!(count_ops(&pages[0], "S"), 3 * 5 + 5);
    }

    #[test]
    fn test_render_pages_splits_across_pages() {
        let renderer = PdfNotationRenderer::new();
        let per_page = measures_per_row(&renderer) * renderer.systems_per_page();
        let pages = renderer.render_pages(&empty_measures(per_page + 1)).unwrap();
        assert_eq!(pages.len(), 2);
        assert_eq!(count_ops(&pages[1], "S"), 5 + 1); // one system, one measure
    }

    #[test]
    fn test_render_pages_with_no_measures_gives_one_blank_page() {
        let pages = PdfNotationRenderer::new().render_pages(&[]).unwrap();
        assert_eq!(pages.len(), 1);
        assert!(pages[0].is_empty());
    }

    const G_MAJOR: u8 = 67; // 1 sharp
    const A_MAJOR: u8 = 69; // 3 sharps
    const B_FLAT_MAJOR: u8 = 58; // 2 flats
    const C_SHARP_MAJOR: u8 = 61; // 7 sharps
    const C_FLAT_MAJOR: u8 = 47; // 7 flats

    fn key(root: u8) -> KeySignature {
        KeySignature::new(root, crate::domain::KeyMode::Major)
    }

    #[test]
    fn test_key_signature_sharp_order_and_positions() {
        let glyphs = key_signature_glyphs(&key(C_SHARP_MAJOR), Clef::Treble);
        let positions: Vec<i32> = glyphs.iter().map(|g| g.2).collect();
        // F C G D A E B in treble clef
        assert_eq!(positions, vec![8, 5, 9, 6, 3, 7, 4]);
        assert_eq!(positions[0], TOP_LINE_STEPS, "the first sharp (F) is on the top line");
    }

    #[test]
    fn test_key_signature_flat_order_and_positions() {
        let glyphs = key_signature_glyphs(&key(C_FLAT_MAJOR), Clef::Treble);
        let positions: Vec<i32> = glyphs.iter().map(|g| g.2).collect();
        // B E A D G C F in treble clef
        assert_eq!(positions, vec![4, 7, 3, 6, 2, 5, 1]);
        assert_eq!(positions[0], MIDDLE_LINE_STEPS as i32, "the first flat (B) is on the middle line");
    }

    #[test]
    fn test_key_signature_needs_no_ledger_lines() {
        for root in [C_SHARP_MAJOR, C_FLAT_MAJOR] {
            for (_, _, position) in key_signature_glyphs(&key(root), Clef::Treble) {
                assert!(ledger_line_positions(position).is_empty(), "position {position} needs a ledger line");
            }
        }
    }

    #[test]
    fn test_key_signature_glyph_count_and_kind() {
        assert!(key_signature_glyphs(&key(60), Clef::Treble).is_empty()); // C major
        let sharps = key_signature_glyphs(&key(A_MAJOR), Clef::Treble);
        assert_eq!(sharps.len(), 3);
        assert!(sharps.iter().all(|g| g.0.width == SHARP.width));
        let flats = key_signature_glyphs(&key(B_FLAT_MAJOR), Clef::Treble);
        assert_eq!(flats.len(), 2);
        assert!(flats.iter().all(|g| g.0.width == FLAT.width));
    }

    #[test]
    fn test_key_signature_accidentals_do_not_overlap() {
        let glyphs = key_signature_glyphs(&key(C_SHARP_MAJOR), Clef::Treble);
        for pair in glyphs.windows(2) {
            assert!(pair[1].1 >= pair[0].1 + pair[0].0.width, "accidentals overlap");
        }
        let width = key_signature_width(&key(C_SHARP_MAJOR), Clef::Treble);
        let last = glyphs.last().unwrap();
        assert!((width - (last.1 + last.0.width)).abs() < 1e-5);
        assert_eq!(key_signature_width(&key(60), Clef::Treble), 0.0);
    }

    #[test]
    fn test_time_signature_digits() {
        let digits = number_glyphs(12);
        assert_eq!(digits.len(), 2);
        assert_eq!(digits[0].width, TIME_SIG_DIGITS[1].width);
        assert_eq!(digits[1].width, TIME_SIG_DIGITS[2].width);
        assert_eq!(number_glyphs(4).len(), 1);
    }

    #[test]
    fn test_time_signature_width_is_the_wider_number() {
        let wide_top = TimeSignature::new(12, 8);
        let expected = run_width(&number_glyphs(12));
        assert!((time_signature_width(&wide_top) - expected).abs() < 1e-5);
        let wide_bottom = TimeSignature::new(3, 16);
        assert!((time_signature_width(&wide_bottom) - run_width(&number_glyphs(16))).abs() < 1e-5);
    }

    #[test]
    fn test_header_with_nothing_shown_has_no_width() {
        let renderer = PdfNotationRenderer::new()
            .without_clef()
            .without_key_signature()
            .without_time_signature();
        let header = renderer.header_layout(&key(A_MAJOR), Some(&TimeSignature::common_time()));
        assert_eq!(header, HeaderLayout::default());
    }

    #[test]
    fn test_header_orders_clef_key_time_with_gaps() {
        let renderer = PdfNotationRenderer::new();
        let time = TimeSignature::new(3, 4);
        let header = renderer.header_layout(&key(A_MAJOR), Some(&time));
        let (clef, keysig, time_x) = (header.clef_x.unwrap(), header.key_x.unwrap(), header.time_x.unwrap());
        assert!(clef < keysig && keysig < time_x);

        let space = STAFF_LINE_GAP;
        let clef_to_key = (keysig - (clef + TREBLE_CLEF.width * space)) / space;
        let key_to_time = (time_x - (keysig + key_signature_width(&key(A_MAJOR), Clef::Treble) * space)) / space;
        assert!((clef_to_key - SYMBOL_GAP_SPACES).abs() < 1e-4);
        assert!((key_to_time - SYMBOL_GAP_SPACES).abs() < 1e-4);
        assert!((clef - HEADER_LEFT_PAD_SPACES * space).abs() < 1e-4);
        let end = time_x + time_signature_width(&time) * space;
        assert!((header.width - end).abs() < 1e-3);
    }

    #[test]
    fn test_header_skips_key_signature_in_c_major_and_time_when_not_given() {
        let renderer = PdfNotationRenderer::new();
        let header = renderer.header_layout(&key(60), None);
        assert!(header.clef_x.is_some());
        assert!(header.key_x.is_none());
        assert!(header.time_x.is_none());
        assert!((header.width - (HEADER_LEFT_PAD_SPACES + TREBLE_CLEF.width) * STAFF_LINE_GAP).abs() < 1e-4);
    }

    #[test]
    fn test_time_signature_only_on_first_row_clef_and_key_on_every_row() {
        let renderer = PdfNotationRenderer::new();
        let slots = renderer.layout(&measures_in(7, A_MAJOR, TimeSignature::common_time()));
        for slot in &slots {
            assert!(slot.header.clef_x.is_some());
            assert!(slot.header.key_x.is_some());
            assert_eq!(slot.header.time_x.is_some(), slot.system_index == 0);
        }
        assert!(slots.iter().any(|slot| slot.system_index > 0));
    }

    #[test]
    fn test_first_measure_starts_after_the_header() {
        let renderer = PdfNotationRenderer::new();
        let slots = renderer.layout(&measures_in(4, A_MAJOR, TimeSignature::common_time()));
        for slot in slots.iter().filter(|slot| slot.index_in_system == 0) {
            assert!((slot.x - (PAGE_MARGIN_X + slot.header.width)).abs() < 1e-3);
        }
        // The first row carries the time signature, so its header is the widest
        assert!(slots[0].header.width > slots[2].header.width);
    }

    #[test]
    fn test_rows_span_the_full_width() {
        let renderer = PdfNotationRenderer::new();
        for slot in renderer.layout(&measures_in(9, B_FLAT_MAJOR, TimeSignature::new(6, 8))) {
            let row_end = slot.x + slot.width * (slot.system_len - slot.index_in_system) as f32;
            let full_row = slot.system_len == measures_per_row(&renderer);
            if full_row {
                assert!((row_end - (PAGE_WIDTH - PAGE_MARGIN_X)).abs() < 1e-2, "row ends at {row_end}");
            }
        }
    }

    #[test]
    fn test_header_fits_with_the_widest_signatures() {
        let renderer = PdfNotationRenderer::new();
        let slots = renderer.layout(&measures_in(3, C_SHARP_MAJOR, TimeSignature::new(12, 16)));
        for slot in slots {
            assert!(slot.width > 0.0);
            assert!(slot.header.width < PdfNotationRenderer::usable_width() / 2.0);
        }
    }

    #[test]
    fn test_header_glyphs_are_drawn_once_per_row() {
        let renderer = PdfNotationRenderer::new();
        // 5 measures, 2 per row: rows have 2, 2 and 1 measures
        let pages = renderer
            .render_pages(&measures_in(5, A_MAJOR, TimeSignature::common_time()))
            .unwrap();
        // First row: clef + 3 sharps + 2 digits. Later rows: clef + 3 sharps.
        assert_eq!(count_ops(&pages[0], "f"), (1 + 3 + 2) + (1 + 3) + (1 + 3));
    }

    #[test]
    fn test_header_flags_turn_symbols_off() {
        let measures = measures_in(1, A_MAJOR, TimeSignature::common_time());
        let fills = |renderer: PdfNotationRenderer| count_ops(&renderer.render_pages(&measures).unwrap()[0], "f");
        assert_eq!(fills(PdfNotationRenderer::new()), 1 + 3 + 2);
        assert_eq!(fills(PdfNotationRenderer::new().without_clef()), 3 + 2);
        assert_eq!(fills(PdfNotationRenderer::new().without_key_signature()), 1 + 2);
        assert_eq!(fills(PdfNotationRenderer::new().without_time_signature()), 1 + 3);
    }

    #[test]
    fn test_push_glyph_scales_staff_spaces_to_points() {
        let mut ops = vec![];
        push_glyph(&mut ops, &SHARP, 70.0, 80.0);
        let cm = ops.iter().find(|op| op.operator == "cm").unwrap();
        let values: Vec<f32> = cm.operands.iter().map(|o| o.as_float().unwrap()).collect();
        assert_eq!(values, vec![STAFF_LINE_GAP, 0.0, 0.0, STAFF_LINE_GAP, 70.0, 80.0]);
        assert_eq!(count_ops(&ops, "f"), 1);
        assert_eq!(ops.first().unwrap().operator, "q");
        assert_eq!(ops.last().unwrap().operator, "Q");
        assert_eq!(count_ops(&ops, "h"), SHARP.ops.iter().filter(|op| **op == PathOp::Close).count());
    }

    #[test]
    fn test_treble_clef_wraps_the_g_line() {
        // The clef's origin is the G line: the curl sits above it and the tail below the staff
        let ys: Vec<f32> = TREBLE_CLEF
            .ops
            .iter()
            .flat_map(|op| match *op {
                PathOp::Move(_, y) | PathOp::Line(_, y) => vec![y],
                PathOp::Curve(_, c1y, _, c2y, _, y) => vec![c1y, c2y, y],
                PathOp::Close => vec![],
            })
            .collect();
        let top = ys.iter().cloned().fold(f32::MIN, f32::max);
        let bottom = ys.iter().cloned().fold(f32::MAX, f32::min);
        assert!(top > 2.0, "the clef should reach above the staff");
        assert!(bottom < -1.0, "the clef should reach below the staff");
    }

    #[test]
    fn test_ledger_lines_below_staff() {
        assert_eq!(ledger_line_positions(0), Vec::<i32>::new()); // bottom line
        assert_eq!(ledger_line_positions(-1), Vec::<i32>::new()); // first space below: no line
        assert_eq!(ledger_line_positions(-2), vec![-2]); // C4
        assert_eq!(ledger_line_positions(-3), vec![-2]); // space under first ledger line
        assert_eq!(ledger_line_positions(-4), vec![-2, -4]); // A3
        assert_eq!(ledger_line_positions(-5), vec![-2, -4]);
    }

    #[test]
    fn test_ledger_lines_above_staff() {
        assert_eq!(ledger_line_positions(8), Vec::<i32>::new()); // top line
        assert_eq!(ledger_line_positions(9), Vec::<i32>::new()); // first space above: no line
        assert_eq!(ledger_line_positions(10), vec![10]); // A5
        assert_eq!(ledger_line_positions(11), vec![10]);
        assert_eq!(ledger_line_positions(12), vec![10, 12]); // C6
    }

    #[test]
    fn test_no_ledger_lines_inside_staff() {
        for position in 0..=TOP_LINE_STEPS {
            assert!(ledger_line_positions(position).is_empty());
        }
    }

    #[test]
    fn test_ledger_line_is_just_over_two_spaces_long() {
        let (start, end) = ledger_line_extent(100.0, notehead_stem_anchor().0);
        let spaces = (end - start) / STAFF_LINE_GAP;
        assert!(spaces > 2.0 && spaces < 2.2, "length was {spaces} spaces");
        assert!(((start + end) / 2.0 - 100.0).abs() < 1e-4, "must be centered on the notehead");
    }

    #[test]
    fn test_ledger_lines_thicker_than_staff_lines() {
        assert!(LEDGER_LINE_WIDTH > STAFF_LINE_WIDTH);
    }

    #[test]
    fn test_push_ledger_lines_draws_one_stroke_per_line() {
        let mut ops = vec![];
        push_ledger_lines(&mut ops, 50.0, 650.0, -4, notehead_stem_anchor().0);
        assert_eq!(ops.iter().filter(|op| op.operator == "S").count(), 2);
        let mut none = vec![];
        push_ledger_lines(&mut none, 50.0, 650.0, 4, notehead_stem_anchor().0);
        assert!(none.is_empty());
    }

    const MIDDLE: f32 = 100.0;

    #[test]
    fn test_stem_direction_follows_middle_line() {
        assert!(stem_geometry(50.0, MIDDLE - STAFF_STEP, MIDDLE, 0).up);
        assert!(!stem_geometry(50.0, MIDDLE, MIDDLE, 0).up);
        assert!(!stem_geometry(50.0, MIDDLE + STAFF_STEP, MIDDLE, 0).up);
    }

    #[test]
    fn test_stem_attaches_to_correct_side() {
        let up = stem_geometry(50.0, MIDDLE - 10.0, MIDDLE, 0);
        let down = stem_geometry(50.0, MIDDLE + 10.0, MIDDLE, 0);
        assert!(up.x > 50.0, "up-stem belongs on the right of the notehead");
        assert!(down.x < 50.0, "down-stem belongs on the left of the notehead");
    }

    #[test]
    fn test_stem_standard_length_is_one_octave() {
        let up = stem_geometry(50.0, MIDDLE - 5.0, MIDDLE, 0);
        let down = stem_geometry(50.0, MIDDLE + 5.0, MIDDLE, 0);
        let length = STEM_LENGTH_SPACES * STAFF_LINE_GAP;
        assert!((up.end_y - (MIDDLE - 5.0 + length)).abs() < 1e-4);
        assert!((down.end_y - (MIDDLE + 5.0 - length)).abs() < 1e-4);
    }

    #[test]
    fn test_ledger_line_stems_reach_middle_line() {
        // Far below the staff: a standard stem would end below the middle line
        let up = stem_geometry(50.0, MIDDLE - 30.0, MIDDLE, 0);
        assert_eq!(up.end_y, MIDDLE);
        // Far above the staff
        let down = stem_geometry(50.0, MIDDLE + 30.0, MIDDLE, 0);
        assert_eq!(down.end_y, MIDDLE);
    }

    /// All (x, y) points in the path operators of `ops`
    fn path_points(ops: &[Operation]) -> Vec<(f32, f32)> {
        ops.iter()
            .filter(|op| op.operator == "m" || op.operator == "c")
            .flat_map(|op| {
                let values: Vec<f32> = op.operands.iter().map(|o| o.as_float().unwrap()).collect();
                values.chunks(2).map(|pair| (pair[0], pair[1])).collect::<Vec<_>>()
            })
            .collect()
    }

    #[test]
    fn test_stem_only_lengthened_beyond_two_flags() {
        let length = |flags| {
            let stem = stem_geometry(50.0, MIDDLE - 5.0, MIDDLE, flags);
            stem.end_y - (MIDDLE - 5.0)
        };
        assert_eq!(length(0), length(1));
        assert_eq!(length(1), length(2));
        let step = FLAG_SPACING_SPACES * STAFF_LINE_GAP;
        assert!((length(3) - length(2) - step).abs() < 1e-4);
        assert!((length(4) - length(2) - 2.0 * step).abs() < 1e-4);
    }

    #[test]
    fn test_flags_draw_one_filled_shape_each() {
        for count in 0..=4u8 {
            let stem = stem_geometry(50.0, MIDDLE - 10.0, MIDDLE, count);
            let mut ops = vec![];
            push_flags(&mut ops, &stem, count);
            assert_eq!(count_ops(&ops, "f"), count as usize);
            assert_eq!(count_ops(&ops, "q"), count_ops(&ops, "Q"));
        }
    }

    #[test]
    fn test_flags_are_on_the_right_of_the_stem() {
        for note_y in [MIDDLE - 10.0, MIDDLE + 10.0] {
            let stem = stem_geometry(50.0, note_y, MIDDLE, 4);
            let mut ops = vec![];
            push_flags(&mut ops, &stem, 4);
            let stem_left = stem.x - STEM_LINE_WIDTH / 2.0;
            for (x, _) in path_points(&ops) {
                assert!(x >= stem_left - 1e-4, "flag point at x={x} is left of the stem");
            }
        }
    }

    #[test]
    fn test_flags_hang_from_stem_end_toward_the_notehead() {
        let up = stem_geometry(50.0, MIDDLE - 10.0, MIDDLE, 3);
        let mut ops = vec![];
        push_flags(&mut ops, &up, 3);
        assert!(path_points(&ops).iter().all(|&(_, y)| y <= up.end_y + 1e-4));

        let down = stem_geometry(50.0, MIDDLE + 10.0, MIDDLE, 3);
        let mut ops = vec![];
        push_flags(&mut ops, &down, 3);
        assert!(path_points(&ops).iter().all(|&(_, y)| y >= down.end_y - 1e-4));
    }

    #[test]
    fn test_flags_do_not_reach_past_the_notehead_center() {
        for count in 1..=4u8 {
            let note_y = MIDDLE - 10.0;
            let up = stem_geometry(50.0, note_y, MIDDLE, count);
            let mut ops = vec![];
            push_flags(&mut ops, &up, count);
            let lowest = path_points(&ops).iter().map(|p| p.1).fold(f32::MAX, f32::min);
            assert!(lowest >= note_y, "{count} flags reach below the notehead center");

            let note_y = MIDDLE + 10.0;
            let down = stem_geometry(50.0, note_y, MIDDLE, count);
            let mut ops = vec![];
            push_flags(&mut ops, &down, count);
            let highest = path_points(&ops).iter().map(|p| p.1).fold(f32::MIN, f32::max);
            assert!(highest <= note_y, "{count} flags reach above the notehead center");
        }
    }

    /// Largest x reached by the Bezier curves in `ops`, found by sampling the curves
    fn max_curve_x(ops: &[Operation]) -> f32 {
        let mut current = (0.0f32, 0.0f32);
        let mut max_x = f32::MIN;
        for op in ops {
            let v: Vec<f32> = op.operands.iter().map(|o| o.as_float().unwrap()).collect();
            match op.operator.as_str() {
                "m" => current = (v[0], v[1]),
                "c" => {
                    for step in 0..=64 {
                        let t = step as f32 / 64.0;
                        let m = 1.0 - t;
                        let x = m.powi(3) * current.0
                            + 3.0 * m * m * t * v[0]
                            + 3.0 * m * t * t * v[2]
                            + t.powi(3) * v[4];
                        max_x = max_x.max(x);
                    }
                    current = (v[4], v[5]);
                }
                _ => {}
            }
        }
        max_x
    }

    #[test]
    fn test_dot_is_never_on_a_line() {
        let y = 100.0;
        // Even positions are lines (including ledger lines), odd positions are spaces
        for position in [-4, -2, 0, 2, 4, 6, 8, 10, 12] {
            let (_, dot_y) = dot_position(50.0, y, position, None, 0, notehead_stem_anchor().0);
            assert_eq!(dot_y, y + STAFF_STEP, "position {position} is a line, so the dot goes up a space");
        }
        for position in [-5, -3, -1, 1, 3, 5, 7, 9, 11] {
            let (_, dot_y) = dot_position(50.0, y, position, None, 0, notehead_stem_anchor().0);
            assert_eq!(dot_y, y, "position {position} is a space, so the dot stays level");
        }
    }

    #[test]
    fn test_dot_sits_right_of_notehead_with_a_gap() {
        let (half_width, _) = notehead_stem_anchor();
        let radius = DOT_DIAMETER_SPACES * STAFF_LINE_GAP / 2.0;
        let (dot_x, _) = dot_position(50.0, 100.0, 1, None, 0, notehead_stem_anchor().0);
        let gap = (dot_x - radius) - (50.0 + half_width);
        assert!((gap - DOT_GAP_SPACES * STAFF_LINE_GAP).abs() < 1e-4);
    }

    #[test]
    fn test_dot_clears_the_stem() {
        let radius = DOT_DIAMETER_SPACES * STAFF_LINE_GAP / 2.0;
        for note_y in [MIDDLE - 10.0, MIDDLE + 10.0] {
            let stem = stem_geometry(50.0, note_y, MIDDLE, 0);
            let (dot_x, _) = dot_position(50.0, note_y, 1, Some(&stem), 0, notehead_stem_anchor().0);
            let stem_right = stem.x + STEM_LINE_WIDTH / 2.0;
            assert!(dot_x - radius > stem_right, "dot touches the stem (up = {})", stem.up);
        }
    }

    #[test]
    fn test_dot_clears_flags_on_up_stems() {
        let radius = DOT_DIAMETER_SPACES * STAFF_LINE_GAP / 2.0;
        for flags in 1..=4u8 {
            let stem = stem_geometry(50.0, MIDDLE - 10.0, MIDDLE, flags);
            assert!(stem.up);
            let mut ops = vec![];
            push_flags(&mut ops, &stem, flags);
            let flag_right = max_curve_x(&ops);
            for position in [0, 1] {
                let (dot_x, _) = dot_position(50.0, MIDDLE - 10.0, position, Some(&stem), flags, notehead_stem_anchor().0);
                assert!(dot_x - radius > flag_right, "dot hits {flags} flag(s) at position {position}");
            }
        }
    }

    #[test]
    fn test_down_stem_flags_do_not_push_the_dot_away() {
        let note_y = MIDDLE + 10.0;
        let plain = dot_position(50.0, note_y, 1, None, 0, notehead_stem_anchor().0);
        let stem = stem_geometry(50.0, note_y, MIDDLE, 3);
        assert!(!stem.up);
        assert_eq!(dot_position(50.0, note_y, 1, Some(&stem), 3, notehead_stem_anchor().0), plain);
    }

    #[test]
    fn test_flag_right_extent_is_sane() {
        let width = FLAG_WIDTH_SPACES * STAFF_LINE_GAP;
        let extent = flag_right_extent();
        assert!(extent > 0.8 * width && extent <= 1.15 * width, "extent was {extent}, width {width}");
    }

    #[test]
    fn test_push_dot_draws_a_small_filled_circle() {
        let mut ops = vec![];
        push_dot(&mut ops, 70.0, 80.0);
        assert_eq!(count_ops(&ops, "f"), 1);
        assert_eq!(count_ops(&ops, "q"), count_ops(&ops, "Q"));
        let cm = ops.iter().find(|op| op.operator == "cm").unwrap();
        let values: Vec<f32> = cm.operands.iter().map(|o| o.as_float().unwrap()).collect();
        assert_eq!(&values[4..], &[70.0, 80.0]);
        let radius = DOT_DIAMETER_SPACES * STAFF_LINE_GAP / 2.0;
        for (x, y) in path_points(&ops) {
            assert!(x.abs() <= radius + 1e-4 && y.abs() <= radius + 1e-4);
        }
    }

    #[test]
    fn test_notehead_is_balanced_and_closed() {
        let mut ops = vec![];
        push_notehead(&mut ops, 100.0, 650.0);
        let operators: Vec<&str> = ops.iter().map(|op| op.operator.as_str()).collect();
        assert_eq!(operators.first(), Some(&"q"));
        assert_eq!(operators.last(), Some(&"Q"));
        assert!(operators.contains(&"h"));
        assert!(operators.contains(&"f"));
    }

    // Fill count of a bare measure: only noteheads, flags, beams, dots and accidentals fill
    fn bare_fills(measures: &[Measure]) -> usize {
        let renderer = PdfNotationRenderer::new().without_clef().without_key_signature().without_time_signature();
        renderer.render_pages(measures).unwrap().iter().map(|page| count_ops(page, "f")).sum()
    }

    fn measure_of(time: TimeSignature, notes: &[(u8, crate::domain::NoteDuration, bool)]) -> Measure {
        let mut measure = Measure::new(1, time, key(60));
        for &(midi, duration, dotted) in notes {
            let note = Note::new(midi, duration);
            measure.notes.push(if dotted { note.dotted() } else { note });
        }
        measure
    }

    #[test]
    fn test_beamed_eighths_have_a_beam_and_no_flags() {
        use crate::domain::NoteDuration::*;
        // Four eighths, then a half note: four noteheads, one beam, and the half note's ring
        let measure = measure_of(
            TimeSignature::common_time(),
            &[(67, Eighth, false), (69, Eighth, false), (71, Eighth, false), (72, Eighth, false), (64, Half, false)],
        );
        assert_eq!(bare_fills(&[measure]), 4 + 1 + 1);
    }

    #[test]
    fn test_unbeamed_eighth_keeps_its_flag() {
        use crate::domain::NoteDuration::*;
        // A lone eighth between quarters: notehead + flag, plus three more noteheads
        let measure = measure_of(
            TimeSignature::common_time(),
            &[(60, Quarter, false), (62, Eighth, false), (64, Quarter, false), (65, Quarter, false)],
        );
        assert_eq!(bare_fills(&[measure]), 4 + 1);
    }

    #[test]
    fn test_notes_are_not_beamed_across_a_barline() {
        use crate::domain::NoteDuration::*;
        // A lone eighth at the end of one measure and another at the start of the next
        let end = measure_of(TimeSignature::common_time(), &[(60, Quarter, false), (62, Eighth, false)]);
        let start = measure_of(TimeSignature::common_time(), &[(64, Eighth, false), (65, Quarter, false)]);
        // Two noteheads and two flags for the eighths, two quarter noteheads
        assert_eq!(bare_fills(&[end, start]), 2 + 2 + 2);
    }

    #[test]
    fn test_six_eight_gets_two_beams_of_three() {
        use crate::domain::NoteDuration::*;
        let notes: Vec<_> = [67u8, 69, 71, 72, 71, 69].iter().map(|&p| (p, Eighth, false)).collect();
        assert_eq!(bare_fills(&[measure_of(TimeSignature::new(6, 8), &notes)]), 6 + 2);
    }

    #[test]
    fn test_sixteenths_get_two_beams() {
        use crate::domain::NoteDuration::*;
        let notes: Vec<_> = [64u8, 65, 67, 69].iter().map(|&p| (p, Sixteenth, false)).collect();
        // Four noteheads and two beams (one is the secondary beam)
        assert_eq!(bare_fills(&[measure_of(TimeSignature::common_time(), &notes)]), 4 + 2);
    }

    #[test]
    fn test_dotted_eighth_and_sixteenth_get_a_stub_and_a_dot() {
        use crate::domain::NoteDuration::*;
        let measure = measure_of(
            TimeSignature::common_time(),
            &[(64, Eighth, true), (67, Sixteenth, false)],
        );
        // Two noteheads, the main beam, the stub on the sixteenth, and the dot
        assert_eq!(bare_fills(&[measure]), 2 + 2 + 1);
    }

    #[test]
    fn test_every_stem_of_a_beamed_group_is_drawn() {
        use crate::domain::NoteDuration::*;
        let notes: Vec<_> = [64u8, 65, 67, 69].iter().map(|&p| (p, Eighth, false)).collect();
        let measure = measure_of(TimeSignature::common_time(), &notes);
        let renderer = PdfNotationRenderer::new().without_clef().without_key_signature().without_time_signature();
        let page = &renderer.render_pages(&[measure]).unwrap()[0];
        // 5 staff lines + 1 barline + 4 stems
        assert_eq!(count_ops(page, "S"), 5 + 1 + 4);
    }

    #[test]
    fn test_beamed_notes_carry_accidentals() {
        use crate::domain::NoteDuration::*;
        // C#4 and D#4 eighths beamed together: two noteheads, two sharps, one beam
        let measure = measure_of(TimeSignature::common_time(), &[(61, Eighth, false), (63, Eighth, false)]);
        assert_eq!(bare_fills(&[measure]), 2 + 2 + 1);
    }

    #[test]
    fn test_exercises_from_the_generator_render_with_beams() {
        // End to end: a generated exercise renders, and fewer shapes are needed than with flags
        let measures = crate::generator::ExerciseGenerator::new(crate::generator::GeneratorConfig::default())
            .generate_exercise(16);
        let renderer = PdfNotationRenderer::new();
        let pages = renderer.render_pages(&measures).unwrap();
        assert!(!pages.is_empty() && pages.iter().all(|page| !page.is_empty()));
    }

    #[test]
    fn test_many_generated_exercises_render_without_failing() {
        use crate::domain::KeyMode;
        use crate::generator::{ExerciseGenerator, GeneratorConfig, IntervalSelection, OctaveRange};
        for root in 47u8..=71 {
            for selection in [IntervalSelection::default(), IntervalSelection::steps_only(), IntervalSelection::larger_intervals()] {
                let mut config = GeneratorConfig::default();
                config.key = crate::domain::Key::new(root, KeyMode::Major);
                config.interval_selection = selection;
                config.range = OctaveRange::new(48, 95); // wide, to reach ledger lines on both sides
                let measures = ExerciseGenerator::new(config).generate_exercise(12);
                let pages = PdfNotationRenderer::new().render_pages(&measures).unwrap();
                assert!(!pages.is_empty());
            }
        }
    }

    // ----- bass clef -----

    fn bass_position(midi: u8, key_root: u8) -> i32 {
        staff_position(&crate::presentation::spelling::spell_pitch(midi, &key(key_root), false), Clef::Bass)
    }

    #[test]
    fn test_staff_position_bass() {
        assert_eq!(bass_position(43, 60), 0); // G2 bottom line
        assert_eq!(bass_position(45, 60), 1); // A2 first space
        assert_eq!(bass_position(47, 60), 2); // B2 second line
        assert_eq!(bass_position(48, 60), 3); // C3 second space
        assert_eq!(bass_position(50, 60), 4); // D3 middle line
        assert_eq!(bass_position(53, 60), 6); // F3 the F line, fourth line
        assert_eq!(bass_position(57, 60), 8); // A3 top line
        assert_eq!(bass_position(59, 60), 9); // B3 space above the staff
        assert_eq!(bass_position(60, 60), 10); // middle C: first ledger line above
        assert_eq!(bass_position(41, 60), -1); // F2 space below the staff
        assert_eq!(bass_position(40, 60), -2); // E2 first ledger line below
        assert_eq!(bass_position(36, 60), -4); // C2 second ledger line below
    }

    #[test]
    fn test_middle_c_is_two_octaves_apart_between_clefs() {
        // The same written position means a different note: the bottom line is E4 in treble, G2 in bass
        let treble_bottom = staff_position(&crate::presentation::spelling::spell_pitch(64, &key(60), false), Clef::Treble);
        let bass_bottom = staff_position(&crate::presentation::spelling::spell_pitch(43, &key(60), false), Clef::Bass);
        assert_eq!(treble_bottom, bass_bottom);
        // Middle C: one ledger line below treble, one above bass
        assert_eq!(position_in(60, 60), -2);
        assert_eq!(bass_position(60, 60), 10);
    }

    #[test]
    fn test_bass_key_signature_positions() {
        let sharps: Vec<i32> = key_signature_glyphs(&key(C_SHARP_MAJOR), Clef::Bass).iter().map(|g| g.2).collect();
        // F C G D A E B: fourth line, second space, fourth space, middle line, first space, third space, second line
        assert_eq!(sharps, vec![6, 3, 7, 4, 1, 5, 2]);
        let flats: Vec<i32> = key_signature_glyphs(&key(C_FLAT_MAJOR), Clef::Bass).iter().map(|g| g.2).collect();
        // B E A D G C F: second line, third space, first space, middle line, bottom line, second space,
        // and F flat in the space just below the staff
        assert_eq!(flats, vec![2, 5, 1, 4, 0, 3, -1]);
    }

    #[test]
    fn test_bass_key_signature_is_the_treble_pattern_a_third_lower() {
        let shifted = |glyphs: Vec<(Glyph, f32, i32)>| -> Vec<i32> { glyphs.iter().map(|g| g.2 + 2).collect() };
        for root in [C_SHARP_MAJOR, C_FLAT_MAJOR, A_MAJOR, B_FLAT_MAJOR] {
            let bass = shifted(key_signature_glyphs(&key(root), Clef::Bass));
            let treble: Vec<i32> = key_signature_glyphs(&key(root), Clef::Treble).iter().map(|g| g.2).collect();
            assert_eq!(bass, treble, "root {root}");
        }
    }

    #[test]
    fn test_bass_key_signature_needs_no_ledger_lines() {
        for root in [C_SHARP_MAJOR, C_FLAT_MAJOR] {
            for (_, _, position) in key_signature_glyphs(&key(root), Clef::Bass) {
                assert!(ledger_line_positions(position).is_empty(), "position {position}");
            }
        }
    }

    #[test]
    fn test_bass_key_signature_keeps_the_same_widths_and_offsets() {
        for root in [60, A_MAJOR, B_FLAT_MAJOR, C_SHARP_MAJOR] {
            assert_eq!(key_signature_width(&key(root), Clef::Bass), key_signature_width(&key(root), Clef::Treble));
        }
    }

    #[test]
    fn test_bass_clef_glyph_is_on_the_f_line() {
        // The clef's origin is the F line: its curl is above it and its tail well below
        let ys: Vec<f32> = BASS_CLEF
            .ops
            .iter()
            .flat_map(|op| match *op {
                PathOp::Move(_, y) | PathOp::Line(_, y) => vec![y],
                PathOp::Curve(_, c1y, _, c2y, _, y) => vec![c1y, c2y, y],
                PathOp::Close => vec![],
            })
            .collect();
        let top = ys.iter().cloned().fold(f32::MIN, f32::max);
        let bottom = ys.iter().cloned().fold(f32::MAX, f32::min);
        // The F line is 1 space under the top line and 3 spaces above the bottom line
        assert!(top > 0.5 && top <= 1.2, "top {top}");
        assert!(bottom < -2.0 && bottom > -3.0, "bottom {bottom}");
    }

    #[test]
    fn test_clef_picks_the_glyph_and_its_height() {
        assert_eq!(clef_glyph(Clef::Treble).width, TREBLE_CLEF.width);
        assert_eq!(clef_glyph(Clef::Bass).width, BASS_CLEF.width);
        assert_ne!(TREBLE_CLEF.width, BASS_CLEF.width);
    }

    #[test]
    fn test_header_uses_the_clef_width() {
        let treble = PdfNotationRenderer::new().header_layout(&key(60), None);
        let bass = PdfNotationRenderer::new().with_clef(Clef::Bass).header_layout(&key(60), None);
        assert!((treble.width - (HEADER_LEFT_PAD_SPACES + TREBLE_CLEF.width) * STAFF_LINE_GAP).abs() < 1e-3);
        assert!((bass.width - (HEADER_LEFT_PAD_SPACES + BASS_CLEF.width) * STAFF_LINE_GAP).abs() < 1e-3);
    }

    #[test]
    fn test_clef_glyph_is_drawn_on_its_reference_line() {
        let renderer = PdfNotationRenderer::new().with_clef(Clef::Bass).without_key_signature().without_time_signature();
        let measure = Measure::new(1, TimeSignature::common_time(), key(60));
        let page = &renderer.render_pages(&[measure]).unwrap()[0];
        // The only filled shape is the clef; read the vertical position from its scaling transform
        let cm = page.iter().find(|op| op.operator == "cm").unwrap();
        let values: Vec<f32> = cm.operands.iter().map(|o| o.as_float().unwrap()).collect();
        let bottom = renderer.first_staff_bottom_y();
        assert!((values[5] - (bottom + 6.0 * STAFF_STEP)).abs() < 1e-3, "clef y {}", values[5]);

        let treble = PdfNotationRenderer::new().without_key_signature().without_time_signature();
        let page = &treble.render_pages(&[Measure::new(1, TimeSignature::common_time(), key(60))]).unwrap()[0];
        let cm = page.iter().find(|op| op.operator == "cm").unwrap();
        let values: Vec<f32> = cm.operands.iter().map(|o| o.as_float().unwrap()).collect();
        assert!((values[5] - (bottom + 2.0 * STAFF_STEP)).abs() < 1e-3);
    }

    /// Vertical centre of every notehead drawn on the first page (they are the only tilted transforms)
    fn notehead_ys(renderer: &PdfNotationRenderer, measure: Measure) -> Vec<f32> {
        let page = renderer.render_pages(&[measure]).unwrap().remove(0);
        page.iter()
            .filter(|op| op.operator == "cm")
            .map(|op| op.operands.iter().map(|o| o.as_float().unwrap()).collect::<Vec<f32>>())
            .filter(|v| v[1].abs() > 1e-4)
            .map(|v| v[5])
            .collect()
    }

    #[test]
    fn test_same_pitch_sits_on_different_lines_in_each_clef() {
        use crate::domain::NoteDuration::Quarter;
        let measure = |midi: u8| {
            let mut m = Measure::new(1, TimeSignature::common_time(), key(60));
            m.notes.push(Note::new(midi, Quarter));
            m
        };
        let treble = PdfNotationRenderer::new().without_clef().without_key_signature().without_time_signature();
        let bass = PdfNotationRenderer::new()
            .with_clef(Clef::Bass)
            .without_clef()
            .without_key_signature()
            .without_time_signature();
        let bottom = treble.first_staff_bottom_y();

        // E4 is the bottom line of the treble staff, G2 the bottom line of the bass staff
        assert_eq!(notehead_ys(&treble, measure(64)), vec![bottom]);
        assert_eq!(notehead_ys(&bass, measure(43)), vec![bottom]);
        // Middle C is below the treble staff and above the bass staff
        assert!(notehead_ys(&treble, measure(60))[0] < bottom);
        assert!(notehead_ys(&bass, measure(60))[0] > bottom + 8.0 * STAFF_STEP);
    }

    #[test]
    fn test_bass_stems_follow_the_middle_line_of_the_bass_staff() {
        use crate::domain::NoteDuration::Quarter;
        let renderer = PdfNotationRenderer::new()
            .with_clef(Clef::Bass)
            .without_clef()
            .without_key_signature()
            .without_time_signature();
        // D3 (50) is on the bass middle line, F3 (53) above it, B2 (47) below it
        let stem_end = |midi: u8| -> (f32, f32) {
            let mut m = Measure::new(1, TimeSignature::common_time(), key(60));
            m.notes.push(Note::new(midi, Quarter));
            let page = renderer.render_pages(&[m]).unwrap().remove(0);
            let head_y = notehead_ys(&renderer, {
                let mut m = Measure::new(1, TimeSignature::common_time(), key(60));
                m.notes.push(Note::new(midi, Quarter));
                m
            })[0];
            let line_end = page
                .iter()
                .rev()
                .find(|op| op.operator == "l")
                .map(|op| op.operands[1].as_float().unwrap())
                .unwrap();
            (head_y, line_end)
        };
        let (head, end) = stem_end(53); // above the middle line: stem down
        assert!(end < head);
        let (head, end) = stem_end(47); // below the middle line: stem up
        assert!(end > head);
    }

    #[test]
    fn test_bass_exercises_render_in_every_key_without_failing() {
        use crate::domain::KeyMode;
        use crate::generator::{ExerciseGenerator, GeneratorConfig, IntervalSelection, OctaveRange};
        for root in 47u8..=71 {
            for selection in [IntervalSelection::default(), IntervalSelection::larger_intervals()] {
                let mut config = GeneratorConfig::default();
                config.key = crate::domain::Key::new(root, KeyMode::Major);
                config.interval_selection = selection;
                config.range = OctaveRange::new(36, 59); // the default bass range, C2 to B3
                let measures = ExerciseGenerator::new(config).generate_exercise(12);
                let pages = PdfNotationRenderer::new().with_clef(Clef::Bass).render_pages(&measures).unwrap();
                assert!(!pages.is_empty());
            }
        }
    }

    #[test]
    fn test_bass_beams_work_like_treble_beams() {
        use crate::domain::NoteDuration::*;
        // Four eighths in bass clef: noteheads plus one beam, no flags
        let mut measure = Measure::new(1, TimeSignature::common_time(), key(60));
        for midi in [43u8, 45, 47, 48] {
            measure.notes.push(Note::new(midi, Eighth));
        }
        let renderer = PdfNotationRenderer::new()
            .with_clef(Clef::Bass)
            .without_clef()
            .without_key_signature()
            .without_time_signature();
        assert_eq!(count_ops(&renderer.render_pages(&[measure]).unwrap()[0], "f"), 4 + 1);
    }

    // ----- whole, half and quarter notes -----

    /// Counts of stroked shapes, filled shapes and closed outlines on the page for one bare measure
    /// holding one note. A ring (a hollow head) has two outlines, the outer edge and the hole.
    fn shape_of(duration: crate::domain::NoteDuration, midi: u8) -> (usize, usize, usize) {
        let mut measure = Measure::new(1, TimeSignature::common_time(), key(60));
        measure.notes.push(Note::new(midi, duration));
        let renderer = PdfNotationRenderer::new().without_clef().without_key_signature().without_time_signature();
        let page = renderer.render_pages(&[measure]).unwrap().remove(0);
        (count_ops(&page, "S"), count_ops(&page, "f"), count_ops(&page, "h"))
    }

    fn strokes_and_fills(duration: crate::domain::NoteDuration, midi: u8) -> (usize, usize) {
        let (strokes, fills, _) = shape_of(duration, midi);
        (strokes, fills)
    }

    // Five staff lines and one barline are stroked on every page of this kind
    const BASE_STROKES: usize = 5 + 1;

    #[test]
    fn test_a_half_note_has_a_hollow_head_and_a_stem() {
        use crate::domain::NoteDuration::Half;
        let (strokes, fills, outlines) = shape_of(Half, 67);
        // The head is one filled ring with a hole in it (two outlines), and the stem is a line
        assert_eq!(strokes, BASE_STROKES + 1);
        assert_eq!(fills, 1);
        assert_eq!(outlines, 2);
    }

    #[test]
    fn test_a_whole_note_has_a_filled_ring_and_no_stem() {
        use crate::domain::NoteDuration::Whole;
        let (strokes, fills) = strokes_and_fills(Whole, 67);
        assert_eq!(strokes, BASE_STROKES, "a whole note has no stem");
        assert_eq!(fills, 1, "the ring is one filled outline");
    }

    #[test]
    fn test_a_quarter_note_has_a_filled_head_and_a_stem() {
        use crate::domain::NoteDuration::Quarter;
        // A solid oval: one outline and no hole
        assert_eq!(shape_of(Quarter, 67), (BASE_STROKES + 1, 1, 1));
    }

    #[test]
    fn test_half_whole_and_quarter_notes_are_drawn_differently() {
        use crate::domain::NoteDuration::*;
        // Half: stem and a ring. Whole: a ring and no stem. Quarter: stem and a solid head.
        let half = shape_of(Half, 67);
        let whole = shape_of(Whole, 67);
        let quarter = shape_of(Quarter, 67);
        assert_ne!(half, whole);
        assert_ne!(half, quarter);
        assert_ne!(whole, quarter);
    }

    #[test]
    fn test_whole_notes_never_get_a_stem_at_any_pitch() {
        use crate::domain::NoteDuration::Whole;
        for midi in 36u8..=96 {
            assert_eq!(strokes_and_fills(Whole, midi).0, BASE_STROKES + ledger_strokes(midi), "midi {midi}");
        }
    }

    /// Ledger lines a note needs in treble clef, each of which is stroked
    fn ledger_strokes(midi: u8) -> usize {
        ledger_line_positions(position_in(midi, 60)).len()
    }

    #[test]
    fn test_half_notes_get_a_stem_at_every_pitch() {
        use crate::domain::NoteDuration::Half;
        for midi in 36u8..=96 {
            let (strokes, _) = strokes_and_fills(Half, midi);
            assert_eq!(strokes, BASE_STROKES + ledger_strokes(midi) + 1, "midi {midi}");
        }
    }

    #[test]
    fn test_a_half_note_stem_matches_a_quarter_note_stem() {
        use crate::domain::NoteDuration::{Half, Quarter};
        // Same direction, same length: only the head differs
        let stem_line = |duration, midi| -> Vec<f32> {
            let mut measure = Measure::new(1, TimeSignature::common_time(), key(60));
            measure.notes.push(Note::new(midi, duration));
            let renderer = PdfNotationRenderer::new().without_clef().without_key_signature().without_time_signature();
            let page = renderer.render_pages(&[measure]).unwrap().remove(0);
            // The stem is the last line drawn
            let last_line = page.iter().rposition(|op| op.operator == "l").unwrap();
            let start = &page[last_line - 1];
            let end = &page[last_line];
            [start, end].iter().flat_map(|op| op.operands.iter().map(|o| o.as_float().unwrap())).collect()
        };
        for midi in [55u8, 62, 67, 71, 72, 79, 84] {
            assert_eq!(stem_line(Half, midi), stem_line(Quarter, midi), "midi {midi}");
        }
    }

    #[test]
    fn test_half_note_stems_follow_the_middle_line() {
        use crate::domain::NoteDuration::Half;
        let stem_goes_up = |midi: u8| -> bool {
            let mut measure = Measure::new(1, TimeSignature::common_time(), key(60));
            measure.notes.push(Note::new(midi, Half));
            let renderer = PdfNotationRenderer::new().without_clef().without_key_signature().without_time_signature();
            let page = renderer.render_pages(&[measure]).unwrap().remove(0);
            let last_line = page.iter().rposition(|op| op.operator == "l").unwrap();
            let start_y = page[last_line - 1].operands[1].as_float().unwrap();
            let end_y = page[last_line].operands[1].as_float().unwrap();
            end_y > start_y
        };
        assert!(stem_goes_up(64)); // E4, bottom line
        assert!(stem_goes_up(67)); // G4
        assert!(!stem_goes_up(71)); // B4, middle line: down
        assert!(!stem_goes_up(77)); // F5
    }

    #[test]
    fn test_dotted_half_note_keeps_its_dot_clear_of_the_stem() {
        use crate::domain::NoteDuration::Half;
        let mut measure = Measure::new(1, TimeSignature::common_time(), key(60));
        measure.notes.push(Note::new(67, Half).dotted());
        let renderer = PdfNotationRenderer::new().without_clef().without_key_signature().without_time_signature();
        let page = renderer.render_pages(&[measure]).unwrap().remove(0);
        assert_eq!(count_ops(&page, "f"), 2, "the ring and the dot");
    }

    #[test]
    fn test_whole_note_head_is_wider_than_the_others() {
        use crate::domain::NoteDuration::*;
        let whole = notehead_half_width(Whole);
        assert!((whole - 1.688 * STAFF_LINE_GAP / 2.0).abs() < 1e-3);
        for narrow in [Half, Quarter, Eighth, Sixteenth] {
            assert!(whole > notehead_half_width(narrow) + 0.5, "{narrow:?}");
        }
        assert_eq!(notehead_half_width(Half), notehead_half_width(Quarter));
    }

    #[test]
    fn test_whole_note_head_is_centred_on_the_note() {
        let mut ops = vec![];
        push_whole_notehead(&mut ops, 100.0, 80.0);
        let cm = ops.iter().find(|op| op.operator == "cm").unwrap();
        let values: Vec<f32> = cm.operands.iter().map(|o| o.as_float().unwrap()).collect();
        let half = WHOLE_NOTEHEAD.width * STAFF_LINE_GAP / 2.0;
        assert!((values[4] + half - 100.0).abs() < 1e-3, "left edge at {}", values[4]);
        assert_eq!(values[5], 80.0);
        assert_eq!(count_ops(&ops, "f"), 1);
        // Two contours: the outer edge and the hole
        assert_eq!(count_ops(&ops, "h"), 2);
    }

    #[test]
    fn test_ledger_lines_are_longer_under_a_whole_note() {
        use crate::domain::NoteDuration::*;
        let (whole_start, whole_end) = ledger_line_extent(100.0, notehead_half_width(Whole));
        let (half_start, half_end) = ledger_line_extent(100.0, notehead_half_width(Half));
        assert!(whole_end - whole_start > half_end - half_start + 2.0);
        // Still centred, and still just over two noteheads wide (Gould)
        assert!(((whole_start + whole_end) / 2.0 - 100.0).abs() < 1e-4);
        let head_width = 2.0 * notehead_half_width(Whole);
        assert!(whole_end - whole_start > head_width && whole_end - whole_start < head_width + 3.0 * LEDGER_LINE_EXTENSION);
    }

    #[test]
    fn test_accidental_and_dot_keep_clear_of_a_whole_note_head() {
        use crate::domain::NoteDuration::Whole;
        let half_width = notehead_half_width(Whole);
        let glyph = SHARP;
        let origin = accidental_origin_x(100.0, false, &glyph, half_width);
        assert!(origin + glyph.width * STAFF_LINE_GAP < 100.0 - half_width, "the sharp overlaps the head");
        let (dot_x, _) = dot_position(100.0, 50.0, 1, None, 0, half_width);
        let radius = DOT_DIAMETER_SPACES * STAFF_LINE_GAP / 2.0;
        assert!(dot_x - radius > 100.0 + half_width, "the dot overlaps the head");
    }

    #[test]
    fn test_whole_note_uses_the_wider_head_for_its_dot_in_a_real_render() {
        use crate::domain::NoteDuration::Whole;
        let dot_centre_x = |dotted: bool| -> f32 {
            let mut measure = Measure::new(1, TimeSignature::common_time(), key(60));
            let note = Note::new(67, Whole);
            measure.notes.push(if dotted { note.dotted() } else { note });
            let renderer = PdfNotationRenderer::new().without_clef().without_key_signature().without_time_signature();
            let page = renderer.render_pages(&[measure]).unwrap().remove(0);
            // The last transform in the page is the dot's, when there is one
            let cm = page.iter().rev().find(|op| op.operator == "cm").unwrap();
            cm.operands[4].as_float().unwrap()
        };
        let head_left = dot_centre_x(false);
        let dot = dot_centre_x(true);
        let note_x = head_left + notehead_half_width(Whole);
        assert!(dot > note_x + notehead_half_width(Whole), "dot at {dot}, head ends at {}", note_x + notehead_half_width(Whole));
    }
}

use super::*;
use crate::{Color, RichTextParagraph, RichTextParagraphType};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Capture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct SourceCapture {
    cases: Vec<SourceCase>,
}

#[derive(Deserialize)]
struct SourceCase {
    constructed: ConstructorProfile,
}

#[derive(Deserialize)]
struct ConstructorProfile {
    font_size_at_0_1_2_bits: [u32; 3],
    color_at_0_argb: u32,
    alignment: u32,
    gravity: u8,
}

fn constructor_profile() -> ConstructorProfile {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-parsed-cell-text.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "5801413a28a251d1313aa5574bd5882c3b70c05aee123bd463e2df35a47f55a7"
    );
    let capture: SourceCapture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.cases.len(), 7);
    for case in &capture.cases {
        assert_eq!(
            case.constructed.font_size_at_0_1_2_bits,
            [50.0_f32.to_bits(), 17.0_f32.to_bits(), 17.0_f32.to_bits()]
        );
        assert_eq!(case.constructed.color_at_0_argb, 0xff25_2525);
        assert_eq!(case.constructed.alignment, 2);
        assert_eq!(case.constructed.gravity, 1);
    }
    capture.cases.into_iter().next().unwrap().constructed
}

#[derive(Deserialize)]
struct Case {
    name: String,
    bounds_bits: [u32; 4],
    texts_utf8: [String; 4],
    merge: Option<[u32; 4]>,
    font_size_bits: Option<u32>,
    margin_bits: Option<[u32; 4]>,
    text_scale_bits: u32,
    document_density_bits: u32,
    supplied_local_split_band_bits: Vec<[u32; 4]>,
    warm_first_column_width_bits: Option<u32>,
    states: Vec<State>,
    #[serde(default)]
    after_warm_native_padding: Vec<NativePadding>,
}

#[derive(Deserialize)]
struct NativePadding {
    slot: usize,
    stored_max_character_height_bits: u32,
    native_getter_max_character_height_bits: u32,
    sorted_native_padding_rect_bits: Vec<[u32; 4]>,
}

#[derive(Deserialize)]
struct State {
    stage: String,
    content_rect_bits: [u32; 4],
    measured_rect_bits: [u32; 4],
    cells: Vec<Cell>,
}

#[derive(Deserialize)]
struct Cell {
    slot: usize,
    owner_slot: usize,
    source_rect_bits: [u32; 4],
    frame_bits: [u32; 4],
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    position_bits: [u32; 2],
    layout_rect_bits: [u32; 4],
}

fn rect(bits: [u32; 4]) -> BoundingBox {
    let [x_min, y_min, x_max, y_max] = bits.map(|value| f64::from(f32::from_bits(value)));
    BoundingBox {
        x_min,
        y_min,
        x_max,
        y_max,
    }
}

fn bits(rect: BoundingBox) -> [u32; 4] {
    [rect.x_min, rect.y_min, rect.x_max, rect.y_max].map(|value| (value as f32).to_bits())
}

fn source_table(case: &Case, profile: &ConstructorProfile) -> RichTextTable {
    let [left, top, right, bottom] = case.bounds_bits.map(f32::from_bits);
    let width = (right - left) * 0.5;
    let height = (bottom - top) * 0.5;
    let mut table = tests::grid(&[height; 2], &[width; 2]);
    table.bbox = rect(case.bounds_bits);
    table.style.content_bbox = Some(table.bbox);
    for (slot, cell) in table
        .rows
        .iter_mut()
        .flat_map(|row| &mut row.cells)
        .enumerate()
    {
        let x = left + width * (slot % 2) as f32;
        let y = top + height * (slot / 2) as f32;
        cell.bbox = rect([x, y, x + width, y + height].map(f32::to_bits));
        cell.content.bbox = cell.bbox;
        cell.content.text = case.texts_utf8[slot].clone();
        cell.content.font_size = Some(f32::from_bits(
            case.font_size_bits
                .unwrap_or(profile.font_size_at_0_1_2_bits[0]),
        ));
        cell.content.color = Some(Color {
            r: (profile.color_at_0_argb >> 16) as u8,
            g: (profile.color_at_0_argb >> 8) as u8,
            b: profile.color_at_0_argb as u8,
        });
        cell.content.gravity = Some(profile.gravity);
        cell.content.margins = case.margin_bits.map(|margins| margins.map(f32::from_bits));
        cell.content.paragraphs.push(RichTextParagraph {
            kind: RichTextParagraphType::Alignment,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: profile.alignment.to_le_bytes().to_vec(),
        });
    }
    if let Some([row, column, last_row, last_column]) = case.merge {
        let cell = &mut table.rows[row as usize].cells[column as usize];
        cell.row_span = last_row - row + 1;
        cell.column_span = last_column - column + 1;
    }
    table
}

fn assert_geometry(plan: &PreparedTable, table: &RichTextTable, state: &State, name: &str) {
    let label = format!("{name} {}", state.stage);
    assert_eq!(
        bits(plan.content_bbox),
        state.content_rect_bits,
        "{label} content"
    );
    assert_eq!(
        bits(plan.measured_bbox),
        state.measured_rect_bits,
        "{label} measured"
    );
    for cell in &state.cells {
        let row = cell.slot / 2;
        let column = cell.slot % 2;
        assert_eq!(
            bits(plan.rows[row].cells[column].frame),
            cell.frame_bits,
            "{label} slot{} frame",
            cell.slot
        );
        assert_eq!(
            bits(table.rows[row].cells[column].content.bbox),
            cell.source_rect_bits,
            "{label} immutable source{}",
            cell.slot
        );
        let owner = plan
            .topology
            .frame_owner(CellPosition { row, column })
            .unwrap();
        assert_eq!(
            owner.row * 2 + owner.column,
            cell.owner_slot,
            "{label} slot{} owner",
            cell.slot
        );
        let mut native_lines = Vec::new();
        for entry in &cell.entries {
            let line = [
                entry.layout_rect_bits[1],
                entry.position_bits[1],
                entry.layout_rect_bits[3],
            ];
            if native_lines.last() != Some(&line) {
                native_lines.push(line);
            }
        }
        if !native_lines.is_empty() {
            let actual = &plan.rows[row].cells[column];
            assert_eq!(
                actual.layout.lines.len(),
                native_lines.len(),
                "{label} slot{} lines",
                cell.slot
            );
            for (line, native) in actual.layout.lines.iter().zip(native_lines) {
                let expected =
                    native.map(|value| f64::from(f32::from_bits(value)) + actual.frame.y_min);
                assert_eq!(
                    [line.top, line.baseline, line.bottom],
                    expected,
                    "{label} slot{} text bounds",
                    cell.slot
                );
            }
        }
    }
}

fn cold_plan(
    case: &Case,
    table: &RichTextTable,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> PreparedTable {
    let half_border = drawable_half_border(table);
    let mut plan = PreparedTable {
        measured_bbox: BoundingBox::default(),
        content_bbox: BoundingBox::default(),
        rows: initialize_rows(table, half_border).unwrap(),
        min_first_page_height: 0.0,
        constraint: ObjectSpanLayoutConstraint::Normal,
        bands: BandList::new(
            case.supplied_local_split_band_bits
                .iter()
                .copied()
                .map(rect)
                .collect(),
        )
        .unwrap(),
        pending_gaps: vec![0.0; 2],
        half_border,
        topology: TableGrid::new(table).unwrap(),
    };
    pagination::cold(&mut plan, table, theme, renderer)
        .unwrap_or_else(|error| panic!("{} cold: {error:?}", case.name));
    plan.update_geometry(table, renderer).unwrap();
    plan
}

fn compare_case(case: &Case, profile: &ConstructorProfile, fonts: &crate::fonts::FontBook) {
    let table = source_table(case, profile);
    let source = table.clone();
    let theme = RenderTheme::for_canvas(false);
    let renderer = TextRenderer::new(
        super::super::text::TextSettings {
            scale: f32::from_bits(case.document_density_bits)
                * f32::from_bits(case.text_scale_bits),
            ..Default::default()
        },
        fonts,
    );
    let mut plan = cold_plan(case, &table, theme, &renderer);
    assert_geometry(&plan, &table, &case.states[0], &case.name);
    pagination::warm(&mut plan, &table, theme, &renderer).unwrap();
    plan.update_geometry(&table, &renderer).unwrap();
    assert_geometry(&plan, &table, &case.states[1], &case.name);
    assert_eq!(table, source, "{} source", case.name);
    for native in &case.after_warm_native_padding {
        let cell = &plan.rows[native.slot / 2].cells[native.slot % 2];
        let bands = cell.bands.as_ref().unwrap();
        assert_eq!(
            bands.spacing_capacity().to_bits(),
            native.stored_max_character_height_bits,
            "{} slot{} spacing capacity",
            case.name,
            native.slot
        );
        assert_eq!(
            native.stored_max_character_height_bits,
            native.native_getter_max_character_height_bits
        );
        assert_eq!(
            bands
                .padding_rectangles()
                .into_iter()
                .map(|rect| bits(*rect))
                .collect::<Vec<_>>(),
            native.sorted_native_padding_rect_bits,
            "{} slot{} native padding",
            case.name,
            native.slot
        );
    }
    if let Some(width) = case.warm_first_column_width_bits {
        let mut resized = table.clone();
        resized.column_widths[0] = f32::from_bits(width);
        let source = resized.clone();
        let mut reflowed = cold_plan(case, &resized, theme, &renderer);
        pagination::warm(&mut reflowed, &resized, theme, &renderer).unwrap();
        reflowed.update_geometry(&resized, &renderer).unwrap();
        assert_geometry(&reflowed, &resized, &case.states[2], &case.name);
        assert_eq!(resized, source);
    }
}

#[test]
fn whole_native_table_layout_matches_cold_and_warm_rust_geometry() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-live-layout.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "cedaea603a3aa3c6e07ea8bdd09af7ec7c1186f35cba0e8c4282dd5fe8309b87"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.cases.len(), 18);
    assert_eq!(
        capture
            .cases
            .iter()
            .map(|case| case.states.len())
            .sum::<usize>(),
        38
    );
    assert_eq!(
        capture
            .cases
            .iter()
            .flat_map(|case| &case.states)
            .map(|state| state.cells.len())
            .sum::<usize>(),
        152
    );
    let fonts = crate::fonts::FontBook::default();
    let profile = constructor_profile();
    for case in &capture.cases {
        compare_case(case, &profile, &fonts);
    }
}

#[test]
fn native_table_padding_matches_geometry_and_first_pair_spacing_capacity() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-live-padding.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "d9c2d969448ea5f2c517f9bdf47c91c68a9dae0866dca3b84897a9ccdd9907d5"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.cases.len(), 10);
    let fonts = crate::fonts::FontBook::default();
    let profile = constructor_profile();
    for case in &capture.cases {
        assert_eq!(case.states.len(), 2);
        assert_eq!(case.after_warm_native_padding.len(), 4);
        compare_case(case, &profile, &fonts);
    }
}

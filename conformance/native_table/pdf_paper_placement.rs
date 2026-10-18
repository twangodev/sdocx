use super::*;

const COMPOSER_BASE: u64 = 0x0100_0000;
const BASE_BASE: u64 = 0x0500_0000;
const VIEW_BASE: u64 = 0x0300_0000;
const STUBS: u64 = 0x0800_0000;
const PAGE_BACKGROUND: u64 = MODEL + 0x10000;
const VIEW_ARRAY: u64 = MODEL + 0x11000;
const VIEWS: u64 = MODEL + 0x12000;
const RECORDS: u64 = MODEL + 0x20000;
const VTABLE: u64 = MODEL + 0x30000;
const CANVAS: u64 = MODEL + 0x50000;
const CANVAS_VTABLE: u64 = MODEL + 0x60000;
const WIDTH_STUB: u64 = STUBS;
const WIDTH_VALUE: u64 = STUBS + 0x100;
const RETURN_STUB: u64 = STUBS + 0x20;
const CLIP_CAPTURE_STUB: u64 = STUBS + 0x40;
const REGISTER_X19: i32 = REGISTER_X0 + 19;
const REGISTER_X22: i32 = REGISTER_X0 + 22;
const REGISTER_X23: i32 = REGISTER_X0 + 23;

#[derive(Clone)]
struct Placement {
    case_name: String,
    note_width: u32,
    note_height: u32,
    binding_id: i32,
    source_page_index: i32,
    rect: [f32; 4],
    source_box_host: Option<[f32; 4]>,
}

fn retained_records() -> Vec<Placement> {
    // Frozen production-parser outputs; source hashes are emitted in the fixture.
    // The source boxes below are separate host metadata from the inventoried PDFs,
    // not placement inputs; the native fragment receives only the note rectangles.
    // This keeps the oracle independent of ignored .sdocx, CSV and scratch inputs.
    let mut records = vec![Placement {
        case_name: "quiz".into(),
        note_width: 1812,
        note_height: 15372,
        binding_id: 0,
        source_page_index: 0,
        rect: [0.0, 0.0, 1812.0, 1485.0],
        source_box_host: Some([0.0, 0.0, 726.20428, 595.44006]),
    }];
    let heights: [i32; 20] = [
        1397, 1397, 1398, 1398, 1397, 1398, 1398, 1397, 1398, 1398, 1397, 1398, 1398, 1397, 1398,
        1398, 1398, 1397, 1397, 1398,
    ];
    let mut top = 0.0f32;
    for (page, height) in heights.into_iter().enumerate() {
        let bottom = top + height as f32;
        records.push(Placement {
            case_name: "continuous".into(),
            note_width: 1080,
            note_height: 27952,
            binding_id: 0,
            source_page_index: page as i32,
            rect: [0.0, top, 1080.0, bottom],
            source_box_host: Some([0.0, 0.0, 612.0, 792.0]),
        });
        top = bottom;
    }
    assert_eq!(top, 27952.0);
    records
}

fn write_width_stub(engine: Engine, width: u32) {
    write(engine, WIDTH_VALUE, &width.to_le_bytes());
}

fn assert_host_arena_ranges(placement_count: usize) {
    let regions = [
        ("page background", PAGE_BACKGROUND, 1280_u64),
        ("view pointer array", VIEW_ARRAY, placement_count as u64 * 8),
        ("views", VIEWS, placement_count as u64 * 0x400),
        ("records", RECORDS, placement_count as u64 * 0x40),
        ("vtables", VTABLE, placement_count as u64 * 0x200),
        ("canvases", CANVAS, placement_count as u64 * 0x100),
        (
            "canvas vtables",
            CANVAS_VTABLE,
            placement_count as u64 * 0x100,
        ),
    ];
    let arena_end = MODEL + 0x100000;
    for (name, start, size) in regions {
        let end = start.checked_add(size).expect("host arena range overflow");
        assert!(
            start >= MODEL && end <= arena_end,
            "{name} escaped the mapped MODEL arena"
        );
    }
    for left in 0..regions.len() {
        let (left_name, left_start, left_size) = regions[left];
        let left_end = left_start + left_size;
        for &(right_name, right_start, right_size) in &regions[left + 1..] {
            let right_end = right_start + right_size;
            assert!(
                left_end <= right_start || right_end <= left_start,
                "host arena ranges overlap: {left_name} and {right_name}"
            );
        }
    }
}

fn write_ret_to_stop(engine: Engine, address: u64) {
    // LDR X30, #8; RET; literal STOP. This bounds the executed native caller
    // slice immediately after its SetPosition callback returns.
    write(engine, address, &0x5800_005e_u32.to_le_bytes());
    write(engine, address + 4, &0xd65f_03c0_u32.to_le_bytes());
    write(engine, address + 8, &STOP.to_le_bytes());
}

fn execute_one(machine: &Machine, placement: &Placement, ordinal: usize) -> String {
    let engine = machine.engine;
    let page_background = PAGE_BACKGROUND;
    let first_view = VIEWS + ordinal as u64 * 0x400;
    let record = RECORDS + ordinal as u64 * 0x40;
    let vtable = VTABLE + ordinal as u64 * 0x200;
    let canvas = CANVAS + ordinal as u64 * 0x100;
    let canvas_vtable = CANVAS_VTABLE + ordinal as u64 * 0x100;

    write(engine, page_background, &[0; 1280]);
    write(engine, VIEW_ARRAY, &[0; 0x100]);
    for index in 0..=ordinal {
        write(
            engine,
            VIEW_ARRAY + index as u64 * 8,
            &(VIEWS + index as u64 * 0x400).to_le_bytes(),
        );
    }
    write(engine, page_background + 1232, &VIEW_ARRAY.to_le_bytes());
    write(
        engine,
        page_background + 1240,
        &(VIEW_ARRAY + (ordinal as u64 + 1) * 8).to_le_bytes(),
    );
    write(engine, record, &[0; 64]);
    write(engine, record + 16, &placement.binding_id.to_le_bytes());
    write(
        engine,
        record + 20,
        &placement.source_page_index.to_le_bytes(),
    );
    for (axis, value) in placement.rect.iter().enumerate() {
        write(engine, record + 24 + axis as u64 * 4, &value.to_le_bytes());
    }
    write(engine, first_view, &[0; 512]);
    write(engine, first_view, &vtable.to_le_bytes());
    write(engine, first_view + 309, &[1]); // Skip unrelated pivot callbacks.
    write(engine, vtable + 336, &(VIEW_BASE + 0x70b8c).to_le_bytes());
    write(engine, vtable + 456, &RETURN_STUB.to_le_bytes());
    write(engine, canvas, &[0; 16]);
    write(engine, canvas, &canvas_vtable.to_le_bytes());
    write(engine, canvas_vtable + 48, &RETURN_STUB.to_le_bytes());
    write(engine, canvas_vtable + 64, &CLIP_CAPTURE_STUB.to_le_bytes());

    write_width_stub(engine, placement.note_width);
    assert_eq!(
        machine.call(WIDTH_STUB, &[0, WIDTH_VALUE]) as u32,
        placement.note_width,
        "host width stub output mismatch"
    );
    assert_eq!(
        machine.call(COMPOSER_BASE + 0x54eeb0, &[0, WIDTH_VALUE]) as u32,
        placement.note_width,
        "WPage::GetWidth PLT boundary mismatch"
    );
    register(engine, REGISTER_X19, page_background);
    register(engine, REGISTER_X22, ordinal as u64);
    register(engine, REGISTER_X23, record);
    register(engine, REGISTER_X0 + 1, WIDTH_VALUE);
    machine.call(COMPOSER_BASE + 0x3e6734, &[PAGE_BACKGROUND + 1128]);
    assert_eq!(
        read_register(engine, REGISTER_X0 + 24) as u32,
        placement.note_width,
        "native WPage-width input was not retained for {} page {}",
        placement.case_name,
        placement.source_page_index
    );
    let placed = [
        read_float(engine, first_view + 276),
        read_float(engine, first_view + 280),
        read_float(engine, first_view + 284),
        read_float(engine, first_view + 288),
    ];

    machine.call(VIEW_BASE + 0x70df8, &[first_view, 1]);
    write(engine, first_view + 304, &[1]); // Active View::Draw guard.
    write_ret_to_stop(engine, VIEW_BASE + 0x6fe4c);
    machine.call(VIEW_BASE + 0x6fd7c, &[first_view, canvas]);
    let clip_width = read_u32(engine, canvas) as i32;
    let clip_height = read_u32(engine, canvas + 4) as i32;
    assert!(clip_width > 0 && clip_height > 0);
    let source_box = placement
        .source_box_host
        .map(|rect| format!("[{},{},{},{}]", rect[0], rect[1], rect[2], rect[3]))
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"case\":{:?},\"note_width\":{},\"note_height\":{},\"binding_id\":{},\"source_page_index\":{},\"saved_rect\":{:?},\"source_page_box_host_only\":{},\"native_view_rect\":{:?},\"native_clip_px\":[{},{}]}}",
        placement.case_name,
        placement.note_width,
        placement.note_height,
        placement.binding_id,
        placement.source_page_index,
        placement.rect,
        source_box,
        placed,
        clip_width,
        clip_height,
    )
}

pub(super) fn capture(machine: &mut Machine, composer: &Path, base: &Path, view: &Path) {
    map_library(
        machine.engine,
        composer,
        COMPOSER_BASE,
        "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f",
    );
    map_library(
        machine.engine,
        base,
        BASE_BASE,
        "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
    );
    map_library(
        machine.engine,
        view,
        VIEW_BASE,
        "c4a17e4232c2074d3833604974d75ac961fab4d9651949bb2552704c86621dc8",
    );
    check(unsafe { uc_mem_map(machine.engine, STUBS, 0x1000, 7) });
    write(machine.engine, WIDTH_STUB, &0xb940_0020_u32.to_le_bytes()); // LDR W0, [X1]
    write(
        machine.engine,
        WIDTH_STUB + 4,
        &0xd65f_03c0_u32.to_le_bytes(),
    );
    bind_native(machine.engine, COMPOSER_BASE + 0x54eeb0, WIDTH_STUB);
    bind_native(
        machine.engine,
        COMPOSER_BASE + 0x54db30,
        BASE_BASE + 0xb108c,
    );
    write(machine.engine, RETURN_STUB, &0xd65f03c0_u32.to_le_bytes());
    write(
        machine.engine,
        CLIP_CAPTURE_STUB,
        &[
            0x03, 0x00, 0x00, 0xb9, 0x04, 0x04, 0x00, 0xb9, 0xc0, 0x03, 0x5f, 0xd6,
        ],
    );
    write_ret_to_stop(machine.engine, COMPOSER_BASE + 0x3e67a0);

    let mut placements = retained_records();
    assert_eq!(
        placements
            .iter()
            .filter(|row| row.case_name == "quiz")
            .count(),
        1
    );
    assert_eq!(
        placements
            .iter()
            .filter(|row| row.case_name == "continuous")
            .count(),
        20
    );
    placements.push(Placement {
        case_name: "synthetic_nonzero_origin".to_string(),
        note_width: 1080,
        note_height: 2000,
        binding_id: 0,
        source_page_index: 0,
        rect: [23.0, 17.0, 1000.0, 777.0],
        source_box_host: None,
    });
    placements.extend([
        Placement {
            case_name: "fractional_origin_changed_width".into(),
            note_width: 1080,
            note_height: 2000,
            binding_id: 0,
            source_page_index: 0,
            rect: [23.25, 17.5, 1000.25, 777.75],
            source_box_host: None,
        },
        Placement {
            case_name: "boundary_exact".into(),
            note_width: 1000,
            note_height: 2000,
            binding_id: 0,
            source_page_index: 0,
            rect: [1.0, 0.0, 1001.0, 1000.0],
            source_box_host: None,
        },
        Placement {
            case_name: "boundary_just_over".into(),
            note_width: 1000,
            note_height: 2000,
            binding_id: 0,
            source_page_index: 0,
            rect: [1.0, 0.0, 1000.999, 1000.0],
            source_box_host: None,
        },
        Placement {
            case_name: "boundary_just_under".into(),
            note_width: 1000,
            note_height: 2000,
            binding_id: 0,
            source_page_index: 0,
            rect: [1.0, 0.0, 1001.001, 1000.0],
            source_box_host: None,
        },
    ]);
    assert_eq!(placements.len(), 26);
    assert_host_arena_ranges(placements.len());
    let captures = placements
        .iter()
        .enumerate()
        .map(|(index, placement)| execute_one(machine, placement, index))
        .collect::<Vec<_>>();
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"input_sources\":{{\"quiz_sdocx_sha256\":\"38fd0ef0729d3a113e1c14bcc10557dcc263e5a3582fd80a3cf99c8c2c4ad40a\",\"continuous_sdocx_sha256\":\"fa2d3ba44023871c6a53436e810772f7b4f45b190dd28e05c172886b8f7e40a0\",\"records\":\"frozen production-parser outputs embedded in this module\"}},\"source_box_metadata\":{{\"quiz_source_pdf_sha256\":\"f01243e2d712d47ad08735ec4e47da2a94974818f2cd7c28da2db3ee326c4279\",\"continuous_source_pdf_sha256\":\"1cd5ae14ddbcccc29ab7e0e8842aa2777d3c1d5c35463fcc7e6d0e7469afb689\",\"provenance\":\"MediaBox values from docs/reverse-engineering/rendering-corpus-findings.md; output metadata only, excluded from placement inputs\"}},\"composer_sha256\":\"52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f\",\"base_sha256\":\"e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb\",\"view_sha256\":\"c4a17e4232c2074d3833604974d75ac961fab4d9651949bb2552704c86621dc8\",\"native_addresses\":{{\"createPdf_caller_fragment\":\"Composer+0x3e6734\",\"View_SetPosition\":\"View+0x70b8c\",\"View_SetClipToBounds\":\"View+0x70df8\",\"View_Draw_clip_wrapper\":\"View+0x6fd7c\",\"RectF_Width\":\"Base+0xb108c\"}},\"capture_boundary\":\"Original ARM64 instructions from PageBackgroundView::createPdf execute from the WPage::GetWidth call through scaled-right/scaled-bottom computation and an indirect call to original View::SetPosition. WPage::GetWidth is supplied by a bounded host stub from the explicit retained note width. Source-page boxes are host-only metadata from the pinned PDF hashes and are not read by this fragment. Original View::SetClipToBounds and View::Draw clip wrapper execute; canvas Save/clipRect are minimal host callbacks. A trampoline replaces each post-capture continuation. No PDF open/render, native PDFView construction, page content, pixels, or source-page box transformation execute.\",\"placements\":[{}]}}",
        captures.join(",")
    );
}

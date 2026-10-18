use super::*;

const PDFIUM_BASE: u64 = 0x0100_0000;
const PDFIUM_SHA256: &str = "4bd55ef116541205cb8aaf04812a317fe11911c0ff5a5d44526d98d6b0e48854";
const PAGE: u64 = MODEL + 0x80000;
const X19: i32 = 218;
const S8: i32 = 144;
const S9: i32 = 145;
const S10: i32 = 146;
const S11: i32 = 147;
const ENTRY_AFTER_PAGE_GETTER: u64 = 0x58a064;
const FRAGMENT_END: u64 = 0x58a118;

fn read_s(engine: Engine, register_id: i32) -> f32 {
    let mut bits = 0_u32;
    check(unsafe { uc_reg_read(engine, register_id, ptr::from_mut(&mut bits).cast()) });
    f32::from_bits(bits)
}

pub(super) fn capture(machine: &mut Machine, pdfium: &Path) {
    map_library(machine.engine, pdfium, PDFIUM_BASE, PDFIUM_SHA256);
    let page_box = [13.25_f32, 29.5, 213.25, 179.5];
    let height = page_box[3] - page_box[1];
    let point = [17.25_f32, 41.5];
    println!("{{\"evidence\":\"executed bounded libSPenPdfiumB.so function-entry assembly fragment; CPDF_Page getter/page construction skipped; subsequent XObject/dictionary calls skipped\",\"pdfium_sha256\":{:?},\"native_symbol\":\"FPDFPage_NewFormObj+0x50..+0x104 (0x58a064..0x58a118)\",\"page_box_host_input\":{:?},\"host_input_top_left_point_for_composition\":{:?}", PDFIUM_SHA256, page_box, point);
    for (page_rotation, name) in [(0_u64, "page_r0"), (1, "page_r90"), (2, "page_r180"), (3, "page_r270")] {
        let form_argument = 4 - page_rotation;
        write(machine.engine, PAGE, &[0; 512]);
        for (i, value) in page_box.iter().enumerate() {
            write(machine.engine, PAGE + 88 + i as u64 * 4, &value.to_bits().to_le_bytes());
        }
        register(machine.engine, REGISTER_X0, PAGE);
        register(machine.engine, X19, form_argument);
        register(machine.engine, REGISTER_SP, STACK);
        let error = unsafe {
            uc_emu_start(
                machine.engine,
                PDFIUM_BASE + ENTRY_AFTER_PAGE_GETTER,
                PDFIUM_BASE + FRAGMENT_END,
                1_000_000,
                10_000,
            )
        };
        assert_eq!(error, 0, "native geometry fragment error {error}");
        assert_eq!(read_register(machine.engine, 260), PDFIUM_BASE + FRAGMENT_END, "fragment endpoint not reached");
        let sp = read_register(machine.engine, REGISTER_SP);
        let matrix = std::array::from_fn::<_, 4, _>(|i| read_float(machine.engine, sp + i as u64 * 4));
        let translation = [read_s(machine.engine, S8), read_s(machine.engine, S9)];
        let form_bbox_extent = [read_s(machine.engine, S10), read_s(machine.engine, S11)];
        let matrix_bits: Vec<_> = matrix.iter().map(|v| format!("0x{:08x}", v.to_bits())).collect();
        let translation_bits: Vec<_> = translation.iter().map(|v| format!("0x{:08x}", v.to_bits())).collect();
        let extent_bits: Vec<_> = form_bbox_extent.iter().map(|v| format!("0x{:08x}", v.to_bits())).collect();
        // The handler's FPDF_GetPageHeight dispatches to CPDF_Page::GetPageHeight, which returns
        // the raw effective-box height. This point/matrix composition is still host-side arithmetic.
        let pdfium_bottom_left = [point[0], height - point[1]];
        let raw = [
            matrix[0] * pdfium_bottom_left[0] + matrix[2] * pdfium_bottom_left[1] + translation[0],
            matrix[1] * pdfium_bottom_left[0] + matrix[3] * pdfium_bottom_left[1] + translation[1],
        ];
        println!(",\"{}\":{{\"page_rotation_quarter_turn\":{},\"open_writer_form_argument_4_minus_page_rotation\":{},\"native_form_turn_after_modulo_4\":{},\"matrix_abcd\":{:?},\"matrix_abcd_f32_bits\":{:?},\"matrix_ef\":{:?},\"matrix_ef_f32_bits\":{:?},\"form_bbox_extent\":{:?},\"form_bbox_extent_f32_bits\":{:?},\"host_composed_handler_input_after_H_minus_y\":{:?},\"host_composed_raw_page_point_using_raw_box_height\":{:?},\"host_composed_point_basis_boundary\":\"The handler's page-height call and H-y instruction are established by disassembly, but this point is host-supplied and the composition is host arithmetic, not a full writer capture.\"}}", name, page_rotation, form_argument, form_argument % 4, matrix, matrix_bits, translation, translation_bits, form_bbox_extent, extent_bits, pdfium_bottom_left, raw);
    }
    println!("}}");
}

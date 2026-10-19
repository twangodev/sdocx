use super::*;

const C: u64 = 0x0100_0000;
const B: u64 = 0x0500_0000;
const STUB: u64 = 0x0800_0000;
const DATA: u64 = 0x0900_0000;
const DATA_SIZE: u64 = 0x1_0000;
const SELF: u64 = DATA;
const NOTE: u64 = DATA + 0x1000;
const DOC: u64 = DATA + 0x3000;
const DOC_VT: u64 = DATA + 0x4000;
const DRAW: u64 = DATA + 0x5000;
const DRAW_VT: u64 = DATA + 0x5100;
const INFO_ARRAY: u64 = DATA + 0x6000;
const INFO: u64 = DATA + 0x6100;
const SOURCE_INDEX: u64 = DATA + 0x6400;
const PATH: u64 = DATA + 0x6700;
const TAG: u64 = DATA + 0x6600;
const CONTROL_INFO: u64 = DATA + 0x7000;
const CONTROL_PATH: u64 = DATA + 0x7800;
const STRING_LEN: u64 = STUB + 0x280;
const EMPTY: u64 = STUB;
const COEDIT: u64 = STUB + 0x1d0;
const GET_PAGE: u64 = STUB + 0x20;
const WIDTH: u64 = STUB + 0x40;
const HEIGHT: u64 = STUB + 0x60;
const PAGE_RECT: u64 = STUB + 0x80;
const OPEN_WRITER: u64 = STUB + 0xa0;
const PDF_PAGE: u64 = STUB + 0xc0;
const DRAW_PDF: u64 = STUB + 0xe0;
const ADD_PAGES: u64 = STUB + 0x100;
const REMOVE_IMAGE: u64 = STUB + 0x120;
const REMOVE_PDF: u64 = STUB + 0x140;
const COUNT: u64 = STUB + 0x160;
const STRING_SET: u64 = STUB + 0x180;
const OBJECTS: u64 = STUB + 0x1a0;
const PDF_TAG: u64 = STUB + 0x1c0;
const NOOP: u64 = STUB + 0x1e0;
const FACTORY: u64 = STUB + 0x200;
const BODY: u64 = STUB + 0x220;
const DTOR: u64 = STUB + 0x240;

struct State {
    string_sets: usize,
    record_count_calls: usize,
    draw: usize,
    pdf_page: usize,
    opens: usize,
    args: Vec<[u64; 5]>,
    update_args: Vec<[u64; 4]>,
    removed_images: usize,
    removed_pdfs: usize,
    width: u32,
    height: u32,
    rects: Vec<[f32; 4]>,
}
unsafe extern "C" fn cb(e: Engine, a: u64, _: u32, p: *mut c_void) {
    let s = unsafe { &mut *p.cast::<State>() };
    match a {
        EMPTY => register(e, REGISTER_X0, 1),
        COEDIT => register(e, REGISTER_X0, 0),
        NOOP | BODY | DTOR => register(e, REGISTER_X0, 0),
        FACTORY => register(e, REGISTER_X0, DATA + 0x5000),
        GET_PAGE => register(e, REGISTER_X0, DATA + 0x2000),
        WIDTH => register(e, REGISTER_X0, s.width as u64),
        HEIGHT => register(e, REGISTER_X0, s.height as u64),
        PAGE_RECT => {
            for (i, v) in [0.0_f32, 0.0, 726.2043, 595.44006].iter().enumerate() {
                register(e, 136 + i as i32, u64::from(v.to_bits()));
            }
            register(e, REGISTER_X0, 0);
        }
        OPEN_WRITER => {
            s.opens += 1;
            register(e, REGISTER_X0, 0);
        }
        PDF_PAGE => {
            s.pdf_page += 1;
            register(e, REGISTER_X0, INFO);
        }
        DRAW_PDF => {
            let r = read_register(e, REGISTER_X0 + 2);
            s.rects
                .push(std::array::from_fn(|i| read_float(e, r + i as u64 * 4)));
            s.draw += 1;
            register(e, REGISTER_X0, 1);
        }
        ADD_PAGES => {
            s.args.push(std::array::from_fn(|i| {
                read_register(e, REGISTER_X0 + i as i32)
            }));
            register(e, REGISTER_X0, 1);
        }
        REMOVE_IMAGE => {
            s.removed_images += 1;
            register(e, REGISTER_X0, 1);
        }
        REMOVE_PDF => {
            s.removed_pdfs += 1;
            register(e, REGISTER_X0, 1);
        }
        STRING_LEN => {
            let address = read_register(e, REGISTER_X0);
            let mut length = 0;
            while read_u64(e, address + length) as u8 != 0 {
                length += 1;
                assert!(length < 4096, "unterminated native String input");
            }
            register(e, REGISTER_X0, length as u64);
        }
        COUNT => {
            s.record_count_calls += 1;
            register(e, REGISTER_X0, 1);
        }
        STRING_SET => {
            s.string_sets += 1;
            register(e, REGISTER_X0, 1);
        }
        OBJECTS => register(e, REGISTER_X0, 0),
        PDF_TAG => register(e, REGISTER_X0, TAG),
        _ => panic!("unexpected Composer host call {a:#x}"),
    }
}
unsafe extern "C" fn capture_update(e: Engine, _: u64, _: u32, p: *mut c_void) {
    let s = unsafe { &mut *p.cast::<State>() };
    s.update_args.push(std::array::from_fn(|i| {
        read_register(e, REGISTER_X0 + i as i32)
    }));
}
fn hook(m: &Machine, a: u64, p: *mut c_void) {
    write(m.engine, a, &0xd65f03c0_u32.to_le_bytes());
    let mut id = 0;
    check(unsafe { uc_hook_add(m.engine, &mut id, 4, cb as *mut c_void, p, a, a) });
}
pub(super) fn capture(m: &mut Machine, composer: &Path, base: &Path, source: &Path) {
    map_library(
        m.engine,
        base,
        B,
        "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
    );
    assert!(DATA >= 0x0900_0000 && DATA + DATA_SIZE <= 0x0901_0000);
    check(unsafe { uc_mem_map(m.engine, DATA, DATA_SIZE, 7) });
    map_library(
        m.engine,
        composer,
        C,
        "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f",
    );
    check(unsafe { uc_mem_map(m.engine, STUB, 0x1000, 7) });
    let mut s = Box::new(State {
        string_sets: 0,
        record_count_calls: 0,
        draw: 0,
        pdf_page: 0,
        opens: 0,
        args: Vec::new(),
        update_args: Vec::new(),
        removed_images: 0,
        removed_pdfs: 0,
        width: 1812,
        height: 1485,
        rects: Vec::new(),
    });
    let p = ptr::from_mut(s.as_mut()).cast();
    for a in [
        EMPTY,
        COEDIT,
        GET_PAGE,
        WIDTH,
        HEIGHT,
        PAGE_RECT,
        OPEN_WRITER,
        PDF_PAGE,
        DRAW_PDF,
        ADD_PAGES,
        REMOVE_IMAGE,
        REMOVE_PDF,
        COUNT,
        STRING_SET,
        OBJECTS,
        PDF_TAG,
        NOOP,
        FACTORY,
        BODY,
        DTOR,
        STRING_LEN,
    ] {
        hook(m, a, p);
    }
    for (plt, target) in [
        (C + 0x5514c0, EMPTY),
        (C + 0x54f180, COEDIT),
        (C + 0x54f640, GET_PAGE),
        (C + 0x54eeb0, WIDTH),
        (C + 0x54eec0, HEIGHT),
        (C + 0x54eda0, FACTORY),
        (C + 0x551520, OPEN_WRITER),
        (C + 0x551830, BODY),
        (C + 0x551840, DTOR),
        (C + 0x551740, C + 0x35b1e0),
        (C + 0x551750, C + 0x35b35c),
        (C + 0x551760, C + 0x35b4f4),
        (C + 0x551820, PDF_TAG),
        (C + 0x54f2f0, OBJECTS),
        (C + 0x54f290, COUNT),
        (C + 0x54fcd0, STRING_SET),
        (C + 0x54fd40, B + 0xc3ed4),
        (C + 0x54db30, B + 0xb108c),
        (C + 0x54f390, B + 0xb1510),
        (C + 0x54d850, NEW),
        (C + 0x54d870, DELETE),
        (B + 0xe55f0, NEW),
        (B + 0xe56f0, NEW),
        (B + 0xe5650, DELETE),
        (B + 0xe6cc0, B + 0xc1e5c),
        (B + 0xe6130, STRING_LEN),
    ] {
        bind_native(m.engine, plt, target);
    }
    write(m.engine, SELF, &[0; 512]);
    write(m.engine, SELF + 80, &NOTE.to_le_bytes());
    write(m.engine, SELF + 184, &DOC.to_le_bytes());
    write(m.engine, SELF + 192, &DRAW.to_le_bytes());
    write(m.engine, DOC, &DOC_VT.to_le_bytes());
    write(m.engine, DRAW, &DRAW_VT.to_le_bytes());
    write(m.engine, DOC_VT + 104, &NOOP.to_le_bytes());
    write(m.engine, DOC_VT + 88, &ADD_PAGES.to_le_bytes());
    write(m.engine, DOC_VT + 112, &NOOP.to_le_bytes());
    write(m.engine, DOC_VT + 152, &REMOVE_IMAGE.to_le_bytes());
    write(m.engine, DOC_VT + 56, &REMOVE_PDF.to_le_bytes());
    write(m.engine, DRAW_VT + 48, &PAGE_RECT.to_le_bytes());
    write(m.engine, DRAW_VT + 56, &EMPTY.to_le_bytes());
    write(m.engine, DRAW_VT + 64, &EMPTY.to_le_bytes());
    write(m.engine, DRAW_VT + 72, &EMPTY.to_le_bytes());
    write(m.engine, DRAW_VT + 80, &EMPTY.to_le_bytes());
    write(m.engine, DRAW_VT + 88, &PDF_PAGE.to_le_bytes());
    write(m.engine, DRAW_VT + 96, &NOOP.to_le_bytes());
    write(m.engine, DRAW_VT + 168, &DRAW_PDF.to_le_bytes());
    let path = source.to_string_lossy();
    write(m.engine, PATH, path.as_bytes());
    write(m.engine, PATH + path.len() as u64, &[0]);
    m.call(B + 0xc2a18, &[INFO + 16]);
    assert_ne!(m.call(B + 0xc3120, &[INFO + 16, PATH]), 0);
    m.call(B + 0xc2a18, &[INFO + 48]);
    m.call(B + 0xc2a18, &[CONTROL_INFO + 16]);
    write(m.engine, INFO_ARRAY, &INFO.to_le_bytes());
    write(m.engine, SELF + 8, &TAG.to_le_bytes());
    write(m.engine, TAG, b"sdocx-page-background\0");
    write(m.engine, SELF + 128, &0_u64.to_le_bytes());
    write(m.engine, SELF + 207, &[0]);
    write(m.engine, SELF + 212, &(-1_i32).to_le_bytes());
    write(m.engine, SOURCE_INDEX, &0_i32.to_le_bytes());
    write(m.engine, SOURCE_INDEX + 4, &0_i32.to_le_bytes());
    write(m.engine, SELF + 256, &SOURCE_INDEX.to_le_bytes());
    write(m.engine, SELF + 280, &INFO_ARRAY.to_le_bytes());
    write(m.engine, SELF + 288, &INFO_ARRAY.to_le_bytes());
    bind_native(m.engine, C + 0x551730, C + 0x35af00);
    let mut update_hook = 0;
    check(unsafe {
        uc_hook_add(
            m.engine,
            &mut update_hook,
            4,
            capture_update as *mut c_void,
            ptr::from_mut(s.as_mut()).cast(),
            C + 0x35af00,
            C + 0x35af00,
        )
    });
    let raw = m.call(C + 0x35a9c4, &[SELF, 0]);
    assert_eq!(s.string_sets, 0);
    let record_count_calls = s.record_count_calls;
    assert!(
        record_count_calls > 0,
        "native captureBackground never queried the retained PDF-record count"
    );
    let result = m.call(C + 0x35ab74, &[SELF, 0, 1]);
    assert_eq!(result, 0);
    assert_eq!(s.args.len(), 1);
    assert_eq!(s.args[0][1], INFO + 16);
    assert_eq!((s.args[0][2], s.args[0][3], s.args[0][4]), (0, 0, 0));
    assert_eq!((s.draw, s.pdf_page, s.opens), (0, 0, 1));
    assert_eq!((s.removed_images, s.removed_pdfs), (1, 1));
    assert_eq!(
        s.update_args,
        vec![[SELF, 0, 0, 1]],
        "Standard exportPDFPages updatePage flags changed"
    );
    let mut control_rects = Vec::new();
    let manual_path = b"manual-paper.pdf\0";
    write(m.engine, CONTROL_PATH, manual_path);
    assert_ne!(m.call(B + 0xc3120, &[CONTROL_INFO + 16, CONTROL_PATH]), 0);
    m.call(B + 0xc2a18, &[CONTROL_INFO + 48]);
    assert_ne!(m.call(B + 0xc3120, &[CONTROL_INFO + 48, CONTROL_PATH]), 0);
    for height in [1485, 15372] {
        s.height = height;
        write(m.engine, INFO_ARRAY, &CONTROL_INFO.to_le_bytes());
        let _ = m.call(C + 0x35af00, &[SELF, 0, 0, 1]);
        control_rects.push(
            *s.rects
                .last()
                .expect("manual background DrawPDF not reached"),
        );
    }
    assert_eq!(s.rects.len(), 2);
    assert_eq!(s.pdf_page, 2);
    assert_eq!(s.draw, 2);
    assert_eq!(s.opens, 3);
    assert_eq!(control_rects[0], [0.0, 0.0, 726.2043, 595.1509]);
    assert_eq!(control_rects[1], [0.0, 0.0, 726.2043, 6160.713]);
    println!(
        "{{\"native_entry\":\"Composer captureBackground→exportPDFPages→updatePage\",\"source_pdf_provenance\":\"quiz page 0; SHA-256 f01243e2d712d47ad08735ec4e47da2a94974818f2cd7c28da2db3ee326c4279\",\"source_pdf_host_path\":\"quiz.pdf (host path input)\",\"retained_pdf_record\":{{\"source_page\":0,\"note_rect\":[0,0,1812,1485]}},\"note_canvas_host_input\":[1812,15372],\"capture_pdf_record_count_calls\":{},\"capture_string_set_calls\":{},\"capture_return_register_uninterpreted\":{},\"add_pages_bridge_args\":[[\"composer_instance\",\"retained_record.source_path\",null,0,0]],\"add_pages_bridge_result\":1,\"native_updatepage_args\":[[\"composer_instance\",0,false,true]],\"imported_paper_branch\":{{\"writer_opens\":1,\"drawpdf_calls\":0,\"pdf_resource_getter_calls\":0,\"remove_overlay_image_calls\":1,\"remove_pdf_overlay_calls\":1}},\"manual_background_controls\":{{\"note_extents\":[[1812,1485],[1812,15372]],\"source_page_rect_host_input\":[0,0,726.2043,595.44006],\"drawpdf_rects\":{:?}}},\"host_boundaries\":[\"PDF record list/count and AddPages bridge (bridge returns separately observed result; PDFium not run in this mode)\",\"WPage width/height, adapter lifecycle, source page rectangle\",\"DrawPDF sink; manual controls do not build an output PDF\"],\"boundary\":\"Composer captureBackground, exportPDFPages, updatePage and imported-paper empty-path skip execute. Two direct updatePage controls use host-supplied manual background path, note extent and source page rectangle. Native Base String constructors build the path inputs; the PDF writer is opened, but this mode does not join or serialize its output.\"}}",
        record_count_calls, s.string_sets, raw, control_rects
    );
}

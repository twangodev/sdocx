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
const STUB: u64 = 0x0800_0000;
const ARRAY_OBJECT: u64 = MODEL + 0x81000;
const NUMBER_OBJECT: u64 = MODEL + 0x81100;
const ARRAY_VTABLE: u64 = MODEL + 0x81200;
const NUMBER_VTABLE: u64 = MODEL + 0x81300;
const CONSTRUCTOR: u64 = STUB;
const GET_PAGE_ATTR: u64 = STUB + 0x20;
const GET_ARRAY: u64 = STUB + 0x40;
const GET_INTEGER: u64 = STUB + 0x60;
const GET_RECT: u64 = STUB + 0x80;

struct AttrState {
    key: String,
    rect: [f32; 4],
    rotate: i32,
}

unsafe fn read_bytes(engine: Engine, address: u64, length: usize) -> Vec<u8> {
    let mut bytes = vec![0; length];
    check(unsafe { uc_mem_read(engine, address, bytes.as_mut_ptr().cast(), length) });
    bytes
}
unsafe extern "C" fn ctor(engine: Engine, address: u64, _: u32, _: *mut c_void) {
    assert_eq!(address, CONSTRUCTOR);
    let destination = read_register(engine, REGISTER_X0);
    let source = read_register(engine, REGISTER_X0 + 1);
    let length = read_register(engine, REGISTER_X0 + 2) as usize;
    let key = unsafe { read_bytes(engine, source, length) };
    write(engine, MODEL + 0x81400, &key);
    write(engine, MODEL + 0x81420, &(key.len() as u64).to_le_bytes());
    write(engine, destination, &[0; 16]);
}
unsafe extern "C" fn get_attr(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    assert_eq!(address, GET_PAGE_ATTR);
    let state = unsafe { &mut *data.cast::<AttrState>() };
    let key_ptr = MODEL + 0x81400;
    let len = read_u64(engine, MODEL + 0x81420) as usize;
    state.key = String::from_utf8(unsafe { read_bytes(engine, key_ptr, len) }).unwrap();
    let object = match state.key.as_str() {
        "MediaBox" => {
            state.rect = [0.0, 0.0, 300.0, 240.0];
            ARRAY_OBJECT
        }
        "CropBox" => {
            state.rect = [13.25, 29.5, 213.25, 179.5];
            ARRAY_OBJECT
        }
        "Rotate" => NUMBER_OBJECT,
        other => panic!("unexpected page attr {other:?}"),
    };
    let out = read_register(engine, REGISTER_X0 + 8);
    write(engine, out, &object.to_le_bytes());
    write(engine, out + 8, &0_u64.to_le_bytes());
}
unsafe extern "C" fn get_array(engine: Engine, address: u64, _: u32, _: *mut c_void) {
    assert_eq!(address, GET_ARRAY);
    register(engine, REGISTER_X0, ARRAY_OBJECT);
}
unsafe extern "C" fn get_integer(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    assert_eq!(address, GET_INTEGER);
    register(engine, REGISTER_X0, unsafe {
        (*data.cast::<AttrState>()).rotate as u32 as u64
    });
}
unsafe extern "C" fn get_rect(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    assert_eq!(address, GET_RECT);
    for (i, value) in unsafe { &(*data.cast::<AttrState>()).rect }
        .iter()
        .enumerate()
    {
        register(engine, 136 + i as i32, u64::from(value.to_bits()));
    }
}
fn install_dimensions(machine: &Machine, state: &mut Box<AttrState>) {
    check(unsafe { uc_mem_map(machine.engine, STUB, 0x1000, 7) });
    write(machine.engine, ARRAY_OBJECT, &ARRAY_VTABLE.to_le_bytes());
    write(machine.engine, ARRAY_OBJECT + 8, &10_u64.to_le_bytes());
    write(machine.engine, NUMBER_OBJECT, &NUMBER_VTABLE.to_le_bytes());
    write(machine.engine, NUMBER_OBJECT + 8, &10_u64.to_le_bytes());
    write(
        machine.engine,
        ARRAY_VTABLE + 72,
        &(STUB + 0x40).to_le_bytes(),
    );
    write(
        machine.engine,
        NUMBER_VTABLE + 56,
        &(STUB + 0x60).to_le_bytes(),
    );
    let data = ptr::from_mut(state.as_mut()).cast();
    for (address, callback) in [
        (
            CONSTRUCTOR,
            ctor as unsafe extern "C" fn(Engine, u64, u32, *mut c_void),
        ),
        (
            GET_PAGE_ATTR,
            get_attr as unsafe extern "C" fn(Engine, u64, u32, *mut c_void),
        ),
        (
            GET_ARRAY,
            get_array as unsafe extern "C" fn(Engine, u64, u32, *mut c_void),
        ),
        (
            GET_INTEGER,
            get_integer as unsafe extern "C" fn(Engine, u64, u32, *mut c_void),
        ),
        (
            GET_RECT,
            get_rect as unsafe extern "C" fn(Engine, u64, u32, *mut c_void),
        ),
    ] {
        write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
        let mut id = 0;
        check(unsafe {
            uc_hook_add(
                machine.engine,
                &mut id,
                4,
                callback as *mut c_void,
                data,
                address,
                address,
            )
        });
    }
    for (plt, target) in [
        (PDFIUM_BASE + 0x639810, CONSTRUCTOR),
        (PDFIUM_BASE + 0x636840, GET_PAGE_ATTR),
        (PDFIUM_BASE + 0x636850, PDFIUM_BASE + 0x4732ac),
        (PDFIUM_BASE + 0x636870, PDFIUM_BASE + 0x473664),
        (PDFIUM_BASE + 0x634550, GET_RECT),
        (PDFIUM_BASE + 0x635b50, PDFIUM_BASE + 0x4fd4a8),
    ] {
        bind_native(machine.engine, plt, target);
    }
}

fn read_s(engine: Engine, register_id: i32) -> f32 {
    let mut bits = 0_u32;
    check(unsafe { uc_reg_read(engine, register_id, ptr::from_mut(&mut bits).cast()) });
    f32::from_bits(bits)
}

pub(super) fn capture(machine: &mut Machine, pdfium: &Path) {
    map_library(machine.engine, pdfium, PDFIUM_BASE, PDFIUM_SHA256);
    let page_box = [13.25_f32, 29.5, 213.25, 179.5];
    let mut attrs = Box::new(AttrState {
        key: String::new(),
        rect: [0.0; 4],
        rotate: 0,
    });
    install_dimensions(machine, &mut attrs);
    let point = [17.25_f32, 41.5];
    println!(
        "{{\"evidence\":\"actual CPDF_Page::UpdateDimensions and GetPageWidth/GetPageHeight precede bounded NewFormObj geometry fragment; dictionary values remain explicit host inputs; later XObject construction skipped\",\"pdfium_sha256\":{:?},\"native_symbol\":\"CPDF_Page::UpdateDimensions+0x472c10 then GetPageWidth+0x472f54/GetPageHeight+0x472f5c; FPDFPage_NewFormObj+0x50..+0x104\",\"page_box_host_input\":{:?},\"host_input_top_left_point_for_composition\":{:?}",
        PDFIUM_SHA256, page_box, point
    );
    for (page_rotation, name) in [
        (0_u64, "page_r0"),
        (1, "page_r90"),
        (2, "page_r180"),
        (3, "page_r270"),
    ] {
        let form_argument = 4 - page_rotation;
        write(machine.engine, PAGE, &[0; 512]);
        attrs.rotate = page_rotation as i32 * 90;
        machine.call(PDFIUM_BASE + 0x472c10, &[PAGE]);
        let effective_box: [f32; 4] =
            std::array::from_fn(|i| read_float(machine.engine, PAGE + 88 + i as u64 * 4));
        assert_eq!(effective_box, page_box);
        machine.call(PDFIUM_BASE + 0x472f54, &[PAGE]);
        let native_width = read_s(machine.engine, 136);
        machine.call(PDFIUM_BASE + 0x472f5c, &[PAGE]);
        let native_height = read_s(machine.engine, 136);
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
        assert_eq!(
            read_register(machine.engine, 260),
            PDFIUM_BASE + FRAGMENT_END,
            "fragment endpoint not reached"
        );
        let sp = read_register(machine.engine, REGISTER_SP);
        let matrix =
            std::array::from_fn::<_, 4, _>(|i| read_float(machine.engine, sp + i as u64 * 4));
        let translation = [read_s(machine.engine, S8), read_s(machine.engine, S9)];
        let form_bbox_extent = [read_s(machine.engine, S10), read_s(machine.engine, S11)];
        let matrix_bits: Vec<_> = matrix
            .iter()
            .map(|v| format!("0x{:08x}", v.to_bits()))
            .collect();
        let translation_bits: Vec<_> = translation
            .iter()
            .map(|v| format!("0x{:08x}", v.to_bits()))
            .collect();
        let extent_bits: Vec<_> = form_bbox_extent
            .iter()
            .map(|v| format!("0x{:08x}", v.to_bits()))
            .collect();
        // The handler's FPDF_GetPageHeight dispatches to CPDF_Page::GetPageHeight; this composition remains host arithmetic.
        let pdfium_bottom_left = [point[0], native_height - point[1]];
        let raw = [
            matrix[0] * pdfium_bottom_left[0] + matrix[2] * pdfium_bottom_left[1] + translation[0],
            matrix[1] * pdfium_bottom_left[0] + matrix[3] * pdfium_bottom_left[1] + translation[1],
        ];
        println!(
            ",\"{}\":{{\"page_rotation_quarter_turn\":{},\"open_writer_form_argument_4_minus_page_rotation\":{},\"native_form_turn_after_modulo_4\":{},\"native_page_dimensions_from_getters\":[{},{}],\"matrix_abcd\":{:?},\"matrix_abcd_f32_bits\":{:?},\"matrix_ef\":{:?},\"matrix_ef_f32_bits\":{:?},\"form_bbox_extent\":{:?},\"form_bbox_extent_f32_bits\":{:?},\"host_composed_handler_input_after_H_minus_y\":{:?},\"host_composed_raw_page_point_using_native_page_height\":{:?},\"host_composed_point_basis_boundary\":\"UpdateDimensions and actual page-dimension getters execute; the source box and point are host inputs, and matrix/point composition remains host arithmetic, not a full writer capture.\"}}",
            name,
            page_rotation,
            form_argument,
            form_argument % 4,
            native_width,
            native_height,
            matrix,
            matrix_bits,
            translation,
            translation_bits,
            form_bbox_extent,
            extent_bits,
            pdfium_bottom_left,
            raw
        );
    }
    println!("}}");
}

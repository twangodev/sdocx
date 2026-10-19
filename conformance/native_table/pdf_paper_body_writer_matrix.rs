use super::*;

const COMPOSER: u64 = 0x0100_0000;
const BASE: u64 = 0x0500_0000;
const DATA: u64 = 0x0900_0000;
const DATA_SIZE: u64 = 0x1_0000;
const STUB: u64 = 0x0800_0000;
const SELF: u64 = DATA + 0x1000;
const WNOTE: u64 = DATA + 0x2000;
const WPAGE: u64 = DATA + 0x3000;
const DRAWING: u64 = DATA + 0x4000;
const DRAW_VTABLE: u64 = DATA + 0x5000;
const BODY: u64 = DATA + 0x6000;
const TEXT: u64 = DATA + 0x7000;
const OBJECT_VECTOR: u64 = DATA + 0x8000;
const DRAWN_RECORD: u64 = DATA + 0x9000;
const DRAW_OBJECT: u64 = DATA + 0xa000;
const PDF_WRITER: u64 = DATA + 0xb000;
const PAGE_RECT_FN: u64 = STUB;
const IS_EMPTY: u64 = STUB + 0x20;
const GET_PAGE: u64 = STUB + 0x40;
const NOTE_WIDTH: u64 = STUB + 0x60;
const NOTE_HEIGHT: u64 = STUB + 0x80;
const DRAWING_FACTORY: u64 = STUB + 0xa0;
const BODY_TEXT_GET: u64 = STUB + 0xc0;
const COMPONENT_TEXT_GET: u64 = STUB + 0xe0;
const CAPTURE_CTOR: u64 = STUB + 0x100;
const CAPTURE_SET_DOC: u64 = STUB + 0x120;
const CAPTURE_SYSTEM_FONT: u64 = STUB + 0x140;
const CAPTURE_GET_DATA: u64 = STUB + 0x160;
const CAPTURE_DTOR: u64 = STUB + 0x180;
const LOAD_BITMAP_FONT: u64 = STUB + 0x1a0;
const DRAWING_GET: u64 = STUB + 0x1c0;
const DRAWING_FLUSH: u64 = STUB + 0x1e0;
const DRAWING_DRAW_OBJECT: u64 = STUB + 0x200;
const DESTRUCTOR: u64 = STUB + 0x220;
const PAGE_MODE: u64 = STUB + 0x240;
const IS_COEDIT: u64 = STUB + 0x260;
const DRAWN_DATA_DTOR: u64 = STUB + 0x280;
const WRITE_BODY_TEXT_ENTRY: u64 = COMPOSER + 0x375dc8;

#[derive(Default)]
struct State {
    width: u32,
    height: u32,
    page_rect: [f32; 4],
    get_data_calls: usize,
    draw_calls: Vec<(u64, [f32; 4])>,
    flush_calls: usize,
    host_callbacks: Vec<String>,
    native_write_body_text_scale_bits: Option<u32>,
}

unsafe extern "C" fn callback(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let state = unsafe { &mut *data.cast::<State>() };
    match address {
        PAGE_RECT_FN => {
            for (i, value) in state.page_rect.iter().enumerate() {
                register(engine, 136 + i as i32, value.to_bits() as u64);
            }
            register(engine, REGISTER_X0, 0);
        }
        IS_EMPTY => register(engine, REGISTER_X0, 0),
        GET_PAGE => register(engine, REGISTER_X0, WPAGE),
        NOTE_WIDTH => register(engine, REGISTER_X0, state.width as u64),
        NOTE_HEIGHT => register(engine, REGISTER_X0, state.height as u64),
        DRAWING_FACTORY => register(engine, REGISTER_X0, DRAWING),
        BODY_TEXT_GET => register(engine, REGISTER_X0, BODY),
        COMPONENT_TEXT_GET => register(engine, REGISTER_X0, TEXT),
        CAPTURE_CTOR | CAPTURE_SET_DOC | CAPTURE_SYSTEM_FONT | CAPTURE_DTOR | LOAD_BITMAP_FONT
        | DRAWN_DATA_DTOR => {
            state.host_callbacks.push(format!("{address:#x}"));
            register(engine, REGISTER_X0, 0);
        }
        CAPTURE_GET_DATA => {
            state.get_data_calls += 1;
            let drawn_data = read_register(engine, REGISTER_X0 + 3);
            assert_eq!(read_register(engine, REGISTER_X0 + 1), 0);
            // DrawnTextData contains three std::vector triples. The synthetic
            // image/object record is explicit input; native WriteBodyText reads
            // it and performs the translate/scale before virtual DrawObject.
            write(engine, drawn_data, &[0; 72]);
            write(engine, drawn_data, &OBJECT_VECTOR.to_le_bytes());
            write(engine, drawn_data + 8, &(OBJECT_VECTOR + 8).to_le_bytes());
            write(engine, drawn_data + 16, &(OBJECT_VECTOR + 8).to_le_bytes());
            write(engine, OBJECT_VECTOR, &DRAWN_RECORD.to_le_bytes());
            write(engine, OBJECT_VECTOR + 8, &[0; 0x18]);
            register(engine, REGISTER_X0, 1);
        }
        DRAWING_GET => register(engine, REGISTER_X0, TEXT),
        PAGE_MODE => register(engine, REGISTER_X0, 1),
        IS_COEDIT => register(engine, REGISTER_X0, 0),
        DRAWING_FLUSH => {
            state.flush_calls += 1;
            register(engine, REGISTER_X0, 0);
        }
        DRAWING_DRAW_OBJECT => {
            let object = read_register(engine, REGISTER_X0 + 1);
            let rect = read_register(engine, REGISTER_X0 + 2);
            let values =
                std::array::from_fn(|i| f32::from_bits(read_u32(engine, rect + i as u64 * 4)));
            state.draw_calls.push((object, values));
            register(engine, REGISTER_X0, 1);
        }
        DESTRUCTOR => register(engine, REGISTER_X0, 0),
        _ => panic!("unexpected BodyTextPDFWriter host callback {address:#x}"),
    }
}

unsafe extern "C" fn observe_write_body_text(
    engine: Engine,
    _: u64,
    _: u32,
    data: *mut c_void,
) {
    let state = unsafe { &mut *data.cast::<State>() };
    assert!(
        state.native_write_body_text_scale_bits.is_none(),
        "native BodyTextPDFWriter::WriteBodyText entry ran more than once"
    );
    // The caller passes BodyTextPDFWriter as x0, pageIndex in x1,
    // and null in x2. The native body-writer reads its scale field at x0+8.
    // This code hook observes the real entry without replacing or modifying
    // the native function.
    let writer_rect = read_register(engine, REGISTER_X0);
    assert_ne!(writer_rect, 0, "native WriteBodyText received a null object");
    state.native_write_body_text_scale_bits = Some(read_u32(engine, writer_rect + 8));
}

fn hook(machine: &Machine, address: u64, state: *mut c_void) {
    write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
    let mut hook = 0;
    check(unsafe {
        uc_hook_add(
            machine.engine,
            &mut hook,
            4,
            callback as *mut c_void,
            state,
            address,
            address,
        )
    });
}

pub(super) fn transform(
    model: &Path,
    composer: &Path,
    base: &Path,
    source_page_rect: [f32; 4],
    note_width: u32,
    note_height: u32,
) -> ([f32; 4], f32) {
    let machine = Machine::new(model);
    map_library(
        machine.engine,
        base,
        BASE,
        "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
    );
    map_library(
        machine.engine,
        composer,
        COMPOSER,
        "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f",
    );
    assert!(DATA + DATA_SIZE <= 0x0901_0000);
    check(unsafe { uc_mem_map(machine.engine, DATA, DATA_SIZE, 7) });
    check(unsafe { uc_mem_map(machine.engine, STUB, 0x1000, 7) });
    let mut state = Box::new(State {
        width: note_width,
        height: note_height,
        page_rect: source_page_rect,
        ..State::default()
    });
    let state_ptr = ptr::from_mut(state.as_mut()).cast();
    for address in [
        PAGE_RECT_FN,
        IS_EMPTY,
        GET_PAGE,
        NOTE_WIDTH,
        NOTE_HEIGHT,
        DRAWING_FACTORY,
        BODY_TEXT_GET,
        COMPONENT_TEXT_GET,
        CAPTURE_CTOR,
        CAPTURE_SET_DOC,
        CAPTURE_SYSTEM_FONT,
        CAPTURE_GET_DATA,
        CAPTURE_DTOR,
        LOAD_BITMAP_FONT,
        DRAWING_GET,
        DRAWING_FLUSH,
        DRAWING_DRAW_OBJECT,
        DESTRUCTOR,
        PAGE_MODE,
        IS_COEDIT,
        DRAWN_DATA_DTOR,
    ] {
        hook(&machine, address, state_ptr);
    }
    let mut observer = 0;
    check(unsafe {
        uc_hook_add(
            machine.engine,
            &mut observer,
            4,
            observe_write_body_text as *mut c_void,
            state_ptr,
            WRITE_BODY_TEXT_ENTRY,
            WRITE_BODY_TEXT_ENTRY,
        )
    });
    for (plt, target) in [
        (COMPOSER + 0x5514c0, IS_EMPTY),
        (COMPOSER + 0x54f640, GET_PAGE),
        (COMPOSER + 0x54f180, IS_COEDIT),
        (COMPOSER + 0x54eeb0, NOTE_WIDTH),
        (COMPOSER + 0x54eec0, NOTE_HEIGHT),
        (COMPOSER + 0x54eda0, DRAWING_FACTORY),
        (COMPOSER + 0x551830, COMPOSER + 0x375dc8),
        (COMPOSER + 0x551840, DESTRUCTOR),
        (COMPOSER + 0x54db30, BASE + 0xb108c),
        (COMPOSER + 0x54db40, BASE + 0xb10a0),
        (COMPOSER + 0x54f2e0, BASE + 0xb11a4),
        (COMPOSER + 0x54f390, BASE + 0xb1510),
        (COMPOSER + 0x550730, BODY_TEXT_GET),
        (COMPOSER + 0x550db0, COMPONENT_TEXT_GET),
        (COMPOSER + 0x54ee30, CAPTURE_CTOR),
        (COMPOSER + 0x54ee40, CAPTURE_SET_DOC),
        (COMPOSER + 0x54f070, CAPTURE_SYSTEM_FONT),
        (COMPOSER + 0x550740, CAPTURE_GET_DATA),
        (COMPOSER + 0x550760, DRAWN_DATA_DTOR),
        (COMPOSER + 0x54eea0, CAPTURE_DTOR),
        (COMPOSER + 0x552eb0, LOAD_BITMAP_FONT),
        (COMPOSER + 0x54fac0, PAGE_MODE),
    ] {
        bind_native(machine.engine, plt, target);
    }
    write(machine.engine, DRAWING, &DRAW_VTABLE.to_le_bytes());
    write(
        machine.engine,
        DRAW_VTABLE + 48,
        &PAGE_RECT_FN.to_le_bytes(),
    );
    write(machine.engine, DRAW_VTABLE + 88, &DRAWING_GET.to_le_bytes());
    write(
        machine.engine,
        DRAW_VTABLE + 96,
        &DRAWING_FLUSH.to_le_bytes(),
    );
    write(
        machine.engine,
        DRAW_VTABLE + 168,
        &DRAWING_DRAW_OBJECT.to_le_bytes(),
    );
    write(machine.engine, SELF, &[0; 512]);
    write(machine.engine, SELF + 72, &PDF_WRITER.to_le_bytes());
    write(machine.engine, SELF + 80, &WNOTE.to_le_bytes());
    write(machine.engine, SELF + 192, &DRAWING.to_le_bytes());
    write(machine.engine, WNOTE, &[0; 0x100]);
    write(machine.engine, WPAGE, &[0; 0x100]);
    write(machine.engine, BODY, &[0; 0x80]);
    write(machine.engine, TEXT, &[0; 0x80]);
    write(machine.engine, DRAWN_RECORD, &[0; 0x100]);
    for (i, value) in [10.0_f32, 20.0, 40.0, 60.0].iter().enumerate() {
        write(
            machine.engine,
            DRAWN_RECORD + i as u64 * 4,
            &value.to_bits().to_le_bytes(),
        );
    }
    write(
        machine.engine,
        DRAWN_RECORD + 16,
        &23.0_f32.to_bits().to_le_bytes(),
    );
    write(
        machine.engine,
        DRAWN_RECORD + 20,
        &17.0_f32.to_bits().to_le_bytes(),
    );
    write(
        machine.engine,
        DRAWN_RECORD + 40,
        &DRAW_OBJECT.to_le_bytes(),
    );
    write(machine.engine, DRAW_OBJECT, &[0; 64]);
    write(machine.engine, SELF + 184, &DRAWING.to_le_bytes());
    // Trigger the actual standard list addBodyText setup, which computes
    // imported-page width / WPage width, then calls native WriteBodyText.
    machine.call(COMPOSER + 0x35b35c, &[SELF, 0]);
    let native_scale_bits = state
        .native_write_body_text_scale_bits
        .expect("native WriteBodyText entry scale was not observed");
    assert_eq!(state.get_data_calls, 1);
    assert_eq!(
        state.draw_calls.len(),
        1,
        "native BodyTextPDFWriter did not dispatch the synthetic marker"
    );
    assert_eq!(state.draw_calls[0].0, DRAW_OBJECT);
    let scale = state.page_rect[2] / state.width as f32;
    let expected = [33.0 * scale, 37.0 * scale, 63.0 * scale, 77.0 * scale];
    for (actual, expected) in state.draw_calls[0].1.into_iter().zip(expected) {
        assert!(
            (actual - expected).abs() < 0.001,
            "native body writer rect mismatch: {actual} vs {expected}"
        );
    }
    assert!(
        machine.heap.cursor < 0x6200_0000,
        "Composer transform allocations escaped their arena"
    );
    let native_scale = f32::from_bits(native_scale_bits);
    assert!(native_scale.is_finite() && native_scale > 0.0);
    assert_eq!(native_scale_bits, scale.to_bits());
    (state.draw_calls[0].1, native_scale)
}

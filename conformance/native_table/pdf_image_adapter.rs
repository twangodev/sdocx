use super::*;
use std::path::Path;

const COMPOSER_BASE: u64 = 0x0600_0000;
const SPENPDF_BASE: u64 = 0x0300_0000;
const SPENBASE_BASE: u64 = 0x0400_0000;
const ADAPTER: u64 = 0x4001_0000;
const SHARED_PTR: u64 = 0x4001_1000;
const IMAGE_STRING: u64 = 0x4001_2000;
const IMAGE_PATH: u64 = 0x4001_3000;
const COMPOSER_SHA256: &str = "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f";

pub(super) struct Input<'a> {
    pub composer: &'a Path,
    pub path: &'a Path,
    pub destination_rect: [f32; 4],
    pub source_clip: [f32; 4],
    pub scale: f32,
}

pub(super) struct Capture {
    pub info: u64,
    pub constructor_info: Vec<u8>,
    pub after_setters_info: Vec<u8>,
    pub add_image_entry_info: Option<u64>,
    pub destination_rect: [f32; 4],
    pub source_clip: [f32; 4],
    pub scale: f32,
    pub path: String,
    pub owns_path: bool,
}

struct AddImageEntry {
    calls: usize,
    info: u64,
}

unsafe extern "C" fn observe_add_image_entry(engine: Engine, _: u64, _: u32, data: *mut c_void) {
    let observation = unsafe { &mut *data.cast::<AddImageEntry>() };
    let mut info = 0_u64;
    let result = unsafe { uc_reg_read(engine, 201, ptr::from_mut(&mut info).cast()) };
    if result == 0 {
        observation.calls += 1;
        observation.info = info;
    }
}

pub(super) fn add_image(machine: &mut Machine, handler: u64, capture: &mut Capture) -> u64 {
    let mut observation = AddImageEntry { calls: 0, info: 0 };
    let mut hook = 0;
    check(unsafe {
        uc_hook_add(
            machine.engine,
            &mut hook,
            4,
            observe_add_image_entry as *mut c_void,
            ptr::from_mut(&mut observation).cast(),
            SPENPDF_BASE + 0x82810,
            SPENPDF_BASE + 0x82810,
        )
    });
    let result = machine.call(SPENPDF_BASE + 0x82810, &[handler, 0, capture.info]);
    check(unsafe { uc_hook_del(machine.engine, hook) });
    assert_eq!(
        observation.calls, 1,
        "native AddImage entry was not observed exactly once"
    );
    assert_eq!(
        observation.info, capture.info,
        "native AddImage x2 differs from constructor-owned info pointer"
    );
    capture.add_image_entry_info = Some(observation.info);
    result
}

/// Construct PdfImageAdapter and execute its String overload plus the native
/// PdfObjectAdapter geometry setters. Host path/rect/clip/scale are inputs.
pub(super) fn populate(machine: &mut Machine, input: Input<'_>) -> Capture {
    verify_library(input.composer, COMPOSER_SHA256);
    let path = input.path.to_string_lossy();
    assert!(
        path.is_ascii() && path.len() <= 22,
        "native String image path must be <=22 ASCII bytes"
    );
    assert!(
        path.starts_with("/tmp/"),
        "probe image path must stay under /tmp"
    );
    assert!(input.destination_rect.iter().all(|value| value.is_finite()));
    assert!(input.source_clip.iter().all(|value| value.is_finite()));
    assert!(input.scale.is_finite() && input.scale > 0.0);
    assert!(input.destination_rect[2] > input.destination_rect[0]);
    assert!(input.destination_rect[3] > input.destination_rect[1]);
    assert!(input.source_clip[2] > input.source_clip[0]);
    assert!(input.source_clip[3] > input.source_clip[1]);

    map_library(
        machine.engine,
        input.composer,
        COMPOSER_BASE,
        COMPOSER_SHA256,
    );
    // Resolve only the imports reached by this exact constructor and setter
    // path. Other Composer imports are outside this bounded invocation.
    bind_native(
        machine.engine,
        COMPOSER_BASE + 0x5500e0,
        COMPOSER_BASE + 0x3447f8,
    ); // PdfObjectAdapter C2
    bind_native(
        machine.engine,
        COMPOSER_BASE + 0x550100,
        SPENPDF_BASE + 0x67fe0,
    ); // PDFUtil::String2string
    bind_native(
        machine.engine,
        COMPOSER_BASE + 0x54dac0,
        SPENBASE_BASE + 0xb11bc,
    ); // RectF::IsEmpty
    bind_native(
        machine.engine,
        COMPOSER_BASE + 0x5500f0,
        SPENBASE_BASE + 0xc6ba8,
    ); // Error::SetError
    bind_native(machine.engine, COMPOSER_BASE + 0x54d850, NEW);
    bind_native(machine.engine, COMPOSER_BASE + 0x54d870, DELETE);

    write(machine.engine, ADAPTER, &[0; 256]);
    write(machine.engine, SHARED_PTR, &[0; 16]);
    machine.call(COMPOSER_BASE + 0x343f0c, &[ADAPTER, SHARED_PTR, 0]);
    let info = read_u64(machine.engine, ADAPTER + 32);
    assert_ne!(
        info, 0,
        "native PdfImageAdapter constructor left mInfo null"
    );
    let constructor_info = read_bytes(machine.engine, info, 128);

    machine.call(SPENBASE_BASE + 0xc2a18, &[IMAGE_STRING]);
    write(machine.engine, IMAGE_PATH, path.as_bytes());
    write(machine.engine, IMAGE_PATH + path.len() as u64, &[0]);
    machine.call(SPENBASE_BASE + 0xc3120, &[IMAGE_STRING, IMAGE_PATH]);
    for (index, value) in input.source_clip.into_iter().enumerate() {
        register(machine.engine, 136 + index as i32, value.to_bits() as u64);
    }
    let set_image_result = machine.call(COMPOSER_BASE + 0x344054, &[ADAPTER, IMAGE_STRING]);
    assert_eq!(
        set_image_result, 1,
        "native PdfImageAdapter::SetImage(String, RectF) failed"
    );
    for (index, value) in input.destination_rect.into_iter().enumerate() {
        register(machine.engine, 136 + index as i32, value.to_bits() as u64);
    }
    machine.call(COMPOSER_BASE + 0x3450ac, &[ADAPTER]); // PdfObjectAdapter::SetRect
    register(machine.engine, 136, input.scale.to_bits() as u64);
    machine.call(COMPOSER_BASE + 0x3450c8, &[ADAPTER]); // PdfObjectAdapter::SetScale

    let destination_rect = std::array::from_fn(|index| {
        f32::from_bits(read_u32(machine.engine, info + 44 + index as u64 * 4))
    });
    let scale = f32::from_bits(read_u32(machine.engine, info + 60));
    let source_clip = std::array::from_fn(|index| {
        f32::from_bits(read_u32(machine.engine, info + 64 + index as u64 * 4))
    });
    let owns_path = read_u32(machine.engine, info + 104) as u8 != 0;
    assert_eq!(
        destination_rect, input.destination_rect,
        "native SetRect field layout changed"
    );
    assert_eq!(
        source_clip, input.source_clip,
        "native SetImage clip field layout changed"
    );
    assert_eq!(
        scale.to_bits(),
        input.scale.to_bits(),
        "native SetScale field layout changed"
    );
    assert!(!owns_path, "String overload unexpectedly owns caller path");
    Capture {
        info,
        constructor_info,
        after_setters_info: read_bytes(machine.engine, info, 128),
        add_image_entry_info: None,
        destination_rect,
        source_clip,
        scale,
        path: path.into_owned(),
        owns_path,
    }
}

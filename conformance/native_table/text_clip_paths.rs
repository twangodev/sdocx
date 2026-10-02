use super::*;

const PDFIUM: u64 = 0x0100_0000;
const PDFIUM_SHA256: &str = "4bd55ef116541205cb8aaf04812a317fe11911c0ff5a5d44526d98d6b0e48854";
const TEXT_OBJECT: u64 = MODEL + 0xd000;
const CLIP_HOLDER: u64 = PDF + 0x83108;
const APPEND_CLIP: u64 = PDF + 0xa2460;
const TRANSFORM_PATH: u64 = PDF + 0xa2444;
const APPEND_RECT: u64 = PDF + 0xa2414;
const MEMCPY: u64 = PDFIUM + 0x630e90;

#[derive(Default, Debug, PartialEq)]
struct PathObservation {
    rect: Option<[f32; 4]>,
    matrix: Option<[f32; 6]>,
    points: Vec<[f32; 2]>,
    point_flags: Vec<[u8; 2]>,
    reference_count: Option<u64>,
    fill_type: Option<u32>,
    holders: usize,
    copies: usize,
}

unsafe extern "C" fn observe_path(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let observation = unsafe { &mut *data.cast::<PathObservation>() };
    match address {
        MEMCPY => {
            let destination = read_register(engine, REGISTER_X0);
            let source = read_register(engine, REGISTER_X0 + 1);
            let count = usize::try_from(read_register(engine, REGISTER_X0 + 2)).unwrap();
            assert!(count <= 128);
            let mut bytes = vec![0; count];
            check(unsafe { uc_mem_read(engine, source, bytes.as_mut_ptr().cast(), count) });
            write(engine, destination, &bytes);
            observation.copies += 1;
        }
        CLIP_HOLDER => {
            assert_eq!(read_register(engine, REGISTER_X0), TEXT_OBJECT + 8);
            observation.holders += 1;
        }
        APPEND_RECT => {
            assert!(observation.rect.is_none());
            observation.rect = Some(std::array::from_fn(|axis| {
                f32::from_bits(read_register(engine, 136 + axis as i32) as u32)
            }));
        }
        TRANSFORM_PATH => {
            assert!(observation.matrix.is_none());
            let matrix = read_register(engine, REGISTER_X0 + 1);
            observation.matrix = Some(std::array::from_fn(|axis| {
                read_float(engine, matrix + axis as u64 * 4)
            }));
        }
        APPEND_CLIP => {
            assert_eq!(read_register(engine, REGISTER_X0), TEXT_OBJECT + 8);
            let path = read_register(engine, REGISTER_X0 + 1);
            let retained = read_u64(engine, path);
            assert_ne!(retained, 0);
            observation.reference_count = Some(read_u64(engine, retained + 8));
            let begin = read_u64(engine, retained + 16);
            let end = read_u64(engine, retained + 24);
            assert_eq!(end - begin, 60);
            for index in 0..5 {
                let point = begin + index * 12;
                observation
                    .points
                    .push([read_float(engine, point), read_float(engine, point + 4)]);
                let mut flags = [0; 2];
                check(unsafe { uc_mem_read(engine, point + 8, flags.as_mut_ptr().cast(), 2) });
                observation.point_flags.push(flags);
            }
            observation.fill_type = Some(read_register(engine, REGISTER_X0 + 2) as u32);
            register(engine, 260, STOP);
        }
        _ => unreachable!(),
    }
}

struct PathRecorder {
    engine: Engine,
    hooks: Vec<usize>,
    observation: Box<PathObservation>,
}

impl PathRecorder {
    fn new(machine: &Machine) -> Self {
        let mut recorder = Self {
            engine: machine.engine,
            hooks: Vec::new(),
            observation: Box::default(),
        };
        for address in [CLIP_HOLDER, MEMCPY] {
            write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
        }
        for address in [
            CLIP_HOLDER,
            MEMCPY,
            APPEND_RECT,
            TRANSFORM_PATH,
            APPEND_CLIP,
        ] {
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    observe_path as *mut c_void,
                    ptr::from_mut(recorder.observation.as_mut()).cast(),
                    address,
                    address,
                )
            });
            recorder.hooks.push(hook);
        }
        recorder
    }
}

impl Drop for PathRecorder {
    fn drop(&mut self) {
        for hook in &self.hooks {
            check(unsafe { uc_hook_del(self.engine, *hook) });
        }
    }
}

fn fixture(machine: &mut Machine, case: &Case, fill: u8) -> String {
    machine.heap.cursor = HEAP;
    machine.heap.allocation_fill = fill;
    let mut gate = Recorder::new(machine);
    let clip = case.fixture(machine, &mut gate, fill);
    let selected = gate.observation.backend_clip;
    let translation = gate.observation.translation;
    drop(gate);
    let recorder = PathRecorder::new(machine);
    if selected {
        write(machine.engine, PAINT + 96, &[0; 8]);
        for (axis, value) in translation.unwrap().into_iter().enumerate() {
            write(
                machine.engine,
                PAINT + 96 + axis as u64 * 4,
                &value.to_le_bytes(),
            );
        }
        register(machine.engine, REGISTER_X0 + 22, TEXT_OBJECT);
        register(machine.engine, REGISTER_X0 + 23, PAINT);
        register(machine.engine, REGISTER_X0 + 24, STACK + 32);
        for (index, value) in [(11, 1.0_f32), (12, -0.0), (14, 0.0)] {
            register(machine.engine, 136 + index, u64::from(value.to_bits()));
        }
        register(machine.engine, 50, 800.0_f64.to_bits());
        check(unsafe { uc_emu_start(machine.engine, PDF_CLIP, STOP, 1_000_000, 10000) });
        assert_eq!(read_register(machine.engine, 260), STOP);
        assert_eq!(recorder.observation.holders, 1);
        assert_eq!(recorder.observation.reference_count, Some(2));
        assert_eq!(recorder.observation.fill_type, Some(1));
    }
    let observation = recorder.observation.as_ref();
    assert_eq!(selected, !observation.points.is_empty());
    let bits: Vec<_> = observation
        .points
        .iter()
        .map(|point| point.map(f32::to_bits))
        .collect();
    format!(
        "{},\"append_rect\":{},\"clip_matrix\":{},\"path_points\":{:?},\"path_point_bits\":{bits:?},\"point_flags\":{:?},\"reference_count\":{},\"fill_type\":{},\"copies\":{}}}",
        clip.strip_suffix('}').unwrap(),
        optional(observation.rect),
        optional(observation.matrix),
        observation.points,
        observation.point_flags,
        observation
            .reference_count
            .map_or_else(|| "null".into(), |count| count.to_string()),
        observation
            .fill_type
            .map_or_else(|| "null".into(), |fill| fill.to_string()),
        observation.copies,
    )
}

pub(super) fn capture(machine: &mut Machine, pdfium: &Path) {
    map_library(machine.engine, pdfium, PDFIUM, PDFIUM_SHA256);
    for (plt, target) in [
        (PDF + 0xaab00, PDFIUM + 0x47957c),
        (PDF + 0xaaba0, PDFIUM + 0x479768),
        (PDF + 0xaabb0, PDFIUM + 0x56efd8),
        (PDF + 0xaab30, PDFIUM + 0x4796e4),
        (PDF + 0xaab50, PDFIUM + 0x479584),
        (PDFIUM + 0x630db0, NEW),
        (PDFIUM + 0x630d60, DELETE),
    ] {
        bind_native(machine.engine, plt, target);
    }
    let captures: Vec<_> = cases()
        .iter()
        .map(|case| {
            let expected = fixture(machine, case, 0);
            for fill in [0xa5, 0xff] {
                assert_eq!(
                    fixture(machine, case, fill),
                    expected,
                    "{} memory fill",
                    case.name
                );
            }
            expected
        })
        .collect();
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"memory_fills\":[0,165,255],\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"composer_library_sha256\":\"{COMPOSER_SHA256}\",\"pdf_library_sha256\":\"{PDF_SHA256}\",\"pdfium_library_sha256\":\"{PDFIUM_SHA256}\",\"clip_start\":\"0x37f6ac\",\"clip_end\":\"0x37f7b0\",\"pdf_gate\":\"0xa23d4\",\"path_start\":\"0xa23e4\",\"path_sink\":\"0xa2460\",\"page_height\":800.0,\"rotation\":0.0,\"measurement_inputs\":\"Supplied Model source rectangles, cached DrawnText rectangles, rounded cell origins, PDF scale, page height and zero-rotation sine/cosine. Composer conditional clips and PDF empty gate execute before resuming the native PDF path window. Pdfium constructs, allocates, appends, transforms and copies actual clip paths. Allocation, memory copy, PDF page getter, paint translation recording, clip-holder initialization and final AppendPath sink are host interfaces. Source rectangle lifecycle, shaping, complete DrawText, PDF serialization and device pixels are not executed.\",\"cases\":[\n{}\n]}}",
        captures.join(",\n"),
    );
}

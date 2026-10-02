use super::*;

const COMPOSER: u64 = 0x0600_0000;
const COMPOSER_SHA256: &str = "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f";
const PDF: u64 = 0x0400_0000;
const PDF_SHA256: &str = "cdc62f9e02a3ef60e0dc504dbb13c4352accb811fb1ec629a7c8648954dd8f04";
const DRAWN: u64 = MODEL + 0x18000;
const PAINT: u64 = MODEL + 0x19000;
const PAINT_VTABLE: u64 = MODEL + 0x1a000;
const ENGINE: u64 = MODEL + 0x1b000;
const ENGINE_VTABLE: u64 = MODEL + 0x1c000;
const RAW_PAINT: u64 = MODEL + 0x1d000;
const WRITER: u64 = MODEL + 0x1e000;
const FILL_COLOR: u64 = PDF + 0xaab80;
const REGISTER_S0: i32 = 136;

#[derive(Clone, Copy)]
enum Transport {
    ForegroundColor,
    TableForegroundAlpha,
    CodeForegroundAlpha,
    BackgroundAlpha,
}

impl Transport {
    fn name(self) -> &'static str {
        match self {
            Self::ForegroundColor => "retained-foreground-color",
            Self::TableForegroundAlpha => "table-foreground-alpha",
            Self::CodeForegroundAlpha => "code-foreground-alpha",
            Self::BackgroundAlpha => "background-alpha-product",
        }
    }

    fn fixture(self, machine: &Machine, alpha: u8, opacity: f32, fill: u8) -> String {
        initialize(machine.engine, alpha, opacity, fill);
        let recorder = Recorder::new(machine);
        register(machine.engine, REGISTER_X0 + 19, PAINT);
        register(machine.engine, REGISTER_X0 + 20, DRAWN);
        run(machine, COMPOSER + 0x383710, COMPOSER + 0x383724);
        let initial_argb = read_u32(machine.engine, RAW_PAINT + 4);
        assert_eq!(initial_argb, u32::from(alpha) << 24 | 0x0012_3456);
        match self {
            Self::ForegroundColor => {}
            Self::TableForegroundAlpha => {
                register(machine.engine, REGISTER_X0 + 20, PAINT);
                register(machine.engine, REGISTER_X0 + 21, DRAWN);
                register(machine.engine, REGISTER_X0 + 22, WRITER);
                run(machine, COMPOSER + 0x37f5f4, COMPOSER + 0x37f62c);
            }
            Self::CodeForegroundAlpha => {
                register(machine.engine, REGISTER_X0 + 20, PAINT);
                register(machine.engine, REGISTER_X0 + 22, WRITER);
                run(machine, COMPOSER + 0x3799f0, COMPOSER + 0x379a04);
            }
            Self::BackgroundAlpha => {
                register(machine.engine, REGISTER_X0 + 19, WRITER);
                register(machine.engine, REGISTER_X0 + 20, PAINT);
                register(machine.engine, REGISTER_X0 + 21, DRAWN);
                run(machine, COMPOSER + 0x37978c, COMPOSER + 0x3797a4);
                run(machine, COMPOSER + 0x3798b4, COMPOSER + 0x3798e0);
            }
        }
        let final_argb = machine.call(PDF + 0x7585c, &[RAW_PAINT]) as u32;
        machine.call(PDF + 0x7583c, &[RAW_PAINT]);
        let final_alpha = f32::from_bits(read_register(machine.engine, REGISTER_S0) as u32);
        let expected_alpha = match self {
            Self::ForegroundColor => alpha,
            Self::TableForegroundAlpha => {
                let source = if alpha >= 128 {
                    1.0
                } else {
                    f32::from(alpha) / 255.0
                };
                ((source * opacity) * 255.0) as u8
            }
            Self::CodeForegroundAlpha => (opacity * 255.0) as u8,
            Self::BackgroundAlpha => (((f32::from(alpha) / 255.0) * opacity) * 255.0) as u8,
        };
        assert_eq!(final_argb >> 24, u32::from(expected_alpha));
        assert_eq!(final_argb & 0x00ff_ffff, 0x0012_3456);
        register(machine.engine, REGISTER_X0 + 23, RAW_PAINT);
        register(machine.engine, REGISTER_X0 + 22, MODEL + 0x1f000);
        run(machine, PDF + 0xa2368, PDF + 0xa2390);
        let observation = recorder.observation.as_ref();
        assert_eq!(
            observation.rgba,
            Some([0x12, 0x34, 0x56, u32::from(expected_alpha)])
        );
        let expected_colors = if matches!(self, Self::BackgroundAlpha) {
            2
        } else {
            1
        };
        assert_eq!(observation.color_calls, expected_colors);
        assert_eq!(
            observation.alpha_calls,
            usize::from(!matches!(self, Self::ForegroundColor))
        );
        format!(
            "{{\"transport\":{:?},\"supplied_argb\":{initial_argb},\"writer_opacity\":{opacity:?},\"writer_opacity_bits\":{},\"final_argb\":{final_argb},\"final_alpha\":{final_alpha:?},\"final_alpha_bits\":{},\"rgba_fill_request\":{:?},\"native_color_calls\":{},\"native_alpha_calls\":{}}}",
            self.name(),
            opacity.to_bits(),
            final_alpha.to_bits(),
            observation.rgba.unwrap(),
            observation.color_calls,
            observation.alpha_calls
        )
    }
}

fn initialize(engine: Engine, alpha: u8, opacity: f32, fill: u8) {
    write(engine, MODEL, &vec![fill; 0x100000]);
    write(engine, DRAWN, &[0; 160]);
    let color = u32::from(alpha) << 24 | 0x0012_3456;
    write(engine, DRAWN + 136, &color.to_le_bytes());
    write(engine, DRAWN + 144, &color.to_le_bytes());
    write(engine, PAINT, &PAINT_VTABLE.to_le_bytes());
    write(engine, PAINT + 8, &ENGINE.to_le_bytes());
    write(engine, PAINT + 16, &RAW_PAINT.to_le_bytes());
    write(engine, PAINT_VTABLE + 16, &(PDF + 0x665a0).to_le_bytes());
    write(engine, PAINT_VTABLE + 24, &(PDF + 0x665b4).to_le_bytes());
    write(engine, ENGINE, &ENGINE_VTABLE.to_le_bytes());
    write(engine, ENGINE_VTABLE + 736, &(PDF + 0x73314).to_le_bytes());
    write(engine, ENGINE_VTABLE + 744, &(PDF + 0x73338).to_le_bytes());
    for offset in [48, 104] {
        write(engine, WRITER + offset, &opacity.to_le_bytes());
    }
    for index in 0..29 {
        register(engine, REGISTER_X0 + index, 0);
    }
}

fn run(machine: &Machine, start: u64, end: u64) {
    register(machine.engine, REGISTER_SP, STACK);
    register(machine.engine, REGISTER_X30, STOP);
    let error = unsafe { uc_emu_start(machine.engine, start, end, 1_000_000, 1000) };
    assert_eq!(
        error,
        0,
        "window {start:x} failed at PC {:x}",
        read_register(machine.engine, 260)
    );
    assert_eq!(read_register(machine.engine, 260), end);
}

#[derive(Default)]
struct Observation {
    color_calls: usize,
    alpha_calls: usize,
    rgba: Option<[u32; 4]>,
}

unsafe extern "C" fn observe(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let observation = unsafe { &mut *data.cast::<Observation>() };
    match address - PDF {
        0x73338 => observation.color_calls += 1,
        0x73314 => observation.alpha_calls += 1,
        0xaab80 => {
            observation.rgba = Some(std::array::from_fn(|index| {
                read_register(engine, REGISTER_X0 + index as i32 + 1) as u32
            }))
        }
        _ => unreachable!(),
    }
}

struct Recorder {
    engine: Engine,
    hooks: Vec<usize>,
    observation: Box<Observation>,
}

impl Recorder {
    fn new(machine: &Machine) -> Self {
        let mut recorder = Self {
            engine: machine.engine,
            hooks: vec![],
            observation: Box::default(),
        };
        write(machine.engine, FILL_COLOR, &0xd65f03c0_u32.to_le_bytes());
        for address in [PDF + 0x73338, PDF + 0x73314, FILL_COLOR] {
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    observe as *mut c_void,
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

impl Drop for Recorder {
    fn drop(&mut self) {
        for hook in &self.hooks {
            check(unsafe { uc_hook_del(self.engine, *hook) });
        }
    }
}

pub(super) fn capture(machine: &mut Machine, composer: &Path, pdf: &Path) {
    map_library(machine.engine, composer, COMPOSER, COMPOSER_SHA256);
    map_library(machine.engine, pdf, PDF, PDF_SHA256);
    let mut cases = vec![];
    for transport in [
        Transport::ForegroundColor,
        Transport::TableForegroundAlpha,
        Transport::CodeForegroundAlpha,
        Transport::BackgroundAlpha,
    ] {
        let opacities: &[f32] = if matches!(transport, Transport::ForegroundColor) {
            &[1.0]
        } else {
            &[0.0, 0.5, 1.0]
        };
        for alpha in [0, 1, 127, 128, 254, 255] {
            for &opacity in opacities {
                let expected = transport.fixture(machine, alpha, opacity, 0);
                for fill in [0xa5, 0xff] {
                    assert_eq!(
                        transport.fixture(machine, alpha, opacity, fill),
                        expected,
                        "{} memory fill",
                        transport.name()
                    );
                }
                cases.push(expected);
            }
        }
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"composer_library_sha256\":\"{COMPOSER_SHA256}\",\"pdf_library_sha256\":\"{PDF_SHA256}\",\"memory_fills\":[0,165,255],\"composer_color_window\":[\"0x383710\",\"0x383724\"],\"composer_table_alpha_window\":[\"0x37f5f4\",\"0x37f62c\"],\"composer_code_alpha_window\":[\"0x3799f0\",\"0x379a04\"],\"composer_background_alpha_ratio_window\":[\"0x37978c\",\"0x3797a4\"],\"composer_background_window\":[\"0x3798b4\",\"0x3798e0\"],\"pdf_color_call_chain\":[\"0x665b4\",\"0x73338\"],\"pdf_alpha_call_chain\":[\"0x665a0\",\"0x73314\"],\"pdf_native_getters\":[\"0x7585c\",\"0x7583c\"],\"pdf_rgba_window\":[\"0xa2368\",\"0xa2390\"],\"capture_boundary\":\"Native Composer color/alpha instruction windows call complete unchanged PDFPaint SetColor/SetAlpha and PDFEnginePdfium SetColor/SetAlpha functions using supplied paint wrapper, engine, vtables and raw paint storage. Native raw paint getters and the PdfiumTextHandler RGBA argument extraction window execute; FPDFPageObj_SetFillColor is intercepted and returns without creating or painting a PDF object. Retained foreground-only cases exercise color transport only; their placement in Body/Placed full writers is source traced, not executed here. The background alpha-ratio window executes the native background alpha load/division into S10; the following window receives writer opacity at WRITER+48; alpha-zero background rows describe setter transport after the omitted caller alpha gate, not reachable paint requests. Table opacity is supplied at WRITER+104 because native X22 has already advanced by eight; code opacity is supplied at WRITER+48. All windows receive supplied live register state, not native constructors or complete writers. Font data, shaping, draw lists, export selection, full callers, clipping, PDF allocation, rendering and pixels are excluded.\",\"cases\":[\n{}\n]}}",
        cases.join(",\n")
    );
}

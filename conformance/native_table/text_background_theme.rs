use super::*;
use frames::{BASE, BASE_SHA256};

const THEME: u64 = MODEL + 0x18000;
const CONSTRUCT: u64 = BASE + 0xe5168;
const GET_COLOR: u64 = BASE + 0xe51a4;
const LIGHT_CONTROL: u64 = BASE + 0xe51d0;
const COLOR_TO_HSL: u64 = BASE + 0xe48f8;
const RGB_TO_HSL: u64 = BASE + 0xe4964;
const HSL_TO_COLOR: u64 = BASE + 0xe4b34;
const FMOD: u64 = BASE + 0xe7c70;
const REGISTER_D0: i32 = 40;

#[link(name = "m")]
unsafe extern "C" {
    fn fmod(dividend: f64, divisor: f64) -> f64;
}

#[derive(Default)]
struct Observation {
    calls: [usize; 4],
    remainders: Vec<[u64; 3]>,
}

unsafe extern "C" fn observe(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let observation = unsafe { &mut *data.cast::<Observation>() };
    match address {
        LIGHT_CONTROL => observation.calls[0] += 1,
        COLOR_TO_HSL => observation.calls[1] += 1,
        RGB_TO_HSL => observation.calls[2] += 1,
        HSL_TO_COLOR => observation.calls[3] += 1,
        FMOD => {
            let dividend = read_register(engine, REGISTER_D0);
            let divisor = read_register(engine, REGISTER_D0 + 1);
            let left = f64::from_bits(dividend);
            let right = f64::from_bits(divisor);
            assert!(left.is_finite() && right.is_finite() && right != 0.0);
            let result = unsafe { fmod(left, right) };
            assert!(result.is_finite());
            observation
                .remainders
                .push([dividend, divisor, result.to_bits()]);
            register(engine, REGISTER_D0, result.to_bits());
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
            hooks: Vec::new(),
            observation: Box::default(),
        };
        write(machine.engine, FMOD, &0xd65f03c0_u32.to_le_bytes());
        for address in [LIGHT_CONTROL, COLOR_TO_HSL, RGB_TO_HSL, HSL_TO_COLOR, FMOD] {
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

fn fixture(machine: &Machine, color: u32, fill: u8) -> String {
    write(machine.engine, MODEL, &vec![fill; 0x100000]);
    write(machine.engine, THEME, &[0; 8]);
    let recorder = Recorder::new(machine);
    machine.call(CONSTRUCT, &[THEME]);
    let mapped = machine.call(GET_COLOR, &[THEME, u64::from(color)]) as u32;
    let observation = recorder.observation.as_ref();
    assert_eq!(observation.calls, [1, 1, 1, 1]);
    assert!((1..=2).contains(&observation.remainders.len()));
    format!(
        "{{\"input\":\"{color:08x}\",\"dark\":\"{mapped:08x}\",\"input_argb\":{color},\"mapped_argb\":{mapped},\"input_nonzero\":{},\"mapped_nonzero\":{},\"native_calls\":{:?},\"fmod_argument_result_bits\":{:?}}}",
        color != 0,
        mapped != 0,
        observation.calls,
        observation.remainders
    )
}

pub(super) fn capture(machine: &mut Machine, base: &Path) {
    map_library(machine.engine, base, BASE, BASE_SHA256);
    for (plt, target) in [
        (0xe7b60, LIGHT_CONTROL),
        (0xe7b00, COLOR_TO_HSL),
        (0xe7b10, RGB_TO_HSL),
        (0xe7b20, HSL_TO_COLOR),
    ] {
        bind_native(machine.engine, BASE + plt, target);
    }
    let cases: Vec<_> = [
        0x0000_0000,
        0x0000_0001,
        0x00ff_ffff,
        0x0012_3456,
        0x0100_0000,
        0x0100_0001,
        0x01ff_ffff,
        0x0112_3456,
        0x8000_0000,
        0x80ff_ffff,
        0xff00_0000,
        0xffff_ffff,
    ]
    .into_iter()
    .map(|color| {
        let expected = fixture(machine, color, 0);
        for fill in [0xa5, 0xff] {
            assert_eq!(
                fixture(machine, color, fill),
                expected,
                "color {color:08x} memory fill"
            );
        }
        expected
    })
    .collect();
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"base_library_sha256\":\"{BASE_SHA256}\",\"memory_fills\":[0,165,255],\"dark_theme_constructor\":\"0xe5168\",\"dark_theme_get_color\":\"0xe51a4\",\"native_call_order\":[\"0xe51d0\",\"0xe48f8\",\"0xe4964\",\"0xe4b34\"],\"fmod_plt\":\"0xe7c70\",\"capture_boundary\":\"Complete native DarkColorTheme construction/GetColor/getColorByLightControl and ColorToHSL/RGBToHSL/HSLToColor execute unchanged for supplied ARGB values. Only libc fmod is host supplied. Recorded nonzero predicates describe captured input/output integers; native background selection, Widget conversion, painting and pixels do not execute. No density or document geometry enters this color-only call.\",\"cases\":[\n{}\n]}}",
        cases.join(",\n")
    );
}

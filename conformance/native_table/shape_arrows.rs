use super::*;

const EFFECT: u64 = MODEL + 0x1000;
const IMPLEMENTATION: u64 = MODEL + 0x2000;
const STYLE: u64 = MODEL + 0x3000;
const SEGMENTS: u64 = MODEL + 0x4000;
const DRAW_PATH: u64 = MODEL + 0xe000;
const DRAW_RECT: u64 = MODEL + 0xe010;
const CALLS: [u64; 21] = [
    0xba910, 0xba8f0, 0xba920, 0xbab00, 0xbab10, 0xbab30, 0xbab50, 0xbab60, 0xbab70, 0xbab80,
    0xbc460, 0xbb210, 0xbb230, 0xbb260, 0xbb240, 0xbca50, 0xbab90, 0xbc730, 0xb8ba0, 0xbc8b0,
    0xbcbe0,
];

#[link(name = "m")]
unsafe extern "C" {
    fn atan2f(y: f32, x: f32) -> f32;
}

#[derive(Default)]
struct Recorder {
    events: Vec<String>,
}

fn floats(engine: Engine, count: usize) -> Vec<f32> {
    (0..count)
        .map(|index| f32::from_bits(read_register(engine, 136 + index as i32) as u32))
        .collect()
}

unsafe extern "C" fn record(engine: Engine, address: u64, _: u32, context: *mut c_void) {
    let output = unsafe { &mut *context.cast::<Recorder>() };
    let event = match address {
        DRAW_PATH => Some("{\"op\":\"draw_path\"}".into()),
        DRAW_RECT => {
            let rect = read_register(engine, REGISTER_X0 + 1);
            let values: [f32; 4] =
                std::array::from_fn(|axis| read_float(engine, rect + axis as u64 * 4));
            Some(format!("{{\"op\":\"rectangle\",\"values\":{values:?}}}"))
        }
        _ => match address - DRAWING_BASE {
            0xbcbe0 => {
                let values = floats(engine, 2);
                register(
                    engine,
                    136,
                    u64::from(unsafe { atan2f(values[0], values[1]) }.to_bits()),
                );
                None
            }
            0xbc8b0 => Some(format!(
                "{{\"op\":\"arrow_pose\",\"type\":{},\"size\":{},\"short\":{},\"values\":{:?}}}",
                read_register(engine, REGISTER_X0 + 2),
                read_register(engine, REGISTER_X0 + 3),
                read_register(engine, REGISTER_X0 + 4),
                floats(engine, 3)
            )),
            0xb8ba0 => {
                let event = format!(
                    "{{\"op\":\"cleanup_request\",\"width\":{},\"height\":{}}}",
                    read_register(engine, REGISTER_X0),
                    read_register(engine, REGISTER_X0 + 1)
                );
                register(engine, REGISTER_X0, 0);
                Some(event)
            }
            0xbab60 | 0xbab70 | 0xbb230 => Some(format!(
                "{{\"op\":{:?},\"values\":{:?}}}",
                match address - DRAWING_BASE {
                    0xbab60 => "move",
                    0xbab70 => "line",
                    _ => "translate",
                },
                floats(engine, 2)
            )),
            0xbb260 => Some(format!(
                "{{\"op\":\"rotate\",\"values\":{:?}}}",
                floats(engine, 1)
            )),
            0xbca50 => Some(format!(
                "{{\"op\":\"circle\",\"values\":{:?}}}",
                floats(engine, 3)
            )),
            0xbc460 => Some("{\"op\":\"close\"}".into()),
            0xbab90 => Some(format!(
                "{{\"op\":\"path_effect\",\"value\":{}}}",
                read_register(engine, REGISTER_X0 + 1)
            )),
            0xbab10 => Some(format!(
                "{{\"op\":\"color\",\"value\":{}}}",
                read_register(engine, REGISTER_X0 + 1)
            )),
            0xbc730 => Some(format!(
                "{{\"op\":\"transfer_mode\",\"value\":{}}}",
                read_register(engine, REGISTER_X0 + 1)
            )),
            0xba910 | 0xba8f0 | 0xba920 | 0xbab00 | 0xbab30 | 0xbab50 | 0xbab80 | 0xbb210
            | 0xbb240 => None,
            _ => unreachable!("unbound arrow call {address:x}"),
        },
    };
    if let Some(event) = event {
        output.events.push(event);
    }
}

pub(super) fn capture(machine: &mut Machine, drawing: &Path) {
    shape_paths::verify_host_math();
    machine.load_drawing(drawing);
    for (plt, target) in [(0xbca00, 0x395980), (0xbc9f0, 0x395a94)] {
        bind_native(machine.engine, DRAWING_BASE + plt, target);
    }
    write(machine.engine, EFFECT + 8, &IMPLEMENTATION.to_le_bytes());
    write(machine.engine, IMPLEMENTATION + 24, &STYLE.to_le_bytes());
    write(machine.engine, CANVAS + 72, &CANVAS_MATRIX.to_le_bytes());
    write(machine.engine, CANVAS_MATRIX, &CANVAS_VTABLE.to_le_bytes());
    write(machine.engine, CANVAS_VTABLE + 96, &DRAW_PATH.to_le_bytes());
    write(machine.engine, CANVAS_VTABLE + 72, &DRAW_RECT.to_le_bytes());
    let mut recorder = Box::<Recorder>::default();
    let mut hooks = Vec::new();
    for address in CALLS
        .into_iter()
        .map(|address| DRAWING_BASE + address)
        .chain([DRAW_PATH, DRAW_RECT])
    {
        write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
        let mut hook = 0;
        check(unsafe {
            uc_hook_add(
                machine.engine,
                &mut hook,
                4,
                record as *mut c_void,
                ptr::from_mut(recorder.as_mut()).cast(),
                address,
                address,
            )
        });
        hooks.push(hook);
    }
    let mut geometry = Vec::new();
    for width in [0.1_f32, 1.0, 3.5, 100.25] {
        write(machine.engine, IMPLEMENTATION + 0x35c, &width.to_le_bytes());
        write(
            machine.engine,
            IMPLEMENTATION + 0x30c,
            &0x80402010_u32.to_le_bytes(),
        );
        write(
            machine.engine,
            IMPLEMENTATION + 0x2b8,
            &0x1234_u64.to_le_bytes(),
        );
        machine.call(DRAWING_BASE + 0xa00bc, &[EFFECT]);
        for size in 0_u64..3 {
            for kind in 1..=5 {
                recorder.events.clear();
                for (axis, value) in [20_f32, 30.0, 45.0].into_iter().enumerate() {
                    register(
                        machine.engine,
                        136 + axis as i32,
                        u64::from(value.to_bits()),
                    );
                }
                assert_eq!(
                    machine.call(DRAWING_BASE + 0xa1ad4, &[EFFECT, CANVAS, kind, size, 0]),
                    1
                );
                geometry.push(format!(
                    "{{\"width\":{width:?},\"type\":{kind},\"size\":{size},\"events\":[{}]}}",
                    recorder.events.join(",")
                ));
            }
        }
    }
    let mut poses = Vec::new();
    for end in [
        [100_f32, 0.0],
        [0.0, 100.0],
        [-100.0, 0.0],
        [0.0, -100.0],
        [123.25, 45.125],
        [0.0, 0.0],
    ] {
        write(machine.engine, SEGMENTS, &[0; 56]);
        write(machine.engine, SEGMENTS, &1_u32.to_le_bytes());
        write(machine.engine, SEGMENTS + 28, &2_u32.to_le_bytes());
        for (axis, value) in end.into_iter().enumerate() {
            write(
                machine.engine,
                SEGMENTS + 32 + axis as u64 * 4,
                &value.to_le_bytes(),
            );
        }
        recorder.events.clear();
        machine.call(DRAWING_BASE + 0xa1308, &[EFFECT, 1, SEGMENTS, CANVAS, 0]);
        machine.call(DRAWING_BASE + 0xa14f8, &[EFFECT, 1, SEGMENTS, 2, CANVAS, 0]);
        poses.push(format!(
            "{{\"end\":{end:?},\"events\":[{}]}}",
            recorder.events.join(",")
        ));
    }
    for hook in hooks {
        check(unsafe { uc_hook_del(machine.engine, hook) });
    }
    println!(
        "{{\"drawing_sha256\":\"{DRAWING_SHA256}\",\"model_sha256\":\"{LIBRARY_SHA256}\",\"boundary\":\"Executed DrawArrow and straight DrawBeginArrow/DrawEndArrow with supplied paths and widths; intercepted path/canvas/paint calls and cleanup surface requests. Cleanup factory returns null: no cleanup pixels execute. Original model arrow-size getters execute; host atan2f is hash-verified Linux libm, not Android bit parity. No complete SetEffect/DrawEffect, Skia painting or Samsung appearance comparison.\",\"geometry\":[{}],\"poses\":[{}]}}",
        geometry.join(",\n"),
        poses.join(",\n")
    );
}

use super::*;

const EFFECT: u64 = MODEL + 0x1000;
const IMPLEMENTATION: u64 = MODEL + 0x2000;
const STYLE: u64 = MODEL + 0x3000;
const PAINT: u64 = MODEL + 0x4000;
const CALLS: [u64; 5] = [0xbab30, 0xbaba0, 0xbab90, 0xbcad0, 0xbcaf0];

#[derive(Default)]
struct Recorder {
    width: f32,
    intervals: Vec<f32>,
    phase: f32,
    cap: Option<u32>,
    join: Option<u32>,
    installed: bool,
}

unsafe extern "C" fn record(engine: Engine, address: u64, _: u32, context: *mut c_void) {
    let output = unsafe { &mut *context.cast::<Recorder>() };
    match address - DRAWING_BASE {
        0xbab30 => output.width = f32::from_bits(read_register(engine, 136) as u32),
        0xbaba0 => {
            let pointer = read_register(engine, REGISTER_X0 + 1);
            let count = read_register(engine, REGISTER_X0 + 2);
            assert!(matches!(count, 2 | 4 | 6));
            output.intervals = (0..count)
                .map(|index| read_float(engine, pointer + index * 4))
                .collect();
            output.phase = f32::from_bits(read_register(engine, 136) as u32);
        }
        0xbab90 => output.installed = true,
        0xbcad0 => output.cap = Some(read_register(engine, REGISTER_X0 + 1) as u32),
        0xbcaf0 => output.join = Some(read_register(engine, REGISTER_X0 + 1) as u32),
        _ => unreachable!(),
    }
}

pub(super) fn capture(machine: &mut Machine, drawing: &Path) {
    machine.load_drawing(drawing);
    for (plt, getter) in [
        (0xbaaf0, 0x39557c),
        (0xbc9e0, 0x3956f4),
        (0xbcac0, 0x3957b0),
        (0xbcae0, 0x39586c),
    ] {
        bind_native(machine.engine, DRAWING_BASE + plt, getter);
    }
    let mut recorder = Box::<Recorder>::default();
    let mut hooks = Vec::new();
    for address in CALLS {
        let address = DRAWING_BASE + address;
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
    let mut cases = Vec::new();
    for width in [-2.0_f32, 0.0, 0.1, 1.0, 3.5, 100.25] {
        for dash in [0_u32, 1, 2, 3, 4, 5, 6, 7, 8, 255] {
            write(machine.engine, IMPLEMENTATION, &[0; 1024]);
            write(machine.engine, STYLE, &[0; 32]);
            machine.heap.cursor = HEAP;
            *recorder = Recorder::default();
            write(machine.engine, EFFECT + 8, &IMPLEMENTATION.to_le_bytes());
            write(machine.engine, IMPLEMENTATION + 24, &STYLE.to_le_bytes());
            write(machine.engine, STYLE, &width.to_le_bytes());
            write(machine.engine, STYLE + 8, &dash.to_le_bytes());
            write(machine.engine, STYLE + 12, &(dash % 4).to_le_bytes());
            write(machine.engine, STYLE + 16, &((dash + 1) % 4).to_le_bytes());
            assert_eq!(machine.call(DRAWING_BASE + 0x9fcec, &[EFFECT, PAINT]), 1);
            write(
                machine.engine,
                IMPLEMENTATION + 0x35c,
                &recorder.width.to_le_bytes(),
            );
            machine.call(DRAWING_BASE + 0xa00bc, &[EFFECT]);
            let lengths: [i32; 3] = std::array::from_fn(|index| {
                read_u32(machine.engine, IMPLEMENTATION + 0x3c0 + index as u64 * 4) as i32
            });
            cases.push(format!(
                "{{\"width\":{width:?},\"dash\":{dash},\"cap\":{},\"join\":{},\"effective_width\":{:?},\"intervals\":{:?},\"phase\":{:?},\"installed\":{},\"applied_cap\":{},\"applied_join\":{},\"arrow_lengths\":{lengths:?}}}",
                dash % 4, (dash + 1) % 4, recorder.width, recorder.intervals, recorder.phase,
                recorder.installed, recorder.cap.map_or("null".into(), |value| value.to_string()),
                recorder.join.map_or("null".into(), |value| value.to_string()),
            ));
        }
    }
    for hook in hooks {
        check(unsafe { uc_hook_del(machine.engine, hook) });
    }
    println!(
        "{{\"library_sha256\":\"{DRAWING_SHA256}\",\"model_sha256\":\"{LIBRARY_SHA256}\",\"boundary\":\"Executed SetLineStyleEffect and model getters; intercepted SkPaint setters and SkDashPathEffect constructor inputs. Executed setArrowSize with supplied effective width. No Skia painting, path effect execution, endpoint placement or device pixels.\",\"cases\":[{}]}}",
        cases.join(",\n")
    );
}

use super::*;

const HOST: u64 = 0x0300_0000;
const OBJECT: u64 = MODEL + 0xb000;
const OBJECT_VTABLE: u64 = MODEL + 0xb100;
const CONTEXT: u64 = MODEL + 0xb200;
const CONTEXT_VTABLE: u64 = MODEL + 0xb300;
const BITMAP: u64 = MODEL + 0xb400;
const BUFFER: u64 = MODEL + 0xb500;
const GPU_BITMAP: u64 = MODEL + 0xb600;

#[derive(Default)]
struct Observation {
    present: bool,
    bounds: [f32; 4],
    events: Vec<String>,
}

unsafe extern "C" fn callback(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let o = unsafe { &mut *data.cast::<Observation>() };
    let arg = |i| read_register(engine, REGISTER_X0 + i);
    match (address - HOST) / 32 {
        0 => {
            o.events.push(format!("thumbnail:{:#x}", arg(0)));
            register(engine, REGISTER_X0, if o.present { BITMAP } else { 0 });
        }
        1 => {
            o.events.push("queue".into());
            register(engine, REGISTER_X0, CONTEXT);
        }
        2 => {
            o.events.push("width".into());
            register(engine, REGISTER_X0, 17);
        }
        3 => {
            o.events.push("height".into());
            register(engine, REGISTER_X0, 11);
        }
        4 => {
            o.events.push("buffer".into());
            register(engine, REGISTER_X0, BUFFER);
        }
        5 => {
            o.events
                .push(format!("create:{},{},{:#x}", arg(1), arg(2), arg(4)));
            register(engine, REGISTER_X0, GPU_BITMAP);
        }
        6 => o.events.push(format!("save:{}", arg(1))),
        7 => {
            o.events.push("rect".into());
            for (i, x) in o.bounds.iter().enumerate() {
                register(engine, 136 + i as i32, u64::from(x.to_bits()));
            }
        }
        8 => o.events.push("paint".into()),
        9 => o.events.push(format!("antialias:{}", arg(1))),
        10 => {
            let rect =
                std::array::from_fn::<_, 4, _>(|i| read_float(engine, arg(2) + 4 * i as u64));
            o.events.push(format!("draw:{:#x}:{rect:?}", arg(1)));
        }
        11 => o.events.push(format!("restore:{}", arg(1) as u32)),
        12 => o.events.push(format!("release:{:#x}", arg(0))),
        13 => o.events.push("paint_drop".into()),
        14 => o.events.push("log".into()),
        _ => unreachable!(),
    }
}

pub(super) fn capture(machine: &Machine) {
    let e = machine.engine;
    check(unsafe { uc_mem_map(e, HOST, 0x1000, 7) });
    for (plt, n) in [
        (0xb8a30, 14),
        (0xbb130, 0),
        (0xb9860, 2),
        (0xb9870, 3),
        (0xb8c10, 4),
        (0xb8aa0, 5),
        (0xb8ab0, 8),
        (0xb92f0, 9),
        (0xb8a90, 12),
        (0xb8ad0, 13),
    ] {
        bind_native(e, DRAWING_BASE + plt, HOST + n * 32);
    }
    for (addr, value) in [
        (DRAWING_OBJECT + 8, CONTEXT),
        (DRAWING_OBJECT + 16, OBJECT),
        (OBJECT, OBJECT_VTABLE),
        (CONTEXT, CONTEXT_VTABLE),
        (CANVAS, CANVAS_VTABLE),
        (OBJECT_VTABLE + 160, HOST + 7 * 32),
        (CONTEXT_VTABLE + 24, HOST + 32),
        (CANVAS_VTABLE + 48, HOST + 6 * 32),
        (CANVAS_VTABLE + 320, HOST + 10 * 32),
        (CANVAS_VTABLE + 56, HOST + 11 * 32),
    ] {
        write(e, addr, &value.to_le_bytes());
    }
    let mut o = Box::new(Observation::default());
    let mut hooks = Vec::new();
    for n in 0..15 {
        let addr = HOST + n * 32;
        write(e, addr, &0xd65f03c0_u32.to_le_bytes());
        let mut hook = 0;
        check(unsafe {
            uc_hook_add(
                e,
                &mut hook,
                4,
                callback as *mut c_void,
                ptr::from_mut(o.as_mut()).cast(),
                addr,
                addr,
            )
        });
        hooks.push(hook);
    }
    let mut cases = Vec::new();
    for (present, bounds) in [
        (false, [1., 2., 3., 4.]),
        (true, [11.25, -7.5, 121.75, 83.5]),
        (true, [-20., 30., 40., 60.]),
    ] {
        o.present = present;
        o.bounds = bounds;
        o.events.clear();
        machine.call(DRAWING_BASE + 0x89f60, &[DRAWING_OBJECT, CANVAS]);
        if present {
            assert_eq!(
                o.events,
                [
                    format!("thumbnail:{OBJECT:#x}"),
                    "queue".into(),
                    "width".into(),
                    "height".into(),
                    "buffer".into(),
                    format!("create:17,11,{BUFFER:#x}"),
                    "save:0".into(),
                    "rect".into(),
                    "paint".into(),
                    "antialias:1".into(),
                    format!("draw:{GPU_BITMAP:#x}:{bounds:?}"),
                    "restore:4294967295".into(),
                    format!("release:{GPU_BITMAP:#x}"),
                    "paint_drop".into(),
                ]
            );
        } else {
            assert_eq!(
                o.events,
                [format!("thumbnail:{OBJECT:#x}"), "log".to_owned()]
            );
        }
        cases.push(format!(
            "{{\"present\":{present},\"bounds\":{bounds:?},\"events\":{:?}}}",
            o.events
        ));
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"model_sha256\":\"{LIBRARY_SHA256}\",\"drawing_sha256\":\"{DRAWING_SHA256}\",\"address\":\"0x89f60\",\"cases\":[{}]}}",
        cases.join(",")
    );
    for hook in hooks {
        check(unsafe { uc_hook_del(e, hook) });
    }
}

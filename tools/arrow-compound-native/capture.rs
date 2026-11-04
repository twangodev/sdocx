use super::*;
const EFFECT: u64 = MODEL + 0x1000;
const IMP: u64 = MODEL + 0x2000;
const STYLE: u64 = MODEL + 0x3000;
const PATH: u64 = MODEL + 0x4000;
const VCALL: u64 = MODEL + 0xe000;
#[derive(Default)]
struct R {
    events: Vec<String>,
}
unsafe extern "C" fn hook(e: Engine, a: u64, _: u32, p: *mut c_void) {
    let r = unsafe { &mut *p.cast::<R>() };
    match a - DRAWING_BASE {
        0xbc980 => register(e, REGISTER_X0, 0),
        0xbc840 => {
            r.events.push("convert_stub".into());
            register(e, REGISTER_X0, 1)
        }
        0xbc850 => {
            r.events
                .push(format!("info:{}", read_register(e, REGISTER_X0 + 1)));
        }
        0xbc880 => {
            r.events
                .push(format!("draw:{}", read_register(e, REGISTER_X0 + 1)));
            register(e, REGISTER_X0, 1)
        }
        0xbab30 => r.events.push(format!(
            "width:{:?}",
            f32::from_bits(read_register(e, 136) as u32)
        )),
        0xbab50 | 0xbab80 | 0xbb250 => {}
        _ => unreachable!(),
    }
}
unsafe extern "C" fn vhook(_: Engine, _: u64, _: u32, _: *mut c_void) {}
pub(super) fn capture(m: &mut Machine, drawing: &Path) {
    m.load_drawing(drawing);
    for (p, t) in [
        (0xbc9d0, 0x395638),
        (0xbc9b0, 0x395928),
        (0xbc9c0, 0x395a3c),
        (0xbc980, 0x392fb8),
    ] {
        bind_native(m.engine, DRAWING_BASE + p, t)
    }
    bind_native(m.engine, DRAWING_BASE + 0xbc850, DRAWING_BASE + 0xa0810);
    let mut r = Box::<R>::default();
    let mut hooks = vec![];
    for o in [
        0xbc980, 0xbc840, 0xbc850, 0xbc880, 0xbab30, 0xbab50, 0xbab80, 0xbb250,
    ] {
        let a = DRAWING_BASE + o;
        if o != 0xbc850 {
            write(m.engine, a, &0xd65f03c0_u32.to_le_bytes());
        }
        let mut h = 0;
        check(unsafe {
            uc_hook_add(
                m.engine,
                &mut h,
                4,
                hook as *mut c_void,
                ptr::from_mut(r.as_mut()).cast(),
                a,
                a,
            )
        });
        hooks.push(h)
    }
    write(m.engine, VCALL, &0xd65f03c0_u32.to_le_bytes());
    let mut h = 0;
    check(unsafe {
        uc_hook_add(
            m.engine,
            &mut h,
            4,
            vhook as *mut c_void,
            ptr::null_mut(),
            VCALL,
            VCALL,
        )
    });
    hooks.push(h);
    write(m.engine, EFFECT + 8, &IMP.to_le_bytes());
    write(m.engine, IMP + 24, &STYLE.to_le_bytes());
    write(m.engine, CANVAS + 72, &CANVAS_MATRIX.to_le_bytes());
    write(m.engine, CANVAS_MATRIX, &CANVAS_VTABLE.to_le_bytes());
    for o in [48, 104] {
        write(m.engine, CANVAS_VTABLE + o, &VCALL.to_le_bytes());
    }
    write(m.engine, IMP + 0x35c, &3.5_f32.to_le_bytes());
    for typ in [8_u32, 7] {
        for compound in [0_u32, 1, 2, 3, 4, 255] {
            if typ == 7 && compound == 255 {
                continue;
            }
            write(m.engine, IMP + 0x340, &typ.to_le_bytes());
            write(m.engine, STYLE + 4, &compound.to_le_bytes());
            write(m.engine, STYLE + 20, &1_u32.to_le_bytes());
            r.events.clear();
            let ret = m.call(DRAWING_BASE + 0xa0258, &[EFFECT, CANVAS, PATH, CANVAS]);
            println!("type={typ} compound={compound} ret={ret} {:?}", r.events);
        }
    }
    for h in hooks {
        check(unsafe { uc_hook_del(m.engine, h) });
    }
}

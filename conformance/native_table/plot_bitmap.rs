use super::*;

const HOST: u64 = 0x0300_0000;
const DATA: u64 = MODEL + 0xb000;
const BITMAP: u64 = MODEL + 0xb200;
const FLIPPED: u64 = MODEL + 0xb300;

#[derive(Default)]
struct Observation {
    events: Vec<String>,
}

unsafe extern "C" fn callback(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let o = unsafe { &mut *data.cast::<Observation>() };
    let arg = |i| read_register(engine, REGISTER_X0 + i);
    match (address - HOST) / 32 {
        0 | 1 => {}
        2 => {
            o.events.push(format!("empty:{:#x}", arg(0)));
            register(engine, REGISTER_X0, 0);
        }
        3 => {
            o.events.push(format!("create:{:#x}", arg(0)));
            register(engine, REGISTER_X0, BITMAP);
        }
        4 => {
            o.events.push(format!("flip:{:#x}:{},{}", arg(0), arg(1), arg(2)));
            register(engine, REGISTER_X0, FLIPPED);
        }
        5 => o.events.push(format!("destroy:{:#x}", arg(0))),
        _ => unreachable!(),
    }
}

pub(super) fn capture(machine: &Machine) {
    let e = machine.engine;
    check(unsafe { uc_mem_map(e, HOST, 0x1000, 7) });
    for (plt, n) in [(0x47b1e0, 0), (0x47b1f0, 1), (0x47ad90, 2), (0x47c9b0, 3), (0x47ce40, 4), (0x47c9a0, 5)] {
        bind_native(e, plt, HOST + n * 32);
    }
    let mut o = Box::new(Observation::default());
    for n in 0..6 {
        let addr = HOST + n * 32;
        write(e, addr, &0xd65f03c0_u32.to_le_bytes());
        let mut hook = 0;
        check(unsafe { uc_hook_add(e, &mut hook, 4, callback as *mut c_void, ptr::from_mut(o.as_mut()).cast(), addr, addr) });
    }
    let mut cases = Vec::new();
    for (a, b) in [(0_u8, 0_u8), (1, 0), (0, 1), (1, 1)] {
        write(e, DATA, &[0; 128]);
        write(e, DATA + 0x44, &[a, b]);
        o.events.clear();
        let result = machine.call(0x2b8520, &[DATA]);
        let expected = if a | b == 0 { BITMAP } else { FLIPPED };
        assert_eq!(result, expected);
        assert_eq!(read_u32(e, DATA + 0x40), 1);
        let first = o.events.clone();
        if a | b == 0 {
            assert_eq!(first, [format!("empty:{:#x}", DATA + 0x10), format!("create:{:#x}", DATA + 0x10)]);
        } else {
            assert_eq!(first, [format!("empty:{:#x}", DATA + 0x10), format!("create:{:#x}", DATA + 0x10), format!("flip:{BITMAP:#x}:{b},{a}"), format!("destroy:{BITMAP:#x}")]);
        }
        o.events.clear();
        assert_eq!(machine.call(0x2b8520, &[DATA]), expected);
        assert!(o.events.is_empty());
        assert_eq!(read_u32(e, DATA + 0x40), 2);
        cases.push(format!("{{\"flags\":[{a},{b}],\"events\":{first:?},\"cached_second_call\":true}}"));
    }
    println!("{{\"model_sha256\":\"{LIBRARY_SHA256}\",\"function\":\"0x2b8520\",\"boundary\":\"injected image data; path test, bitmap factory, bitmap destruction and mutex stubbed\",\"cases\":[{}]}}", cases.join(","));
}

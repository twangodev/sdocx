use super::*;

const HOST: u64 = 0x0300_0000;
const DATA: u64 = MODEL + 0xb000;

#[derive(Default)]
struct Observation { events: Vec<String> }

unsafe extern "C" fn callback(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let o = unsafe { &mut *data.cast::<Observation>() };
    let arg = |i| read_register(engine, REGISTER_X0 + i);
    match (address - HOST) / 32 {
        0 => { o.events.push(format!("check:{}", arg(0))); register(engine, REGISTER_X0, 1); }
        1 => o.events.push(format!("remove:{:#x}:{}", arg(0), arg(1) as u32 as i32)),
        2 => { o.events.push(format!("add:{:#x}:{}:{},{},{}", arg(0), arg(1) as u32 as i32, arg(2), arg(3), arg(4))); register(engine, REGISTER_X0, 42); }
        _ => unreachable!(),
    }
}

pub(super) fn capture(machine: &Machine) {
    let e = machine.engine;
    check(unsafe { uc_mem_map(e, HOST, 0x1000, 7) });
    for (plt, n) in [(0x47aea0, 0), (0x47cb70, 1), (0x47cb40, 2)] { bind_native(e, plt, HOST + n * 32); }
    let mut o = Box::new(Observation::default());
    for n in 0..3 {
        let addr = HOST + n * 32;
        write(e, addr, &0xd65f03c0_u32.to_le_bytes());
        let mut hook = 0;
        check(unsafe { uc_hook_add(e, &mut hook, 4, callback as *mut c_void, ptr::from_mut(o.as_mut()).cast(), addr, addr) });
    }
    let mut cases = Vec::new();
    for (media, previous) in [(77_i32, -1_i32), (-7, 9), (0, 0)] {
        write(e, DATA, &[0; 1024]);
        write(e, DATA + 0x300, &media.to_le_bytes());
        write(e, DATA + 0x200, &(DATA + 0x300).to_le_bytes());
        write(e, DATA + 196, &previous.to_le_bytes());
        register(e, REGISTER_X0 + 19, DATA + 0x200);
        register(e, REGISTER_X0 + 23, DATA);
        register(e, REGISTER_X0 + 24, DATA + 0x100);
        register(e, REGISTER_X0 + 29, DATA + 0x400);
        o.events.clear();
        check(unsafe { uc_emu_start(e, 0x2dba24, 0x2dbac0, 1_000_000, 100) });
        assert_eq!(read_register(e, 260), 0x2dbac0);
        assert_eq!(read_u32(e, DATA + 196), 42);
        assert_eq!(read_bytes(e, DATA + 0x200, 8), (DATA + 0x304).to_le_bytes());
        let mut expected = vec!["check:4".to_owned()];
        if previous >= 0 { expected.push(format!("remove:{:#x}:{previous}", DATA + 0x1c0)); }
        expected.push(format!("add:{:#x}:{media}:0,0,0", DATA + 0x1c0));
        assert_eq!(o.events, expected);
        cases.push(format!("{{\"media_id\":{media},\"previous_index\":{previous},\"events\":{:?},\"result_index\":42}}", o.events));
    }
    println!("{{\"model_sha256\":\"{LIBRARY_SHA256}\",\"start\":\"0x2dba24\",\"stop\":\"0x2dbac0\",\"boundary\":\"synthetic four-byte field and base state; buffer check and image registration stubbed; records x0-x4 only, not SIMD registers or rectangle memory\",\"cases\":[{}]}}", cases.join(","));
}

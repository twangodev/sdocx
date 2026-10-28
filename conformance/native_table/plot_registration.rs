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
        3 => o.events.push("android_log".to_owned()),
        4 => o.events.push("set_error_stubbed".to_owned()),
        _ => unreachable!(),
    }
}

unsafe extern "C" fn native_callback(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let o = unsafe { &mut *data.cast::<Observation>() };
    if address == 0x2b548c {
        o.events.push(format!("native_add:{}", read_register(engine, REGISTER_X0 + 1) as u32 as i32));
    } else {
        o.events.push(format!("native_return:{}", read_register(engine, REGISTER_X0) as u32 as i32));
    }
}

pub(super) fn capture(machine: &Machine) {
    let e = machine.engine;
    check(unsafe { uc_mem_map(e, HOST, 0x1000, 7) });
    for (plt, n) in [(0x47aea0, 0), (0x47cb70, 1), (0x47cb40, 2)] { bind_native(e, plt, HOST + n * 32); }
    let native_add = std::env::var_os("SDOCX_PLOT_NATIVE_ADD").is_some();
    if native_add { bind_native(e, 0x47cb40, 0x2b548c); bind_native(e, 0x47ac10, HOST + 96); bind_native(e, 0x47ac20, HOST + 128); }
    let mut o = Box::new(Observation::default());
    for n in 0..5 {
        let addr = HOST + n * 32;
        write(e, addr, &0xd65f03c0_u32.to_le_bytes());
        let mut hook = 0;
        check(unsafe { uc_hook_add(e, &mut hook, 4, callback as *mut c_void, ptr::from_mut(o.as_mut()).cast(), addr, addr) });
    }
    if native_add {
        for address in [0x2b548c, 0x2dba70] {
            let mut hook = 0;
            check(unsafe { uc_hook_add(e, &mut hook, 4, native_callback as *mut c_void, ptr::from_mut(o.as_mut()).cast(), address, address) });
        }
    }
    let mut cases = Vec::new();
    if !native_add {
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
    }
    if native_add {
    write(e, DATA, &[0; 1024]);
    write(e, DATA + 0x300, &(-7_i32).to_le_bytes());
    write(e, DATA + 0x200, &(DATA + 0x300).to_le_bytes());
    write(e, DATA + 196, &(-1_i32).to_le_bytes());
    register(e, REGISTER_X0 + 19, DATA + 0x200);
    register(e, REGISTER_X0 + 23, DATA);
    register(e, REGISTER_X0 + 24, DATA + 0x100);
    register(e, REGISTER_X0 + 29, DATA + 0x400);
    register(e, REGISTER_SP, machine.stack_pointer);
    o.events.clear();
    let result = unsafe { uc_emu_start(e, 0x2dba24, 0x2dbac0, 1_000_000, 1000) };
    assert_eq!(result, 0, "pc={:#x} lr={:#x}", read_register(e, 260), read_register(e, REGISTER_X30));
    assert_eq!(read_register(e, 260), 0x2dbac0);
    assert_eq!(read_u32(e, DATA + 196) as i32, -1);
    assert_eq!(read_bytes(e, DATA + 0x200, 8), (DATA + 0x304).to_le_bytes());
    assert_eq!(o.events, ["check:4", "native_add:-7", "android_log", "set_error_stubbed", "native_return:-1"]);
    cases.push(format!("{{\"media_id\":-7,\"previous_index\":-1,\"events\":{:?},\"field_bytes_consumed\":4,\"result_index\":-1}}", o.events));
    }
    let boundary = if native_add { "synthetic four-byte field and base state; actual AddImage negative-ID path; buffer check, Android log and SPen Error SetError stubbed, so error-state side effect omitted; no saved document or media lookup" } else { "synthetic four-byte field and base state; buffer check and image registration stubbed; records x0-x4 only, not SIMD registers or rectangle memory" };
    println!("{{\"model_sha256\":\"{LIBRARY_SHA256}\",\"start\":\"0x2dba24\",\"stop\":\"0x2dbac0\",\"boundary\":\"{boundary}\",\"cases\":[{}]}}", cases.join(","));
}

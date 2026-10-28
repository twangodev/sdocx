use super::*;

pub(crate) fn capture_named(model: &Path, base: &Path, skia: &Path) {
    verify_host_math();
    let machine = Machine::new(model);
    map_library(machine.engine, base, BASE, BASE_SHA256);
    map_library(machine.engine, skia, SKIA, SKIA_SHA256);
    for (path, address) in [(model, 0), (base, BASE), (skia, SKIA)] {
        reject_unbound_imports(machine.engine, path, address);
    }
    let cxx = model.with_file_name("libc++_shared.so");
    map_library(
        machine.engine,
        &cxx,
        0x0800_0000,
        "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4",
    );
    reject_unbound_imports(machine.engine, &cxx, 0x0800_0000);
    bind_imports(machine.engine);
    for (got, target) in [
        (0x4ac9a0, HOST + 256),
        (BASE + 0xeff10, HOST + 208),
        (BASE + 0xef830, HOST + 240),
        (BASE + 0xef2a8, HOST + 208),
        (BASE + 0xf0570, HOST + 224),
        (BASE + 0xef280, HOST),
        (BASE + 0xef290, HOST + 16),
        (BASE + 0xef310, HOST + 16),
        (BASE + 0xef728, HOST + 208),
        (BASE + 0xef730, HOST + 208),
        (BASE + 0xef738, HOST + 208),
        (BASE + 0xef740, HOST + 208),
        (BASE + 0xf0588, HOST + 80),
        (BASE + 0xf05a0, HOST + 96),
        (0x4a4560, HOST),
        (0x4a4578, HOST + 16),
        (0x4a4580, HOST + 16),
        (0x4a4588, HOST),
        (0x4a45a8, HOST + 16),
    ] {
        write(machine.engine, got, &target.to_le_bytes());
    }
    for (plt, target) in [
        (0x47ac80, 0x080c1a1c),
        (0x47d820, BASE + 0x9dbbc),
        (0x47bed0, BASE + 0x9c868),
        (0x47bef0, BASE + 0x9c928),
        (0x47bf40, BASE + 0x9d608),
        (0x47b460, BASE + 0x9d000),
        (0x47b420, BASE + 0x9d120),
        (0x47c140, BASE + 0x9df90),
        (0x47f240, 0x2ef320),
        (0x47f250, 0x2ef414),
        (0x47f270, 0x2ef584),
        (0x47f280, 0x2ef5d8),
        (0x47f290, 0x2ef630),
        (0x47f3a0, 0x2ef2d8),
        (0x47f3b0, 0x2ef2e0),
        (0x47f3c0, 0x2f1730),
    ] {
        bind_native(machine.engine, plt, target);
    }
    let fill: u8 = std::env::var("ARC_FILL")
        .unwrap_or_else(|_| "0".into())
        .parse()
        .unwrap();
    let mut state = Box::new(HostState {
        fill,
        cursor: HEAP,
        allocations: BTreeMap::new(),
        calls: BTreeMap::new(),
        audit_events: ptr::null_mut(),
        uuid_sequence: 0,
    });
    let _hooks = Hooks::new(machine.engine, state.as_mut());
    let object = MODEL + 0x3000;
    write(machine.engine, object, &[0; 256]);
    machine.call(0x223ffc, &[object]);
    eprintln!(
        "constructed object {:x} impl {:x} arcimpl {:x}",
        object,
        read_u64(machine.engine, object + 8),
        read_u64(machine.engine, object + 16)
    );
    for (i, v) in [10.0f32, 20.0, 210.0, 120.0].iter().enumerate() {
        register(machine.engine, 136 + i as i32, u64::from(v.to_bits()));
    }
    let result = machine.call(0x224cf4, &[object]);
    let imp = read_u64(machine.engine, object + 8);
    eprintln!(
        "result {result} impl {imp:x} data {:x?}",
        bytes(machine.engine, imp, 128)
    );
    for offset in [24, 32] {
        let path = read_u64(machine.engine, imp + offset);
        eprintln!(
            "path {offset} {path:x} {:?}",
            read_model_path(&machine, path)
        );
    }
    for offset in [152, 160] {
        let list = read_u64(machine.engine, imp + offset);
        let count = machine.call(BASE + 0x9d120, &[list]);
        let item = machine.call(BASE + 0x9d608, &[list, 0]);
        eprintln!(
            "fill {offset} count {count} item {item:x} type {} segments {:?}",
            read_u32(machine.engine, item + 8),
            read_model_path(&machine, item)
        );
    }
    for (idx, x, y) in [(0, 20.0f32, 30.0f32), (1, 40.0, 100.0)] {
        register(machine.engine, 136, u64::from(x.to_bits()));
        register(machine.engine, 137, u64::from(y.to_bits()));
        let result = machine.call(0x224628, &[object, idx]);
        eprintln!("control {idx} result {result}");
    }
    for offset in [24, 32] {
        let path = read_u64(machine.engine, imp + offset);
        eprintln!("moved path {offset} {:?}", read_model_path(&machine, path));
    }
    for offset in [152, 160] {
        let list = read_u64(machine.engine, imp + offset);
        let count = machine.call(BASE + 0x9d120, &[list]);
        let item = machine.call(BASE + 0x9d608, &[list, 0]);
        eprintln!(
            "moved fill {offset} count {count} type {} segments {:?}",
            read_u32(machine.engine, item + 8),
            read_model_path(&machine, item)
        );
    }
    eprintln!("calls {:?}", state.calls);
}

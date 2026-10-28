use super::*;

unsafe extern "C" fn trace_plt(_: Engine, address: u64, _: u32, data: *mut c_void) {
    if address % 16 == 0 { unsafe { &mut *data.cast::<std::collections::BTreeSet<u64>>() }.insert(address); }
}
pub(crate) fn capture_named(model: &Path, base: &Path, skia: &Path) {
    verify_host_math();
    let mut machine = Machine::new(model);
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
        (BASE + 0xeff10, BASE + 0xc6a40),
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
    for (plt, target) in [(4696848, 2569628), (4696864, 2569812), (4696880, 2570108), (4696928, 2573328), (4696960, 2574664), (4696976, 2574812), (4696992, 2575008), (4697024, 2575484), (4697056, 2569500), (4697072, 2569508), (4697088, 134981556), (4697120, 67922856), (4697136, 134981436), (4697152, 134981500), (4697168, 134981580), (4697216, 135010844), (4697232, 134981324), (4697248, 2577968), (4697760, 2589860), (4698016, 67906072), (4698336, 67908248), (4698592, 135136784), (4698608, 135136828), (4699168, 67752224), (4699184, 67758228), (4699200, 67753200), (4699216, 67756780), (4699232, 67751936), (4699248, 67752392), (4699264, 67753788), (4699840, 135136552), (4699856, 135136752), (4701904, 67749992), (4701920, 3000780), (4701936, 67750184), (4701968, 67750012), (4702016, 67753480), (4702176, 2967772), (4702448, 3003672), (4702480, 3002260), (4702560, 2924524), (4702576, 3003252), (4702608, 3003336), (4702880, 2759736), (4702912, 2925132), (4702928, 67807816), (4705040, 2838384), (4706656, 67759556), (4706672, 67760316), (4706784, 67768520), (4708384, 67754940), (4708432, 2921132), (4708448, 2921384), (4708464, 2922344), (4708640, 2927200), (4709248, 2936340), (4709728, 2945552), (4709792, 2945920), (4710176, 2953644), (4710192, 2953800), (4710208, 2954016), (4710432, 2956792), (4710464, 2957020), (4710544, 2957712), (4710720, 2960152), (4710736, 2960332), (4710752, 2960664), (4710912, 2962548), (4711008, 2967156), (4711168, 2978100), (4711184, 67807588), (4711232, 67833944), (4711280, 67836428), (4711296, 2978596), (4711376, 67835984), (4711392, 2979060), (4711408, 2979224), (4711840, 2981752), (4712064, 2992252), (4712080, 2992856), (4712096, 2994400), (4712144, 67834092), (4712288, 67759696), (4712384, 67833996), (4712400, 67834012), (4712448, 2978828), (4712560, 2995472), (4712576, 2995908), (4712640, 67807820), (4712672, 67807828), (4712688, 67808476), (4712704, 67834080), (4714304, 67835912), (4714368, 67833616), (4715072, 3076896), (4715088, 3077140), (4715104, 3077420), (4715120, 3077508), (4715136, 3077592), (4715152, 3077680), (4715168, 3078880), (4715184, 3079096), (4715200, 3080168), (4715216, 3081520), (4715296, 3082184), (4715424, 3076824), (4715440, 3076832), (4715456, 3086128), (4715472, 3085540), (4715504, 67833972), (4715552, 3090252), (4715664, 3085348), (4715680, 3086852), (4715712, 3085688), (4715728, 3088372), (4715744, 3088672), (4715776, 3091864), (4723360, 3889292), (4723536, 3898888), (4724016, 3747768), (4724032, 3747956), (4724048, 3757436), (4724064, 3757624), (4724080, 3757812), (4724096, 3758000), (4724112, 3758188), (4724128, 3758376), (4724144, 3758464), (4724160, 3758652), (4724176, 3758740), (4724400, 3887312), (4724480, 3281476), (4724496, 3281624), (4724784, 3281176), (4724816, 67834976), (4725440, 3639516), (4732096, 67836504), (4733600, 3757028), (4733616, 3629352), (4733632, 3627792), (4733680, 3747552), (4734512, 3642804), (4735552, 3625384), (4735584, 3629756), (4735600, 3629948), (4735776, 3642964), (4735968, 3654656), (4735984, 3655116), (4736000, 3656500), (4736016, 3657988), (4736160, 3662728), (4736192, 3663032), (4736288, 3751228), (4736304, 3759476), (4736336, 3751704), (4736352, 3759692), (4736400, 3750104), (4736416, 3750248), (4736432, 3758828), (4736448, 3758916), (4736496, 3899440), (4736512, 3907760), (4736576, 3889816), (4738352, 3747336), (4738368, 3748144), (4738416, 3748892), (4738448, 3749824), (4738464, 3750012), (4738480, 3751504), (4738496, 3760576), (4738528, 3761180), (4738592, 3763272), (4738784, 3765060), (4738928, 3766036), (4739392, 3823088), (4739408, 3855868), (4739616, 67834936), (4739632, 67834956), (4739648, 3828320), (4739680, 3826664), (4739696, 3793440), (4739712, 3818164), (4739888, 3902188), (4740144, 3836848), (4740160, 3837392), (4740176, 3838504), (4740192, 3790232), (4740512, 3795756), (4741664, 3874796), (4741680, 3875104), (4742304, 3879176), (4743248, 3847944), (4743264, 3881168), (4743280, 3862704), (4743376, 3854832), (4743536, 3882724), (4743552, 3901940), (4743568, 3902064), (4743584, 3849340), (4743600, 3850208), (4743696, 3833732), (4743712, 3835508), (4743760, 3840752), (4743792, 3850884), (4743904, 3889052), (4744944, 3898632), (4744960, 3898976)] { if !(0x080ba000..0x080bb000).contains(&target) { bind_native(machine.engine, plt, target); let disp = (target as i64 - plt as i64) / 4; assert_eq!(read_u32(machine.engine,plt), 0x14000000 | ((disp as u32) & 0x03ffffff)); } }
    bind_native(machine.engine, 0x08000000 + 0x132890, HOST + 208);
    bind_native(machine.engine, 0x08000000 + 0x1328d8, HOST + 208);
    for offset in [0x133958, 0x133988, 0x1339a0, 0x1339b8, 0x1339d0] { bind_native(machine.engine, 0x08000000 + offset, HOST + 208); }
    for (i, offset) in [0xef9c8, 0xef9d0, 0xef9d8, 0xef9e0].into_iter().enumerate() { write(machine.engine, BASE + offset, &(HOST + (17 + i as u64) * 16).to_le_bytes()); }
    write(machine.engine, 0x4a44d8, &(HOST + 208).to_le_bytes());
    write(machine.engine, 0x4a48a8, &(HOST + 240).to_le_bytes());
    write(machine.engine, 0x4aca10, &(HOST + 80).to_le_bytes());
    for offset in [0x4a4568, 0x4a4570] { write(machine.engine, offset, &(HOST + 208).to_le_bytes()); }
    write(machine.engine, 0x4a4618, &(HOST + 21 * 16).to_le_bytes());
    write(machine.engine, 0x4a69f0, &(HOST + 22 * 16).to_le_bytes());
    write(machine.engine, BASE + 0xef2c0, &HOST.to_le_bytes());
    let fill: u8 = std::env::var("ARC_FILL")
        .unwrap_or_else(|_| "0".into())
        .parse()
        .unwrap();
    machine.heap.limit = MODEL + 0x90000;
    machine.heap.allocation_fill = fill;
    machine.heap.audit_intervals = true;
    let mut audit_events = Box::new(Vec::<(bool,u64,usize)>::new());
    machine.heap.audit_events = ptr::from_mut(audit_events.as_mut());
    let mut state = Box::new(HostState {
        fill,
        cursor: MODEL + 0x90000,
        allocations: BTreeMap::new(),
        calls: BTreeMap::new(),
        audit_events: ptr::from_mut(audit_events.as_mut()),
        uuid_sequence: 0,
    });
    let _hooks = Hooks::new(machine.engine, state.as_mut());
    let mut reached = Box::new(std::collections::BTreeSet::<u64>::new());
    for (first,last) in [(0x47aa00,0x48bfff),(BASE+0xe5510,BASE+0xe7d30),(SKIA+0x2868d0,SKIA+0x28c060),(0x08000000+0x1326c0,0x08000000+0x1352f0)] {
        let mut trace = 0;
        check(unsafe { uc_hook_add(machine.engine, &mut trace, 4, trace_plt as *mut c_void, ptr::from_mut(reached.as_mut()).cast(), first,last) });
    }
    for address in [0x2a2ed0, 0x2863a0, 0x3c35ec, 0x3c5918] { machine.call(address, &[]); }
    let object = machine.call(0x36d6cc, &[7, 0]);
    eprintln!("factory object {object:x}");
    let typ = machine.call(0x396d74, &[object, 20]);
    eprintln!("set arc {typ}");
    for (i, v) in [10.0f32,20.0,210.0,120.0].iter().enumerate() { register(machine.engine,136+i as i32,u64::from(v.to_bits())); }
    let rect = machine.call(0x3976f8, &[object, 0]);
    eprintln!("set rect {rect}");
    let size = machine.call(0x399a9c, &[object, 2]);
    eprintln!("writer size {size}");
    let input = MODEL + 0x2000;
    let written = machine.call(0x399b40, &[object, input, 2]);
    eprintln!("writer result {written}");
    std::fs::write(std::env::var("ARC_OUTPUT").expect("ARC_OUTPUT"), bytes(machine.engine,input,size as usize)).unwrap();
    let target = machine.call(0x36d6cc, &[7,0]);
    eprintln!("target object {target:x}");
    let result = machine.call(0x399d04, &[target, input, size, 0, 1848]);
    eprintln!("loader consumed {result} / {size}");
    assert_eq!(size, 394);
    assert_eq!(result, size);
    let mut snapshots = Vec::new();
    for object in [object, target] {
        assert_eq!(machine.call(0x396e14,&[object]),20);
        assert_eq!(machine.call(0x397344,&[object]),2);
        assert_eq!(machine.call(0x396fc4,&[object]),1);
        machine.call(0x2caa60,&[object]);
        let bounds: Vec<u32> = (136..140).map(|r| read_register(machine.engine,r) as u32).collect();
        let path = machine.call(0x396f0c,&[object]);
        let outline = active_path(&machine,path);
        let slot = MODEL + 0x1300;
        let fillpath = machine.call(0x397020,&[object,0,slot]);
        assert_eq!(read_u32(machine.engine,slot),1);
        let fill = active_path(&machine,fillpath);
        let mut controls = Vec::new();
        for index in 0..2 { machine.call(0x397240,&[object,index]); controls.push([read_register(machine.engine,136) as u32,read_register(machine.engine,137) as u32]); }
        snapshots.push((bounds,outline,fill,controls));
    }
    assert_eq!(snapshots[0], snapshots[1]);
    eprintln!("ACTIVE {:?}",snapshots[0]);
    let mut live = BTreeMap::new();
    for &(allocated, address, size) in audit_events.iter() {
        if allocated { assert!(live.insert(address,size).is_none(), "duplicate allocation {address:x}"); }
        else { assert!(live.remove(&address).is_some(), "unknown or duplicate free {address:x}"); }
    }
    assert!(!state.calls.contains_key("realloc"));
    eprintln!("LEDGER events {} live {}", audit_events.len(), live.len());
    eprintln!("MACHINE_HEAP allocations {} deletes {} deleted {:x?}", machine.heap.allocations, machine.heap.deletes, machine.heap.deleted_pointers);
    eprintln!("HOST_CALLS {:?}", state.calls);
    eprintln!("reached plt {:x?}", reached);
}

fn active_path(machine: &Machine, path: u64) -> Vec<Vec<u32>> {
    let (_, segments) = read_model_path(machine, path);
    segments.into_iter().map(|s| match s[0] {
        1 | 2 => vec![s[0],s[1],s[2]],
        3 => vec![s[0],s[1],s[2],s[5],s[6]],
        6 => vec![s[0]],
        other => panic!("unexpected path verb {other}"),
    }).collect()
}

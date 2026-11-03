use super::*;

unsafe extern "C" fn trace_plt(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let op = read_u32(engine,address);
    let target = if (op & 0x9f00001f) == 0x90000010 {
        let ldr = read_u32(engine,address+4);
        if ldr & 0xffc003ff != 0xf9400211 { return; }
        let imm = ((op >> 29) & 3) | (((op >> 5) & 0x7ffff) << 2);
        let imm = ((imm as i64) << 43) >> 43;
        let got = ((address & !4095) as i64 + (imm << 12) + (((ldr >> 10) & 0xfff)*8) as i64) as u64;
        read_u64(engine,got)
    } else if (op & 0xfc000000) == 0x14000000 {
        let imm = (((op & 0x03ffffff) as i64) << 38) >> 36;
        (address as i64 + imm) as u64
    } else { return; };
    let reached = unsafe { &mut *data.cast::<std::collections::BTreeMap<u64,u64>>() };
    if let Some(old) = reached.insert(address,target) { assert_eq!(old,target); }
}
unsafe extern "C" fn capture_canvas(engine: Engine, _: u64, _: u32, data: *mut c_void) {
    let calls = unsafe { &mut *data.cast::<Vec<Vec<u8>>>() };
    calls.push(bytes(engine, read_register(engine, REGISTER_X0+1), 64));
    eprintln!("PAINT_BYTES {:?}",bytes(engine,read_register(engine,REGISTER_X0+2),112));
    eprintln!("PAINT caller {:x} color {:08x}",read_register(engine,REGISTER_X30),read_u32(engine,read_register(engine,REGISTER_X0+2)+92));
}
unsafe extern "C" fn capture_dash(engine: Engine, address:u64, _:u32, _: *mut c_void) {
 if address == SKIA+0x263a3c { let p=read_register(engine,REGISTER_X0+1); let n=read_register(engine,REGISTER_X0+2); let vals:Vec<u32>=(0..n).map(|i|read_u32(engine,p+4*i)).collect(); eprintln!("DASH object {:x} intervals {:?} phase {:08x}",read_register(engine,REGISTER_X0),vals,read_register(engine,136)); } else {eprintln!("EFFECT paint {:x} effect {:x}",read_register(engine,REGISTER_X0),read_register(engine,REGISTER_X0+1));}
}
unsafe extern "C" fn capture_cap_join(engine: Engine, address: u64, _: u32, _: *mut c_void) {
    eprintln!("CAPJOIN {:x} {}", address-SKIA, read_register(engine, REGISTER_X0+1));
}
unsafe extern "C" fn capture_width(engine: Engine, _: u64, _: u32, data: *mut c_void) {
    let widths = unsafe { &mut *data.cast::<Vec<u32>>() };
    widths.push(read_register(engine, 136) as u32);
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
    let pencommon = model.with_file_name("libSPenPenCommon.so");
    map_library(machine.engine,&pencommon,0x0a000000,"afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d");
    reject_unbound_imports(machine.engine,&pencommon,0x0a000000);
    let drawing = model.with_file_name("libSPenDrawing.so");
    map_library(machine.engine, &drawing, DRAWING_BASE, DRAWING_SHA256);
    reject_unbound_imports(machine.engine, &drawing, DRAWING_BASE);
    for (got,target) in [(17594504, 2924524), (17595080, 67749992), (17595088, 67750184), (17595328, 3763980), (17595344, 3077592), (17595352, 3077680), (17597840, 102538260), (17597848, 102541516), (17598056, 3757008), (17598064, 3757028), (17598072, 3629352), (17598080, 3757436), (17598088, 102542344), (17598096, 102542396), (17598112, 102542552), (17598128, 102578236), (17598136, 102583952), (17598144, 102584344), (17598152, 102578568), (17598480, 3899412), (17598496, 3763184), (17598928, 17348204), (17598944, 17416132), (17598952, 17429796), (17598960, 17424832), (17598968, 17416932), (17598976, 17430612), (17598992, 102334304), (17599000, 102343236), (17599008, 102342880), (17599016, 102336584), (17599024, 102578460), (17599056, 3747308), (17599064, 3747328), (17599072, 3627792), (17599080, 3747956), (17599088, 3889016), (17599096, 3889044), (17599104, 3890252), (17599112, 3890064), (17599136, 17415460), (17599160, 17418900), (17599168, 17433080), (17599176, 17438484), (17601312, 102584800), (17601336, 102586552), (17601360, 17417332), (17601392, 17419264), (17601408, 17421832), (17601416, 17422232), (17601448, 3763080), (17601472, 3898888), (17601496, 3764256), (17601688, 3764164), (17601744, 3907732), (17601752, 3887284), (17601760, 102540596), (17601816, 17431112), (17601824, 17431788), (17601840, 17434640), (17601864, 17437052), (17601984, 3763732), (17601992, 3747768), (17602000, 3748144), (17602016, 3758376), (17602024, 3758652), (17602032, 3757624), (17602040, 3757812), (17602152, 3758000), (17602160, 102542656), (17602168, 3758188), (17602176, 102542708)] { write(machine.engine,got,&(target as u64).to_le_bytes()); }
    for (offset,target) in [(0xc7620,HOST+208),(0xc7678,HOST+16),(0xc7688,HOST),(0xc76e0,HOST+16),(0xc76e8,HOST)] { write(machine.engine,DRAWING_BASE+offset, &target.to_le_bytes()); }
    for (got,target) in [(17596680, 168086288), (17596688, 168086688)] { write(machine.engine,got,&(target as u64).to_le_bytes()); }
    for (offset,target) in [(0x7a690,HOST+208),(0x7a6b0,HOST+16),(0x7a6b8,HOST),(0x7a978,HOST+16),(0x7aba0,HOST),(0x7b0c0,HOST+64),(0x7b0d0,MEMSET)] { write(machine.engine,0x0a000000+offset,&target.to_le_bytes()); }
    for (got,target) in [(168274000, 168089472), (168274144, 67906072), (168274688, 168089760), (168274696, 168093108), (168274712, 67907872), (168274760, 168090208), (168274784, 168088804), (168274800, 67738424), (168274808, 67738756), (168274816, 134869004), (168274832, 134869308), (168274840, 67738824)] { write(machine.engine,got,&(target as u64).to_le_bytes()); }
    bind_imports(machine.engine);
    bind_native(machine.engine, DRAWING_BASE+0xbaba0, SKIA+0x263a3c);
    bind_native(machine.engine, DRAWING_BASE+0xbab90, SKIA+0x1d0258);
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
    for offset in [0xef4c0,0xef4c8,0xef4d8,0xef4e0,0xef538,0xef540,0xef548] { write(machine.engine,BASE+offset,&(HOST+208).to_le_bytes()); }
    for (i,offset) in [0x7ab80,0x7ab90,0x7ab88].into_iter().enumerate() { write(machine.engine,0x0a000000+offset,&(HOST+(23+i as u64)*16).to_le_bytes()); }
    write(machine.engine,0x0a07a628,&(HOST+208).to_le_bytes());
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
    let mut reached = Box::new(std::collections::BTreeMap::<u64,u64>::new());
    for (first,last) in [(0x47aa00,0x48bfff),(BASE+0xe5510,BASE+0xe7d30),(SKIA+0x2868d0,SKIA+0x28c060),(0x08000000+0x1326c0,0x08000000+0x1352f0),(DRAWING_BASE+0xb8850,DRAWING_BASE+0xbdc30),(0x0a000000+0x711d0,0x0a000000+0x72890)] {
        let mut trace = 0;
        check(unsafe { uc_hook_add(machine.engine, &mut trace, 4, trace_plt as *mut c_void, ptr::from_mut(reached.as_mut()).cast(), first,last) });
    }
    for address in [0x2a2ed0, 0x2863a0, 0x3c35ec, 0x3c5918] { machine.call(address, &[]); }
    let object = machine.call(0x36d6cc, &[7, 0]);
    eprintln!("factory object {object:x}");
    let typ = machine.call(0x396d74, &[object, 20]);
    eprintln!("set arc {typ}");
    let (bounds,fill_color) = match std::env::var("ARC_CASE").as_deref() { Ok("circle") => ([10.0f32,20.0,110.0,120.0],0xff0088ff), Ok("tall") => ([10.0f32,20.0,110.0,220.0],0x80ff0000), _ => ([10.0f32,20.0,210.0,120.0],0xffff0000) };
    for (i, v) in bounds.iter().enumerate() { register(machine.engine,136+i as i32,u64::from(v.to_bits())); }
    let rect = machine.call(0x3976f8, &[object, 0]);
    eprintln!("set rect {rect}");
    let fill_effect=MODEL+0x8000;
    machine.call(0x3b5778,&[fill_effect]);
    let created=machine.call(0x3b5794,&[fill_effect]);
    let color=machine.call(0x3b5be8,&[fill_effect,fill_color]);
    let effect=machine.call(0x396a58,&[object,fill_effect]);
    eprintln!("fill construct {created} color {color} set {effect}");
    let line_color=MODEL+0x9000;
    machine.call(0x392dec,&[line_color]);
    machine.call(0x392e00,&[line_color]);
    let color=u32::from_str_radix(&std::env::var("ARC_COLOR").unwrap_or_else(|_| "ff1166cc".into()),16).unwrap();
    machine.call(0x393010,&[line_color,color as u64]);
    machine.call(0x375660,&[object,line_color]);
    let line_style=MODEL+0xa000;
    machine.call(0x3953d0,&[line_style]);
    machine.call(0x3953e4,&[line_style]);
    let width: f32=std::env::var("ARC_WIDTH").unwrap_or_else(|_| "6".into()).parse().unwrap();
    register(machine.engine,136,width.to_bits() as u64);
    machine.call(0x39550c,&[line_style]);
    let dash: u64=std::env::var("ARC_DASH").unwrap_or_else(|_| "0".into()).parse().unwrap();
    machine.call(0x395690,&[line_style,dash]);
    for (offset, name) in [(0x39574c,"CAP"),(0x395808,"JOIN"),(0x3955d4,"COMPOUND")] { let value: u64=std::env::var(name).unwrap_or_else(|_| "0".into()).parse().unwrap(); machine.call(offset,&[line_style,value]); }
    machine.call(0x375c8c,&[object,line_style]);
    let moved: f32 = std::env::var("ARC_MOVED").unwrap_or_else(|_| "100".into()).parse().unwrap();
    let moved_y: f32 = std::env::var("ARC_MOVED_Y").unwrap_or_else(|_| "20".into()).parse().unwrap();
    for (i, v) in [moved, moved_y].iter().enumerate() { register(machine.engine, 136+i as i32, u64::from(v.to_bits())); }
    let result = machine.call(0x3970d8, &[object,0]);
    eprintln!("moved result {result}");
    if std::env::var("ARC_SECOND").is_ok() {
      let end_x: f32 = std::env::var("ARC_END_X").unwrap_or_else(|_| "40".into()).parse().unwrap();
      let end_y: f32 = std::env::var("ARC_END_Y").unwrap_or_else(|_| "100".into()).parse().unwrap();
      for (i,v) in [end_x,end_y].iter().enumerate() { register(machine.engine,136+i as i32,u64::from(v.to_bits())); }
      eprintln!("second move {}",machine.call(0x3970d8,&[object,1]));
    }
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
    assert!(size > 0);
    assert_eq!(result, size);
    eprintln!("pen got {:x}",read_u64(machine.engine,0x0a07ab70));
    let drawer = MODEL + 0x5000;
    machine.call(DRAWING_BASE + 0x8a1d8, &[drawer]);
    eprintln!("drawing ctor complete");
    let construction = machine.call(DRAWING_BASE + 0x8a264, &[drawer, 0]);
    eprintln!("drawing construct {construction}");
    let objectset = machine.call(DRAWING_BASE + 0x8a3f0, &[drawer, target]);
    eprintln!("drawing set object {objectset}");
    let mut widths = Box::new(Vec::<u32>::new());
    let mut width_hook = 0;
    check(unsafe { uc_hook_add(machine.engine, &mut width_hook, 4, capture_width as *mut c_void, ptr::from_mut(widths.as_mut()).cast(), SKIA+0x1cacd8, SKIA+0x1cacd8) });
    let mut dash_hooks=Vec::new();
    for address in [SKIA+0x263a3c,SKIA+0x1d0258] { let mut h=0; check(unsafe {uc_hook_add(machine.engine,&mut h,4,capture_dash as *mut c_void,ptr::null_mut::<c_void>(),address,address)}); dash_hooks.push(h); }
    for address in [SKIA+0x1cad40, SKIA+0x1cad74] { let mut h=0; check(unsafe {uc_hook_add(machine.engine,&mut h,4,capture_cap_join as *mut c_void,ptr::null_mut::<c_void>(),address,address)}); }
    let effects = machine.call(DRAWING_BASE+0x8a4d0,&[drawer]);
    eprintln!("WIDTH_SETTERS {:?}", widths);
    eprintln!("drawing set effects {effects}");
    for offset in [0xc8a10,0xc8a18,0xc8a20,0xc8a28] { write(machine.engine,DRAWING_BASE+offset,&(HOST+208).to_le_bytes()); }
    let bitmap = MODEL+0x6000; let canvas=MODEL+0x6100;
    let vtable=MODEL+0x6200; let callback=MODEL+0x35000;
    let mut calls=Box::new(Vec::<Vec<u8>>::new());
    write(machine.engine,callback,&0xd65f03c0_u32.to_le_bytes());
    let mut hook=0;
    check(unsafe { uc_hook_add(machine.engine,&mut hook,4,capture_canvas as *mut c_void,ptr::from_mut(calls.as_mut()).cast(),callback,callback) });
    write(machine.engine,canvas,&vtable.to_le_bytes());
    write(machine.engine,vtable+96,&callback.to_le_bytes());
    write(machine.engine,bitmap,&[0;128]); write(machine.engine,bitmap+72,&canvas.to_le_bytes());
    for (i,v) in [1.0f32,10.0,20.0,210.0,120.0].iter().enumerate() { register(machine.engine,136+i as i32, u64::from(v.to_bits())); }
    let drawn = machine.call(DRAWING_BASE+0x8a7e4,&[drawer,bitmap]);
    eprintln!("drawing inner drawPath {drawn}; canvas calls {}", calls.len());
    for (i,path) in calls.iter().enumerate() {
        let object = MODEL+0x7000; let iter=MODEL+0x7100; let output=MODEL+0x7200;
        write(machine.engine,object,path);
        let points=machine.call(SKIA+0x1d4c20,&[object]); let verbs=machine.call(SKIA+0x1d4c90,&[object]);
        machine.call(SKIA+0x1d855c,&[iter]); machine.call(SKIA+0x1d858c,&[iter,object]);
        let mut record=Vec::new();
        for _ in 0..40 { let verb=machine.call(SKIA+0x1d41ac,&[iter,output]); let n=match verb {0=>1,1=>2,2|3=>3,4=>4,5|6=>0,_=>panic!("verb {verb}")}; record.push((verb,(0..n*2).map(|i| read_u32(machine.engine,output+i*4)).collect::<Vec<_>>())); if verb==6 {break} }
        eprintln!("canvas {i} points {points} verbs {verbs} records {record:?}");
    }
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
    assert_eq!(snapshots[0].0,snapshots[1].0);
    assert_eq!(snapshots[0].1,snapshots[1].1);
    assert_eq!(snapshots[0].2,snapshots[1].2);
    eprintln!("SOURCE_CONTROLS {:?}", snapshots[0].3);
    eprintln!("ACTIVE {:?}",snapshots[1]);
    let mut live = BTreeMap::new();
    for &(allocated, address, size) in audit_events.iter() {
        if allocated { assert!(live.insert(address,size).is_none(), "duplicate allocation {address:x}"); }
        else { assert!(live.remove(&address).is_some(), "unknown or duplicate free {address:x}"); }
    }

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

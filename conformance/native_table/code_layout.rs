use super::*;
use frames::{BASE, BASE_SHA256};

const LAYOUT: u64 = MODEL + 0x10000;
const OBJECT: u64 = MODEL + 0x11000;
const OBJECT_IMPL: u64 = OBJECT + 256;
const OBJECT_VTABLE: u64 = MODEL + 0x12000;
const CHILD_VTABLE: u64 = MODEL + 0x13000;
const TEXT_VTABLE: u64 = MODEL + 0x14000;
const TITLE: u64 = MODEL + 0x15000;
const BODY: u64 = MODEL + 0x16000;
const TITLE_TEXT: u64 = MODEL + 0x17000;
const BODY_TEXT: u64 = MODEL + 0x18000;
const SPLITS: u64 = MODEL + 0x19000;
const HOST: u64 = 0x0300_0800;
const GET_BOUNDS: u64 = HOST;
const SET_RECT: u64 = HOST + 16;
const SET_LAYOUT: u64 = HOST + 32;
const UPDATE: u64 = HOST + 48;
const MEASURE_CHILD: u64 = HOST + 64;
const GET_TEXT_LAYOUT: u64 = HOST + 80;
const GET_HEIGHT: u64 = HOST + 96;
const GET_FIRST_LINE: u64 = HOST + 112;
const SET_PADDING: u64 = HOST + 128;

#[derive(Default, Debug, Clone, PartialEq)]
struct Child {
    frame: Option<[f32; 4]>,
    layout_frame: Option<[f32; 4]>,
    padding: Vec<[f32; 4]>,
    updates: usize,
    measures: usize,
    height_calls: usize,
    first_line_calls: usize,
}

struct Inputs {
    bounds: [f32; 4],
    title_height: f32,
    body_height: f32,
    first_line: f32,
    bounds_calls: usize,
    children: [Child; 2],
}

fn rect(engine: Engine, address: u64) -> [f32; 4] {
    std::array::from_fn(|axis| read_float(engine, address + axis as u64 * 4))
}

fn write_rect(engine: Engine, address: u64, rectangle: [f32; 4]) {
    for (axis, value) in rectangle.into_iter().enumerate() {
        assert!(value.is_finite());
        write(engine, address + axis as u64 * 4, &value.to_le_bytes());
    }
}

fn float_return(engine: Engine, values: &[f32]) {
    for (axis, value) in values.iter().enumerate() {
        register(engine, 136 + axis as i32, u64::from(value.to_bits()));
    }
}

unsafe extern "C" fn supplied_interface(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let inputs = unsafe { &mut *data.cast::<Inputs>() };
    let receiver = read_register(engine, REGISTER_X0);
    match address {
        GET_BOUNDS => {
            assert_eq!(receiver, OBJECT);
            inputs.bounds_calls += 1;
            float_return(engine, &inputs.bounds);
        }
        GET_HEIGHT | GET_FIRST_LINE => {
            let index = match receiver {
                TITLE_TEXT => 0,
                BODY_TEXT => 1,
                _ => panic!("unknown text layout {receiver:x}"),
            };
            assert_eq!(read_register(engine, REGISTER_X0 + 1), 0);
            if address == GET_FIRST_LINE {
                inputs.children[index].first_line_calls += 1;
                float_return(engine, &[inputs.first_line]);
            } else {
                inputs.children[index].height_calls += 1;
                float_return(
                    engine,
                    &[if index == 0 {
                        inputs.title_height
                    } else {
                        inputs.body_height
                    }],
                );
            }
        }
        _ => {
            let index = match receiver {
                TITLE => 0,
                BODY => 1,
                _ => panic!("unknown text child {receiver:x}"),
            };
            let child = &mut inputs.children[index];
            match address {
                SET_RECT => {
                    child.frame = Some(std::array::from_fn(|axis| {
                        f32::from_bits(read_register(engine, 136 + axis as i32) as u32)
                    }));
                }
                SET_LAYOUT => {
                    child.layout_frame = Some(rect(engine, read_register(engine, REGISTER_X0 + 1)));
                }
                UPDATE => child.updates += 1,
                MEASURE_CHILD => child.measures += 1,
                GET_TEXT_LAYOUT => {
                    assert_eq!(read_register(engine, REGISTER_X0 + 1), 0);
                    assert_eq!(read_register(engine, REGISTER_X0 + 2), 0);
                    register(
                        engine,
                        REGISTER_X0,
                        if index == 0 { TITLE_TEXT } else { BODY_TEXT },
                    );
                }
                SET_PADDING => {
                    let list = read_register(engine, REGISTER_X0 + 1);
                    // Observe actual native list nodes without reentering the guest emulator.
                    let implementation = read_u64(engine, list + 8);
                    let count = read_u32(engine, implementation + 16) as usize;
                    assert!(count <= 8);
                    let mut node = read_u64(engine, implementation);
                    child.padding.clear();
                    for _ in 0..count {
                        assert!((HEAP..TLS).contains(&node), "invalid list node {node:x}");
                        child
                            .padding
                            .push(rect(engine, read_u64(engine, node + 16)));
                        node = read_u64(engine, node + 8);
                    }
                }
                _ => panic!("unhandled interface {address:x}"),
            }
        }
    }
}

struct Interfaces {
    engine: Engine,
    hooks: Vec<usize>,
    inputs: Box<Inputs>,
}

impl Interfaces {
    fn new(machine: &Machine, inputs: Inputs) -> Self {
        let mut this = Self {
            engine: machine.engine,
            hooks: Vec::new(),
            inputs: Box::new(inputs),
        };
        for address in (HOST..=SET_PADDING).step_by(16) {
            write(machine.engine, address, &0xd65f03c0_u32.to_le_bytes());
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    supplied_interface as *mut c_void,
                    ptr::from_mut(this.inputs.as_mut()).cast(),
                    address,
                    address,
                )
            });
            this.hooks.push(hook);
        }
        this
    }
}

impl Drop for Interfaces {
    fn drop(&mut self) {
        for hook in &self.hooks {
            check(unsafe { uc_hook_del(self.engine, *hook) });
        }
    }
}

fn child_json(child: &Child) -> String {
    format!(
        "{{\"frame\":{},\"layout_frame\":{},\"padding\":{:?},\"updates\":{},\"measures\":{},\"height_calls\":{},\"first_line_calls\":{}}}",
        child.frame.map_or("null".to_owned(), |r| format!("{r:?}")),
        child
            .layout_frame
            .map_or("null".to_owned(), |r| format!("{r:?}")),
        child.padding,
        child.updates,
        child.measures,
        child.height_calls,
        child.first_line_calls
    )
}

fn snapshot(machine: &Machine, interfaces: &Interfaces) -> String {
    machine.call(DRAWING_BASE + 0x738f4, &[LAYOUT]);
    let first_page = f32::from_bits(read_register(machine.engine, 136) as u32);
    format!(
        "{{\"copy\":{:?},\"title\":{:?},\"body\":{:?},\"measured\":{:?},\"content\":{:?},\"header_delta\":{:?},\"minimum_first_page_height\":{first_page:?},\"dirty\":{},\"bounds_calls\":{},\"title_child\":{},\"body_child\":{}}}",
        rect(machine.engine, LAYOUT + 592),
        rect(machine.engine, LAYOUT + 608),
        rect(machine.engine, LAYOUT + 624),
        rect(machine.engine, LAYOUT + 560),
        rect(machine.engine, LAYOUT + 576),
        read_float(machine.engine, LAYOUT + 640),
        read_u32(machine.engine, LAYOUT + 720) & 255,
        interfaces.inputs.bounds_calls,
        child_json(&interfaces.inputs.children[0]),
        child_json(&interfaces.inputs.children[1])
    )
}

fn fixture(
    machine: &mut Machine,
    name: &str,
    bounds: [f32; 4],
    density: f32,
    bands: &[[f32; 4]],
    title_height: f32,
    body_height: f32,
    has_body: bool,
) -> String {
    write(machine.engine, LAYOUT, &[0; 1024]);
    for address in [OBJECT, TITLE, BODY, TITLE_TEXT, BODY_TEXT] {
        write(machine.engine, address, &[0; 2048]);
    }
    machine.heap.cursor = HEAP;
    write(machine.engine, OBJECT, &OBJECT_VTABLE.to_le_bytes());
    write(machine.engine, OBJECT + 32, &OBJECT_IMPL.to_le_bytes());
    let body_pointer = if has_body { BODY } else { 0 };
    write(
        machine.engine,
        OBJECT_IMPL + 24,
        &body_pointer.to_le_bytes(),
    );
    assert_eq!(machine.call(0x474570, &[OBJECT]), body_pointer);
    write(
        machine.engine,
        OBJECT_VTABLE + 168,
        &GET_BOUNDS.to_le_bytes(),
    );
    for address in [TITLE, BODY] {
        write(machine.engine, address, &CHILD_VTABLE.to_le_bytes());
    }
    for (offset, function) in [
        (336, SET_RECT),
        (392, SET_LAYOUT),
        (376, MEASURE_CHILD),
        (952, GET_TEXT_LAYOUT),
    ] {
        write(
            machine.engine,
            CHILD_VTABLE + offset,
            &function.to_le_bytes(),
        );
    }
    for address in [TITLE_TEXT, BODY_TEXT] {
        write(machine.engine, address, &TEXT_VTABLE.to_le_bytes());
    }
    write(machine.engine, TEXT_VTABLE + 16, &GET_HEIGHT.to_le_bytes());
    write(
        machine.engine,
        TEXT_VTABLE + 224,
        &GET_FIRST_LINE.to_le_bytes(),
    );
    write(machine.engine, LAYOUT + 528, &OBJECT.to_le_bytes());
    write(machine.engine, LAYOUT + 672, &TITLE.to_le_bytes());
    write(machine.engine, LAYOUT + 680, &BODY.to_le_bytes());
    for (offset, value) in [
        (688, 24.0),
        (692, 12.0),
        (696, 16.0),
        (700, 12.0),
        (704, 16.0),
        (708, 12.0),
        (712, 12.0),
        (716, 8.0),
    ] {
        write(
            machine.engine,
            LAYOUT + offset,
            &(value * density).to_le_bytes(),
        );
    }
    write(machine.engine, LAYOUT + 720, &[1]);
    write(machine.engine, LAYOUT + 648, &SPLITS.to_le_bytes());
    write(
        machine.engine,
        LAYOUT + 656,
        &(SPLITS + bands.len() as u64 * 16).to_le_bytes(),
    );
    for (index, band) in bands.iter().enumerate() {
        write_rect(machine.engine, SPLITS + index as u64 * 16, *band);
    }
    let mut interfaces = Interfaces::new(
        machine,
        Inputs {
            bounds,
            title_height,
            body_height,
            first_line: 17.25,
            bounds_calls: 0,
            children: std::array::from_fn(|_| Child::default()),
        },
    );
    machine.call(DRAWING_BASE + 0x732fc, &[LAYOUT]);
    let cold = snapshot(machine, &interfaces);
    for (value, delta) in interfaces
        .inputs
        .bounds
        .iter_mut()
        .zip([17.0, 25.0, 111.0, 1000.0])
    {
        *value += delta;
    }
    interfaces.inputs.title_height += 50.0;
    interfaces.inputs.body_height += 100.0;
    machine.call(DRAWING_BASE + 0x732fc, &[LAYOUT]);
    let warm = snapshot(machine, &interfaces);
    assert_eq!(interfaces.inputs.children[0].measures, 1);
    assert_eq!(
        interfaces.inputs.children[1].measures,
        usize::from(has_body)
    );
    machine.call(DRAWING_BASE + 0x737d0, &[LAYOUT]);
    machine.call(DRAWING_BASE + 0x732fc, &[LAYOUT]);
    let cleared = snapshot(machine, &interfaces);
    format!(
        "{{\"name\":{name:?},\"bounds\":{bounds:?},\"density\":{density:?},\"split_rectangles\":{bands:?},\"supplied_title_height\":{title_height:?},\"supplied_body_height\":{body_height:?},\"supplied_body_present\":{has_body},\"supplied_first_line_height\":17.25,\"warm_input_mutation\":{{\"source_bounds_delta\":[17,25,111,1000],\"title_height_delta\":50,\"body_height_delta\":100}},\"cold\":{cold},\"warm\":{warm},\"cleared\":{cleared}}}"
    )
}

pub(super) fn capture(machine: &mut Machine, base: &Path) {
    frames::load_base(machine, base);
    splits::load_lists(machine);
    for (plt, target) in [
        (0xb8b10, BASE + 0xb108c),
        (0xb8b20, BASE + 0xb109c),
        (0xb9280, BASE + 0xb11a4),
        (0xb8960, BASE + 0xbe518),
        (0xb9330, DRAWING_BASE + 0x73694),
        (0xb92b0, 0x474570),
        (0xb93b0, BASE + 0x9c87c),
        (0xb93e0, SET_PADDING),
        (0xb93f0, UPDATE),
    ] {
        bind_native(machine.engine, DRAWING_BASE + plt, target);
    }
    let mut cases = Vec::new();
    for (name, density, bounds, bands) in [
        ("empty", 1.0, [0.0, 0.0, 240.0, 200.0], vec![]),
        (
            "touching",
            1.0,
            [0.0, 0.0, 240.0, 200.0],
            vec![[0.0, 36.0, 240.0, 56.0]],
        ),
        (
            "intersecting",
            1.0,
            [0.0, 0.0, 240.0, 200.0],
            vec![[0.0, 35.5, 240.0, 56.0]],
        ),
        (
            "inside-header",
            1.0,
            [0.0, 0.0, 240.0, 200.0],
            vec![[0.0, 20.0, 240.0, 56.0]],
        ),
        (
            "after-header",
            1.0,
            [0.0, 0.0, 240.0, 200.0],
            vec![[0.0, 80.0, 240.0, 100.0]],
        ),
        (
            "zero-top",
            1.0,
            [0.0, 0.0, 240.0, 200.0],
            vec![[0.0, 0.0, 240.0, 20.0]],
        ),
        (
            "negative-top",
            1.0,
            [0.0, 0.0, 240.0, 200.0],
            vec![[0.0, -5.0, 240.0, 20.0]],
        ),
        (
            "sorted",
            1.0,
            [0.0, 0.0, 240.0, 200.0],
            vec![[0.0, 20.0, 240.0, 40.0], [0.0, 80.0, 240.0, 100.0]],
        ),
        (
            "unsorted",
            1.0,
            [0.0, 0.0, 240.0, 200.0],
            vec![[0.0, 80.0, 240.0, 100.0], [0.0, 20.0, 240.0, 40.0]],
        ),
        (
            "translated",
            1.0,
            [13.25, 91.5, 253.25, 291.5],
            vec![[0.0, 20.0, 240.0, 56.0]],
        ),
        (
            "fractional-density",
            1.375,
            [0.0, 0.0, 241.25, 200.0],
            vec![[7.0, 20.125, 199.0, 56.5]],
        ),
        (
            "double-density",
            2.0,
            [0.0, 0.0, 480.0, 400.0],
            vec![[0.0, 71.5, 480.0, 112.0]],
        ),
        (
            "narrow",
            1.0,
            [0.0, 0.0, 31.5, 200.0],
            vec![[0.0, 20.0, 31.5, 56.0]],
        ),
    ] {
        let expected = fixture(machine, name, bounds, density, &bands, 91.25, 43.75, true);
        for fill in [0xa5, 0xff] {
            machine.heap.allocation_fill = fill;
            assert_eq!(
                fixture(machine, name, bounds, density, &bands, 91.25, 43.75, true),
                expected
            );
        }
        machine.heap.allocation_fill = 0;
        cases.push(expected);
    }
    for (name, top, title_height, body_height, has_body) in [
        (
            "one-ulp-below-header",
            f32::from_bits(36.0_f32.to_bits() - 1),
            91.25,
            43.75,
            true,
        ),
        (
            "one-ulp-above-header",
            f32::from_bits(36.0_f32.to_bits() + 1),
            91.25,
            43.75,
            true,
        ),
        ("short-title", 20.0, 1.0, 43.75, true),
        ("empty-body", 20.0, 91.25, 0.0, true),
        ("absent-body", 20.0, 91.25, 43.75, false),
    ] {
        let bands = [[300.0, top, 400.0, top + 20.0]];
        let expected = fixture(
            machine,
            name,
            [0.0, 0.0, 240.0, 200.0],
            1.0,
            &bands,
            title_height,
            body_height,
            has_body,
        );
        for fill in [0xa5, 0xff] {
            machine.heap.allocation_fill = fill;
            assert_eq!(
                fixture(
                    machine,
                    name,
                    [0.0, 0.0, 240.0, 200.0],
                    1.0,
                    &bands,
                    title_height,
                    body_height,
                    has_body
                ),
                expected
            );
        }
        machine.heap.allocation_fill = 0;
        cases.push(expected);
    }
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"drawing_library_sha256\":\"{DRAWING_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"measure_address\":\"Drawing+0x732fc\",\"measured_object_address\":\"Drawing+0x73694\",\"minimum_first_page_address\":\"Drawing+0x738f4\",\"allocation_fills\":[0,165,255],\"capture_boundary\":\"Complete native Measure, measuredObject, ClearMeasure and GetMinHeightInFirstPage execute. Native Base RectF Width/Height/OffSet, Matrix constructor and List construction/add/traversal/destruction execute. Native Model ObjectCodeBlock GetBody executes on supplied implementation fields. Supplied host interfaces are source bounds, child SetRect/SetLayout/Update/Measure and supplied TextLayout GetHeight(false)/first-line-height. The child interfaces only observe actual native frames and translated split lists; all output geometry is computed by native instructions. Constants are supplied from independently recovered Content constant records multiplied by case density; native object/layout construction, native constant resolution, actual child shaping/wrapping/margins, upstream constraint0/1/2 frame selection, compositor split generation, full nested code/table pagination and final drawing do not execute. Warm Measure receives mutated input bounds and title/body heights without ClearMeasure; cleared Measure consumes those inputs.\",\"cases\":[\n{}\n]}}",
        cases.join(",\n")
    );
}

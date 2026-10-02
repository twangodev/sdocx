use super::*;

const COMPOSER: u64 = 0x0600_0000;
const COMPOSER_SHA256: &str = "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f";
const DRAWN: u64 = MODEL + 0x18000;
const VECTOR: u64 = MODEL + 0x19000;
const WRITER: u64 = MODEL + 0x1a000;
const SHAPE: u64 = MODEL + 0x1b000;

#[derive(Clone, Copy)]
struct Input {
    present: bool,
    object_flag: u8,
    foreground: u32,
    background: u32,
}

impl Input {
    fn write(self, engine: Engine, fill: u8) {
        write(engine, MODEL, &vec![fill; 0x100000]);
        write(engine, DRAWN, &[0; 160]);
        write(engine, DRAWN + 120, &[self.object_flag]);
        write(engine, DRAWN + 136, &self.foreground.to_le_bytes());
        write(engine, DRAWN + 144, &self.background.to_le_bytes());
        for (axis, value) in [2.0_f32, 3.0, 7.0, 11.0].into_iter().enumerate() {
            write(engine, DRAWN + 104 + axis as u64 * 4, &value.to_le_bytes());
        }
        write(engine, VECTOR, &self.pointer().to_le_bytes());
        write(engine, STACK, &[0; 512]);
        for index in 0..29 {
            register(engine, REGISTER_X0 + index, 0);
        }
        register(engine, REGISTER_SP, STACK);
        register(engine, REGISTER_X30, STOP);
        register(engine, REGISTER_X0 + 19, WRITER);
        register(engine, 1, STACK + 128);
    }

    fn pointer(self) -> u64 {
        if self.present { DRAWN } else { 0 }
    }

    fn json(self) -> String {
        format!(
            "\"present\":{},\"object_flag\":{},\"foreground_argb\":{},\"background_argb\":{},\"layout_rect\":[2.0,3.0,7.0,11.0]",
            self.present, self.object_flag, self.foreground, self.background
        )
    }
}

#[derive(Clone, Copy)]
struct NativeCall {
    address: u64,
    name: &'static str,
    argument: i32,
}

struct Window {
    name: &'static str,
    start: u64,
    exits: Vec<(u64, &'static str)>,
    registers: Vec<(i32, u64)>,
    calls: Vec<NativeCall>,
    object_filtered: bool,
}

impl Window {
    fn caller(
        name: &'static str,
        start: u64,
        end: u64,
        vector_register: i32,
        plt: u64,
        argument: i32,
        object_filtered: bool,
    ) -> Self {
        Self {
            name,
            start,
            exits: vec![(end, "iteration-end")],
            registers: vec![(vector_register, VECTOR)],
            calls: vec![NativeCall {
                address: plt,
                name: "background",
                argument,
            }],
            object_filtered,
        }
    }

    fn gate(
        name: &'static str,
        start: u64,
        accepted: u64,
        rejected: u64,
        drawn_register: i32,
    ) -> Self {
        Self {
            name,
            start,
            exits: vec![(accepted, "alpha-accepted"), (rejected, "alpha-rejected")],
            registers: vec![(drawn_register, DRAWN)],
            calls: vec![],
            object_filtered: false,
        }
    }

    fn fixture(&self, machine: &Machine, input: Input, fill: u8) -> String {
        input.write(machine.engine, fill);
        for &(index, value) in &self.registers {
            let value = if value == DRAWN {
                input.pointer()
            } else {
                value
            };
            register(machine.engine, REGISTER_X0 + index, value);
        }
        let recorder = Recorder::new(machine, self, input.pointer());
        let error =
            unsafe { uc_emu_start(machine.engine, COMPOSER + self.start, STOP, 1_000_000, 1000) };
        assert_eq!(
            error,
            0,
            "{} failed at PC {:x}",
            self.name,
            read_register(machine.engine, 260)
        );
        assert_eq!(read_register(machine.engine, 260), STOP);
        let observation = recorder.observation.as_ref();
        let exit = observation.exit.expect("native window exit");
        if self.calls.len() == 1 {
            let expected = input.present && !(self.object_filtered && input.object_flag != 0);
            assert_eq!(
                observation.calls.len(),
                usize::from(expected),
                "{}",
                self.name
            );
        } else if self.calls.len() == 2 {
            let expected = if !input.present {
                vec![]
            } else if input.object_flag != 0 {
                vec!["object"]
            } else {
                vec!["text"]
            };
            assert_eq!(observation.calls, expected, "{}", self.name);
        } else {
            let color = if self.name == "shared-exporter-foreground-alpha" {
                input.foreground
            } else {
                input.background
            };
            let expected = if color >> 24 == 0 {
                "alpha-rejected"
            } else {
                "alpha-accepted"
            };
            assert_eq!(exit, expected, "{}", self.name);
        }
        let calls: Vec<_> = observation
            .calls
            .iter()
            .map(|call| format!("{call:?}"))
            .collect();
        format!(
            "{{\"window\":{:?},{},\"exit\":{exit:?},\"calls\":[{}]}}",
            self.name,
            input.json(),
            calls.join(",")
        )
    }

    fn json(&self) -> String {
        let exits: Vec<_> = self
            .exits
            .iter()
            .map(|(address, name)| format!("{{\"address\":\"0x{address:x}\",\"name\":{name:?}}}"))
            .collect();
        let calls: Vec<_> = self
            .calls
            .iter()
            .map(|call| {
                format!(
                    "{{\"plt\":\"0x{:x}\",\"name\":{:?},\"drawn_argument\":{}}}",
                    call.address, call.name, call.argument
                )
            })
            .collect();
        let registers: Vec<_> = self
            .registers
            .iter()
            .map(|&(index, value)| format!("[{index},{value}]"))
            .collect();
        format!(
            "{{\"name\":{:?},\"start\":\"0x{:x}\",\"exits\":[{}],\"supplied_registers\":[{}],\"intercepted_calls\":[{}]}}",
            self.name,
            self.start,
            exits.join(","),
            registers.join(","),
            calls.join(",")
        )
    }
}

struct Observation {
    exits: Vec<(u64, &'static str)>,
    imports: Vec<NativeCall>,
    pointer: u64,
    calls: Vec<&'static str>,
    exit: Option<&'static str>,
}

unsafe extern "C" fn observe(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let observation = unsafe { &mut *data.cast::<Observation>() };
    if let Some((_, name)) = observation.exits.iter().find(|(exit, _)| *exit == address) {
        observation.exit = Some(*name);
        register(engine, 260, STOP);
    } else {
        let call = observation
            .imports
            .iter()
            .find(|call| call.address == address)
            .expect("intercepted native call");
        assert_eq!(
            read_register(engine, REGISTER_X0 + call.argument),
            observation.pointer
        );
        observation.calls.push(call.name);
    }
}

struct Recorder {
    engine: Engine,
    hooks: Vec<usize>,
    observation: Box<Observation>,
}

impl Recorder {
    fn new(machine: &Machine, window: &Window, pointer: u64) -> Self {
        let mut recorder = Self {
            engine: machine.engine,
            hooks: vec![],
            observation: Box::new(Observation {
                exits: window
                    .exits
                    .iter()
                    .map(|&(address, name)| (COMPOSER + address, name))
                    .collect(),
                imports: window
                    .calls
                    .iter()
                    .map(|call| NativeCall {
                        address: COMPOSER + call.address,
                        ..*call
                    })
                    .collect(),
                pointer,
                calls: vec![],
                exit: None,
            }),
        };
        for call in &recorder.observation.imports {
            write(machine.engine, call.address, &0xd65f03c0_u32.to_le_bytes());
        }
        let addresses: Vec<_> = recorder
            .observation
            .exits
            .iter()
            .map(|&(address, _)| address)
            .chain(recorder.observation.imports.iter().map(|call| call.address))
            .collect();
        for address in addresses {
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    machine.engine,
                    &mut hook,
                    4,
                    observe as *mut c_void,
                    ptr::from_mut(recorder.observation.as_mut()).cast(),
                    address,
                    address,
                )
            });
            recorder.hooks.push(hook);
        }
        recorder
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        for hook in &self.hooks {
            check(unsafe { uc_hook_del(self.engine, *hook) });
        }
    }
}

fn windows() -> Vec<Window> {
    let mut windows = vec![
        Window::caller(
            "body-exporter-background",
            0x349df8,
            0x349e34,
            24,
            0x5506c0,
            1,
            true,
        ),
        Window::caller(
            "body-writer-background",
            0x375ee8,
            0x375f24,
            23,
            0x552e50,
            1,
            true,
        ),
        Window::caller(
            "table-exporter-background",
            0x3507d0,
            0x3507e4,
            22,
            0x550cd0,
            1,
            false,
        ),
        Window::caller(
            "table-writer-background",
            0x37ef38,
            0x37ef50,
            21,
            0x5534d0,
            2,
            false,
        ),
        Window::caller(
            "code-title-background",
            0x378b28,
            0x378b40,
            21,
            0x5530e0,
            2,
            false,
        ),
        Window::caller(
            "code-body-background",
            0x378ff0,
            0x379008,
            21,
            0x5530e0,
            2,
            false,
        ),
        Window::caller(
            "placed-background",
            0x380790,
            0x3807a4,
            20,
            0x553650,
            1,
            false,
        ),
        Window::gate(
            "body-writer-background-alpha",
            0x376230,
            0x376254,
            0x3762cc,
            1,
        ),
        Window::gate(
            "table-writer-background-alpha",
            0x37f354,
            0x37f374,
            0x37f4d4,
            20,
        ),
        Window::gate(
            "code-writer-background-alpha",
            0x37978c,
            0x3797ac,
            0x379914,
            21,
        ),
        Window::gate(
            "placed-writer-background-alpha",
            0x380ba0,
            0x380bc0,
            0x380d1c,
            20,
        ),
        Window::gate(
            "shared-exporter-foreground-alpha",
            0x355150,
            0x355184,
            0x35516c,
            0,
        ),
    ];
    for (name, start, end, vector_register, object_plt, text_plt) in [
        (
            "body-exporter-dispatch",
            0x349e4c,
            0x349ee4,
            26,
            0x5506d0,
            0x5506e0,
        ),
        (
            "body-writer-dispatch",
            0x375f3c,
            0x375fdc,
            25,
            0x552e60,
            0x552e70,
        ),
    ] {
        windows.push(Window {
            name,
            start,
            exits: vec![(end, "iteration-end")],
            registers: vec![(vector_register, VECTOR)],
            calls: vec![
                NativeCall {
                    address: object_plt,
                    name: "object",
                    argument: 1,
                },
                NativeCall {
                    address: text_plt,
                    name: "text",
                    argument: 1,
                },
            ],
            object_filtered: false,
        });
    }
    windows[3].registers.push((20, SHAPE));
    windows[4].registers.push((20, SHAPE));
    windows[5].registers.push((20, SHAPE));
    windows
}

pub(super) fn capture(machine: &mut Machine, composer: &Path) {
    map_library(machine.engine, composer, COMPOSER, COMPOSER_SHA256);
    let windows = windows();
    let mut cases = vec![];
    for window in &windows {
        for (present, object_flag) in [(false, 0), (true, 0), (true, 1), (true, 255)] {
            if !present && window.calls.is_empty() {
                continue;
            }
            for (foreground, background) in [
                (0xff12_3456, 0x00ff_ffff),
                (0x00ab_cdef, 0xff26_2626),
                (0x0112_3456, 0x0126_2626),
                (0xff12_3456, 0xff26_2626),
            ] {
                let input = Input {
                    present,
                    object_flag,
                    foreground,
                    background,
                };
                let expected = window.fixture(machine, input, 0);
                for fill in [0xa5, 0xff] {
                    assert_eq!(
                        window.fixture(machine, input, fill),
                        expected,
                        "{} memory fill",
                        window.name
                    );
                }
                cases.push(expected);
            }
        }
    }
    let window_json: Vec<_> = windows.iter().map(Window::json).collect();
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"composer_library_sha256\":\"{COMPOSER_SHA256}\",\"memory_fills\":[0,165,255],\"supplied_drawn_fields\":{{\"layout_rect\":104,\"object_flag\":120,\"foreground_argb\":136,\"background_argb\":144}},\"capture_boundary\":\"Unchanged native instruction windows execute one supplied retained DrawnText pointer iteration of primary Body/Table/Code/Placed background callers, Body object/text dispatch, or the indicated alpha gate. Native background/object/text call targets are intercepted to record requests and return. The caller windows stop before advancing their vectors; alpha windows stop before PDF allocation or rectangle/path access. Model+0x18000 holds a supplied zero-initialized 160-byte DrawnText with the recorded fields; Model+0x19000 holds its pointer or null. X19 is a supplied writer address, X29/SP point at supplied zeroed stack locals, and listed window registers are supplied. No font shaping, object producer, complete PDF caller, clipping, path allocation, paint, pixels or native SVG consumer executes. The shared exporter gate reads foreground alpha; legacy writer gates read background alpha.\",\"windows\":[{}],\"cases\":[\n{}\n]}}",
        window_json.join(","),
        cases.join(",\n")
    );
}

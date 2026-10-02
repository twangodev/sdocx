use super::super::text_paragraph_layout;
use super::*;

pub(crate) struct Host {
    fonts: Box<Services>,
    pub(crate) paragraphs: Box<text_paragraph_layout::ParagraphIcu>,
    uuid_sequence: u32,
}

pub(crate) fn install_host(
    machine: &Machine,
    environment: &mut NativeFontEnvironment,
    fonts: Box<Services>,
) -> Box<Host> {
    let mut host = Box::new(Host {
        fonts,
        paragraphs: text_paragraph_layout::ParagraphIcu::new(machine),
        uuid_sequence: 0,
    });
    environment.set_host_import_handler(host_import, ptr::from_mut(host.as_mut()).cast());
    host
}

pub(crate) fn reset_host(
    machine: &mut Machine,
    environment: &mut NativeFontEnvironment,
    host: &mut Host,
    fill: u8,
) {
    machine.heap.cursor = HEAP;
    machine.heap.limit = MODEL + 0x50000;
    machine.heap.allocation_fill = fill;
    machine.heap.allocations = 0;
    machine.heap.fills = 0;
    machine.heap.deletes = 0;
    host.uuid_sequence = 0;
    host.paragraphs.calls.clear();
    reset(machine, environment, &mut host.fonts, fill);
    register(machine.engine, REGISTER_TPIDR_EL0, MODEL + 0x7e000);
    write(machine.engine, MODEL + 0x7e000, &[0; 64]);
    for address in [0x2a2ed0, 0x2863a0, 0x3c35ec, 0x3c5918] {
        assert!(
            (0x4a3208..0x4a38b0)
                .step_by(8)
                .any(|slot| read_u64(machine.engine, slot) == address)
        );
        machine.call(address, &[]);
    }
}

pub(super) fn host_import(engine: Engine, name: &str, args: [u64; 8], data: *mut c_void) -> Option<u64> {
    let host = unsafe { &mut *data.cast::<Host>() };
    if let Some(value) =
        text_paragraph_layout::paragraph_icu_import(engine, name, args, &mut host.paragraphs)
    {
        return Some(value);
    }
    match name {
        "uuid_create" => {
            write(engine, args[0], &(MODEL + 0x7d000).to_le_bytes());
            Some(0)
        }
        "uuid_make" | "uuid_destroy" => Some(0),
        "uuid_export" => {
            host.uuid_sequence += 1;
            let destination = read_u64(engine, args[2]);
            write(
                engine,
                destination,
                format!("00000000-0000-4000-8000-{:012x}\0", host.uuid_sequence).as_bytes(),
            );
            Some(0)
        }
        _ => import(
            engine,
            name,
            args,
            ptr::from_mut(host.fonts.as_mut()).cast(),
        ),
    }
}

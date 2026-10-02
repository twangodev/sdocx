use super::*;

struct Pair {
    name: String,
    entry_member: Option<(u64, u32)>,
    span_member: Option<(u64, u32)>,
    right_to_left: bool,
    missing_font: bool,
    distinct_font: bool,
}

impl Pair {
    fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            entry_member: None,
            span_member: None,
            right_to_left: false,
            missing_font: false,
            distinct_font: false,
        }
    }

    fn fixture(&self, machine: &Machine, fill: u8) -> String {
        write(machine.engine, MODEL, &vec![fill; 0x100000]);
        for (address, size) in [(ENTRIES, 160), (SPANS, 144), (FONT, 32)] {
            write(machine.engine, address, &vec![0; size]);
        }
        float(machine.engine, ENTRIES, 10.0);
        float(machine.engine, ENTRIES + 8, 4.25);
        float(machine.engine, ENTRIES + 12, 20.0);
        float(machine.engine, ENTRIES + 80, 12.0);
        float(machine.engine, ENTRIES + 88, 14.25);
        float(machine.engine, ENTRIES + 92, 20.0);
        float(machine.engine, SPANS, 20.0);
        float(machine.engine, SPANS + 72, 20.0);
        if self.right_to_left {
            for entry in [ENTRIES, ENTRIES + 80] {
                write(machine.engine, entry + 52, &1_u32.to_le_bytes());
            }
            float(machine.engine, ENTRIES + 8, 26.25);
        }
        if let Some((offset, value)) = self.entry_member {
            write(machine.engine, ENTRIES + 80 + offset, &value.to_le_bytes());
        }
        if let Some((offset, value)) = self.span_member {
            write(machine.engine, SPANS + 72 + offset, &value.to_le_bytes());
        }
        let font = if self.missing_font {
            0
        } else if self.distinct_font {
            FONT + 16
        } else {
            FONT
        };
        let same = machine.call(
            SAME_DRAW,
            &[0, ENTRIES, ENTRIES + 80, SPANS, SPANS + 72, FONT, font],
        );
        assert!(same <= 1);
        let mut bytes = [0; 72];
        check(unsafe {
            uc_mem_read(
                machine.engine,
                SPANS + 72,
                bytes.as_mut_ptr().cast(),
                bytes.len(),
            )
        });
        let positions = [ENTRIES, ENTRIES + 80].map(|entry| {
            [
                read_float(machine.engine, entry),
                read_float(machine.engine, entry + 8),
                read_float(machine.engine, entry + 12),
            ]
        });
        let directions = [
            read_u32(machine.engine, ENTRIES + 52),
            read_u32(machine.engine, ENTRIES + 132),
        ];
        format!(
            "{{\"name\":{:?},\"advance_x_y\":{positions:?},\"directions\":{directions:?},\"second_kind\":{},\"second_span_bytes\":{bytes:?},\"missing_font\":{},\"distinct_font_wrapper\":{},\"same_draw\":{}}}",
            self.name,
            read_u32(machine.engine, ENTRIES + 128),
            self.missing_font,
            self.distinct_font,
            same == 1
        )
    }
}

pub(super) fn capture(machine: &Machine) -> Vec<String> {
    let mut cases = vec![Pair::new("adjacent")];
    for (name, offset, value) in [
        ("gap-above", 8, 14.25_f32.to_bits() + 1),
        ("gap-below", 8, 14.25_f32.to_bits() - 1),
        ("different-baseline", 12, 50.0_f32.to_bits()),
        ("kind-five", 48, 5),
        ("mixed-directions", 52, 1),
    ] {
        let mut pair = Pair::new(name);
        pair.entry_member = Some((offset, value));
        cases.push(pair);
    }
    let mut rtl = Pair::new("rtl-adjacent");
    rtl.right_to_left = true;
    cases.push(rtl);
    let mut no_font = Pair::new("missing-font");
    no_font.missing_font = true;
    cases.push(no_font);
    let mut other_font = Pair::new("distinct-font-wrapper");
    other_font.distinct_font = true;
    cases.push(other_font);
    for (offset, value) in [
        (0, 24.0_f32.to_bits()),
        (4, 1),
        (8, 1),
        (12, 1),
        (16, 1),
        (24, 1),
        (32, 1),
        (36, 1),
        (40, 1),
        (44, 1),
        (52, 1),
        (60, 1),
        (66, 1),
    ] {
        let mut pair = Pair::new(&format!("span-member-{offset}"));
        pair.span_member = Some((offset, value));
        cases.push(pair);
    }
    cases
        .into_iter()
        .map(|case| {
            let expected = case.fixture(machine, 0);
            for fill in [0xa5, 0xff] {
                assert_eq!(
                    case.fixture(machine, fill),
                    expected,
                    "{} memory fill",
                    case.name
                );
            }
            expected
        })
        .collect()
}

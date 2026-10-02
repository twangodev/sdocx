use super::*;
use frames::{BASE, BASE_SHA256};
use geometry::{TEXT_SHA256, WIDGET_SHA256};

const WIDGET: u64 = 0x0400_0000;
const TEXT: u64 = 0x0500_0000;
const VECTOR: u64 = MODEL + 0x9000;
const LAYOUT: u64 = MODEL + 0xa000;
const THEME: u64 = MODEL + 0xa800;
const THEME_VTABLE: u64 = MODEL + 0xa900;
const SOURCE: u64 = MODEL + 0xb000;
const SOURCE_BASE: u64 = MODEL + 0xb100;
const SOURCE_DATA: u64 = MODEL + 0xb200;
const SOURCE_STRING: u64 = MODEL + 0xb300;
const STRING_IMPL: u64 = MODEL + 0xb400;
const CHARACTERS: u64 = MODEL + 0xb500;
const THEME_COLOR: u64 = 0x0300_0900;

fn pointer(engine: Engine, address: u64, value: u64) {
    write(engine, address, &value.to_le_bytes());
}

fn word(engine: Engine, address: u64, value: u32) {
    write(engine, address, &value.to_le_bytes());
}

fn byte(engine: Engine, address: u64) -> u8 {
    let mut value = 0;
    check(unsafe { uc_mem_read(engine, address, ptr::from_mut(&mut value).cast(), 1) });
    value
}

#[derive(Clone, Debug)]
enum Attribute {
    Foreground {
        color: u32,
        color_type: u32,
    },
    Background(u32),
    ComposingBackground(u32),
    FontSize(f32),
    FontName(Option<&'static str>),
    Toggle {
        kind: u32,
        enabled: bool,
    },
    Hyperlink(u32),
    ComposingTag(bool),
    Composing,
    Suggestion(u32),
    Correction {
        color: u32,
        underline: u32,
        enabled: bool,
        strike: bool,
    },
    Object(bool),
    Inline(bool),
    OverPages(bool),
    Ignored(u32),
}

impl Attribute {
    fn json(&self) -> String {
        match self {
            Self::Foreground { color, color_type } => {
                format!("{{\"kind\":\"foreground\",\"color\":{color},\"color_type\":{color_type}}}")
            }
            Self::Background(color) => format!("{{\"kind\":\"background\",\"color\":{color}}}"),
            Self::ComposingBackground(color) => {
                format!("{{\"kind\":\"composing_background\",\"color\":{color}}}")
            }
            Self::FontSize(size) => format!("{{\"kind\":\"font_size\",\"size\":{size:?}}}"),
            Self::FontName(name) => format!(
                "{{\"kind\":\"font_name\",\"name\":{}}}",
                name.map_or_else(|| "null".into(), |name| format!("{name:?}"))
            ),
            Self::Toggle { kind, enabled } => {
                format!("{{\"kind\":\"toggle\",\"type\":{kind},\"enabled\":{enabled}}}")
            }
            Self::Hyperlink(kind) => format!("{{\"kind\":\"hyperlink\",\"type\":{kind}}}"),
            Self::ComposingTag(enabled) => {
                format!("{{\"kind\":\"composing_tag\",\"background_enabled\":{enabled}}}")
            }
            Self::Composing => "{\"kind\":\"composing\"}".into(),
            Self::Suggestion(color) => format!("{{\"kind\":\"suggestion\",\"underline\":{color}}}"),
            Self::Correction {
                color,
                underline,
                enabled,
                strike,
            } => format!(
                "{{\"kind\":\"spell_correction\",\"color\":{color},\"underline\":{underline},\"enabled\":{enabled},\"strike\":{strike}}}"
            ),
            Self::Object(enabled) => format!("{{\"kind\":\"object\",\"enabled\":{enabled}}}"),
            Self::Inline(enabled) => format!("{{\"kind\":\"inline\",\"enabled\":{enabled}}}"),
            Self::OverPages(enabled) => {
                format!("{{\"kind\":\"over_pages\",\"enabled\":{enabled}}}")
            }
            Self::Ignored(kind) => format!("{{\"kind\":\"ignored\",\"type\":{kind}}}"),
        }
    }

    fn apply(&self, machine: &Machine, span: u64, scale: f32) {
        let setter = match self {
            Self::Object(value) => Some((0x8dce8, *value)),
            Self::Inline(value) => Some((0x8dd08, *value)),
            Self::OverPages(value) => Some((0x8dd18, *value)),
            _ => None,
        };
        if let Some((setter, value)) = setter {
            machine.call(TEXT + setter, &[span, u64::from(value)]);
            return;
        }
        for (address, size) in [(SOURCE, 24), (SOURCE_BASE, 16), (SOURCE_DATA, 32)] {
            write(machine.engine, address, &vec![0; size]);
        }
        pointer(machine.engine, SOURCE + 8, SOURCE_BASE);
        pointer(machine.engine, SOURCE + 16, SOURCE_DATA);
        let kind = match *self {
            Self::Foreground { color, color_type } => {
                word(machine.engine, SOURCE_DATA, color);
                word(machine.engine, SOURCE_DATA + 4, color_type);
                1
            }
            Self::Background(color) => {
                word(machine.engine, SOURCE_DATA, color);
                17
            }
            Self::ComposingBackground(color) => {
                word(machine.engine, SOURCE_DATA, color);
                15
            }
            Self::FontSize(size) => {
                word(machine.engine, SOURCE_DATA, size.to_bits());
                3
            }
            Self::FontName(name) => {
                let name_pointer = name.map_or(0, |name| {
                    assert!(name.len() < 64 && !name.contains('\0'));
                    write(machine.engine, SOURCE_STRING, &[0; 16]);
                    write(machine.engine, STRING_IMPL, &[0; 24]);
                    pointer(machine.engine, SOURCE_STRING + 8, STRING_IMPL);
                    let units: Vec<_> = name.encode_utf16().collect();
                    word(machine.engine, STRING_IMPL + 8, units.len() as u32);
                    word(machine.engine, STRING_IMPL + 12, units.len() as u32);
                    pointer(machine.engine, STRING_IMPL + 16, CHARACTERS);
                    for (index, unit) in units.into_iter().chain([0]).enumerate() {
                        write(
                            machine.engine,
                            CHARACTERS + index as u64 * 2,
                            &unit.to_le_bytes(),
                        );
                    }
                    SOURCE_STRING
                });
                pointer(machine.engine, SOURCE_DATA, name_pointer);
                4
            }
            Self::Toggle { kind, enabled } => {
                write(machine.engine, SOURCE_DATA, &[u8::from(enabled)]);
                kind
            }
            Self::Hyperlink(kind) => {
                word(machine.engine, SOURCE_DATA, kind);
                9
            }
            Self::ComposingTag(enabled) => {
                write(machine.engine, SOURCE_DATA, &[u8::from(enabled)]);
                18
            }
            Self::Composing => 16,
            Self::Suggestion(color) => {
                word(machine.engine, SOURCE_DATA + 4, color);
                21
            }
            Self::Correction {
                color,
                underline,
                enabled,
                strike,
            } => {
                word(machine.engine, SOURCE_DATA, color);
                word(machine.engine, SOURCE_DATA + 16, underline);
                write(
                    machine.engine,
                    SOURCE_DATA + 20,
                    &[u8::from(enabled), u8::from(strike)],
                );
                22
            }
            Self::Ignored(kind) => kind,
            Self::Object(_) | Self::Inline(_) | Self::OverPages(_) => unreachable!(),
        };
        word(machine.engine, SOURCE_BASE, kind);
        register(machine.engine, 136, u64::from(scale.to_bits()));
        machine.call(WIDGET + 0xd7ba8, &[LAYOUT, SOURCE, span]);
    }
}

#[derive(Debug, PartialEq)]
struct Span {
    font_size: f32,
    foreground: u32,
    background: u32,
    composing_background: u32,
    style: u8,
    font_name: Option<String>,
    underline: u32,
    correction_foreground: u32,
    flags: u8,
    inline: bool,
    over_pages: bool,
    correction_enabled: bool,
    math_answer: bool,
}

impl Span {
    fn read(engine: Engine, address: u64) -> Self {
        let font_name = read_u64(engine, address + 24);
        let font_name = (font_name != 0).then(|| {
            let implementation = read_u64(engine, font_name + 8);
            let length = read_u32(engine, implementation + 12) as usize;
            assert!(length < 64);
            let characters = read_u64(engine, implementation + 16);
            let mut bytes = vec![0; length * 2];
            check(unsafe {
                uc_mem_read(engine, characters, bytes.as_mut_ptr().cast(), bytes.len())
            });
            let units: Vec<_> = bytes
                .chunks_exact(2)
                .map(|unit| u16::from_le_bytes(unit.try_into().unwrap()))
                .collect();
            String::from_utf16(&units).unwrap()
        });
        Self {
            font_size: read_float(engine, address),
            foreground: read_u32(engine, address + 4),
            background: read_u32(engine, address + 8),
            composing_background: read_u32(engine, address + 12),
            style: byte(engine, address + 16),
            font_name,
            underline: read_u32(engine, address + 32),
            correction_foreground: read_u32(engine, address + 36),
            flags: byte(engine, address + 40),
            inline: byte(engine, address + 64) != 0,
            over_pages: byte(engine, address + 65) != 0,
            correction_enabled: byte(engine, address + 66) != 0,
            math_answer: byte(engine, address + 67) != 0,
        }
    }

    fn json(&self) -> String {
        let name = self
            .font_name
            .as_ref()
            .map_or_else(|| "null".into(), |name| format!("{name:?}"));
        format!(
            "{{\"font_size\":{:?},\"foreground\":{},\"background\":{},\"composing_background\":{},\"style\":{},\"font_name\":{name},\"underline\":{},\"correction_foreground\":{},\"flags\":{},\"inline\":{},\"over_pages\":{},\"correction_enabled\":{},\"math_answer\":{}}}",
            self.font_size,
            self.foreground,
            self.background,
            self.composing_background,
            self.style,
            self.underline,
            self.correction_foreground,
            self.flags,
            self.inline,
            self.over_pages,
            self.correction_enabled,
            self.math_answer
        )
    }
}

#[derive(Default)]
struct Theme {
    xor: u32,
    inputs: Vec<u32>,
}

unsafe extern "C" fn theme_color(engine: Engine, _: u64, _: u32, data: *mut c_void) {
    let theme = unsafe { &mut *data.cast::<Theme>() };
    assert_eq!(read_register(engine, REGISTER_X0), THEME);
    assert_eq!(read_register(engine, REGISTER_X0 + 2), 3);
    let color = read_register(engine, REGISTER_X0 + 1) as u32;
    theme.inputs.push(color);
    register(engine, REGISTER_X0, u64::from(color ^ theme.xor));
}

struct ThemeHook {
    engine: Engine,
    hook: usize,
    state: Box<Theme>,
}

impl ThemeHook {
    fn new(machine: &Machine) -> Self {
        write(machine.engine, THEME_COLOR, &0xd65f03c0_u32.to_le_bytes());
        let mut recorder = Self {
            engine: machine.engine,
            hook: 0,
            state: Box::default(),
        };
        check(unsafe {
            uc_hook_add(
                machine.engine,
                &mut recorder.hook,
                4,
                theme_color as *mut c_void,
                ptr::from_mut(recorder.state.as_mut()).cast(),
                THEME_COLOR,
                THEME_COLOR,
            )
        });
        recorder
    }
}

impl Drop for ThemeHook {
    fn drop(&mut self) {
        check(unsafe { uc_hook_del(self.engine, self.hook) });
    }
}

struct Case {
    name: String,
    delta: f32,
    scale: f32,
    theme_xor: u32,
    object_defaults: bool,
    left: Vec<Attribute>,
    right: Vec<Attribute>,
}

impl Case {
    fn new(name: impl Into<String>, left: Vec<Attribute>, right: Vec<Attribute>) -> Self {
        Self {
            name: name.into(),
            delta: 0.0,
            scale: 1.0,
            theme_xor: 0,
            object_defaults: true,
            left,
            right,
        }
    }

    fn fixture(&self, machine: &mut Machine, theme: &mut ThemeHook, fill: u8) -> String {
        write(machine.engine, MODEL, &vec![fill; 0x100000]);
        machine.heap.cursor = HEAP;
        machine.heap.allocation_fill = fill;
        *theme.state = Theme {
            xor: self.theme_xor,
            inputs: Vec::new(),
        };
        for (address, size) in [(LAYOUT, 608), (THEME_VTABLE, 88)] {
            write(machine.engine, address, &vec![0; size]);
        }
        pointer(machine.engine, LAYOUT + 352, THEME);
        pointer(machine.engine, THEME, THEME_VTABLE);
        pointer(machine.engine, THEME_VTABLE + 80, THEME_COLOR);
        word(machine.engine, LAYOUT + 540, self.delta.to_bits());
        machine.call(WIDGET + 0xc0d38, &[VECTOR, 2]);
        let spans = read_u64(machine.engine, VECTOR);
        assert_eq!(read_u64(machine.engine, VECTOR + 8), spans + 144);
        if self.object_defaults {
            let color = machine.call(THEME_COLOR, &[THEME, 0xff26_2626, 3]);
            register(machine.engine, REGISTER_X0 + 20, 2);
            register(machine.engine, 144, u64::from(self.scale.to_bits()));
            register(machine.engine, 145, u64::from(self.delta.to_bits()));
            pointer(machine.engine, STACK + 32, spans);
            register(machine.engine, REGISTER_SP, STACK);
            register(machine.engine, REGISTER_X0, color);
            check(unsafe {
                uc_emu_start(
                    machine.engine,
                    WIDGET + 0xd4fdc,
                    WIDGET + 0xd5004,
                    1_000_000,
                    1000,
                )
            });
            assert_eq!(read_register(machine.engine, 260), WIDGET + 0xd5004);
        }
        for (address, attributes) in [(spans, &self.left), (spans + 72, &self.right)] {
            for attribute in attributes {
                attribute.apply(machine, address, self.scale);
            }
        }
        let left = Span::read(machine.engine, spans);
        let right = Span::read(machine.engine, spans + 72);
        let different = machine.call(TEXT + 0x8db40, &[spans, spans + 72]) != 0;
        let reversed = machine.call(TEXT + 0x8db40, &[spans + 72, spans]) != 0;
        assert_eq!(different, reversed);
        let left_inputs = self
            .left
            .iter()
            .map(Attribute::json)
            .collect::<Vec<_>>()
            .join(",");
        let right_inputs = self
            .right
            .iter()
            .map(Attribute::json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"name\":{:?},\"font_size_delta\":{:?},\"scale\":{:?},\"theme_xor\":{},\"object_defaults\":{},\"left_inputs\":[{left_inputs}],\"right_inputs\":[{right_inputs}],\"left\":{},\"right\":{},\"different\":{different},\"theme_inputs\":{:?}}}",
            self.name,
            self.delta,
            self.scale,
            self.theme_xor,
            self.object_defaults,
            left.json(),
            right.json(),
            theme.state.inputs
        )
    }
}

fn load(machine: &Machine, base: &Path, widget: &Path, text: &Path) {
    frames::load_base(machine, base);
    map_library(machine.engine, widget, WIDGET, WIDGET_SHA256);
    map_library(machine.engine, text, TEXT, TEXT_SHA256);
    for (plt, target) in [
        (WIDGET + 0xe6640, NEW),
        (WIDGET + 0xe6660, DELETE),
        (WIDGET + 0xe8b50, 0x40c698),
        (WIDGET + 0xe8ba0, 0x40a42c),
        (WIDGET + 0xe9da0, 0x40a484),
        (WIDGET + 0xe9dd0, 0x409cf8),
        (WIDGET + 0xe8bd0, 0x40949c),
        (WIDGET + 0xe8bf0, 0x408cd8),
        (WIDGET + 0xe8c10, 0x40b6e4),
        (WIDGET + 0xe9df0, 0x40d5a8),
        (WIDGET + 0xe8c30, 0x40dc84),
        (WIDGET + 0xe8e30, 0x40ac40),
        (WIDGET + 0xe9dc0, 0x415454),
        (WIDGET + 0xe9db0, 0x4147ec),
        (WIDGET + 0xe9de0, 0x4159c8),
        (WIDGET + 0xe9e10, 0x418680),
        (WIDGET + 0xe9e30, 0x4193e0),
        (WIDGET + 0xe9e40, 0x419174),
        (WIDGET + 0xe9e50, 0x419320),
        (WIDGET + 0xe9e60, 0x4194a8),
        (WIDGET + 0xe8100, TEXT + 0x8dcbc),
        (WIDGET + 0xe9e00, TEXT + 0x8dcc8),
        (WIDGET + 0xe9e20, TEXT + 0x8dd28),
        (WIDGET + 0xe7d50, BASE + 0xc2a18),
        (WIDGET + 0xe7d60, BASE + 0xc2abc),
        (BASE + 0xe56f0, NEW),
        (BASE + 0xe55f0, NEW),
        (BASE + 0xe7bd0, MEMSET),
        (TEXT + 0xf06d0, BASE + 0xc49e4),
    ] {
        bind_native(machine.engine, plt, target);
    }
}

fn cases() -> Vec<Case> {
    let mut cases = vec![Case::new("object-defaults", vec![], vec![])];
    let mut defaults = Case::new("constructor-defaults", vec![], vec![]);
    defaults.object_defaults = false;
    cases.push(defaults);
    for (name, attribute) in [
        (
            "foreground",
            Attribute::Foreground {
                color: 0x8012_3456,
                color_type: 0,
            },
        ),
        ("background", Attribute::Background(0x8012_3456)),
        (
            "composing-background",
            Attribute::ComposingBackground(0x8012_3456),
        ),
        ("font-size", Attribute::FontSize(24.0)),
        ("font-name", Attribute::FontName(Some("Roboto"))),
        ("empty-font-name", Attribute::FontName(Some(""))),
        ("object-flag", Attribute::Object(true)),
        ("inline-ignored", Attribute::Inline(true)),
        ("over-pages-ignored", Attribute::OverPages(true)),
        ("hyperlink", Attribute::Hyperlink(3)),
        ("composing-tag-background", Attribute::ComposingTag(true)),
        ("composing-tag-style", Attribute::ComposingTag(false)),
        ("suggestion", Attribute::Suggestion(0xff12_3456)),
        (
            "correction",
            Attribute::Correction {
                color: 0xff12_3456,
                underline: 0xff65_4321,
                enabled: true,
                strike: true,
            },
        ),
    ] {
        cases.push(Case::new(name, vec![], vec![attribute]));
    }
    for kind in [5, 6, 7, 20] {
        cases.push(Case::new(
            format!("toggle-{kind}"),
            vec![],
            vec![Attribute::Toggle {
                kind,
                enabled: true,
            }],
        ));
        cases.push(Case::new(
            format!("toggle-{kind}-reset"),
            vec![],
            vec![
                Attribute::Toggle {
                    kind,
                    enabled: true,
                },
                Attribute::Toggle {
                    kind,
                    enabled: false,
                },
            ],
        ));
    }
    cases.push(Case::new(
        "composing-underline",
        vec![],
        vec![Attribute::Composing],
    ));
    cases.push(Case::new(
        "composing-overrides-disabled-underline",
        vec![],
        vec![
            Attribute::Toggle {
                kind: 7,
                enabled: false,
            },
            Attribute::Composing,
        ],
    ));
    for kind in [0, 2, 8, 10, 11, 12, 13, 14, 19, 23, 255] {
        cases.push(Case::new(
            format!("ignored-type-{kind}"),
            vec![],
            vec![Attribute::Ignored(kind)],
        ));
    }
    for kind in [0, 1, 9, 10, u32::MAX] {
        cases.push(Case::new(
            format!("hyperlink-type-{kind}"),
            vec![],
            vec![Attribute::Hyperlink(kind)],
        ));
    }
    for (name, left, right) in [
        (
            "font-null-and-default",
            vec![],
            vec![Attribute::FontName(None)],
        ),
        (
            "font-equal-distinct-strings",
            vec![Attribute::FontName(Some("Roboto"))],
            vec![Attribute::FontName(Some("Roboto"))],
        ),
        (
            "font-case-sensitive",
            vec![Attribute::FontName(Some("Roboto"))],
            vec![Attribute::FontName(Some("roboto"))],
        ),
        (
            "font-empty-equal",
            vec![Attribute::FontName(Some(""))],
            vec![Attribute::FontName(Some(""))],
        ),
        (
            "font-overwrite",
            vec![Attribute::FontName(Some("Roboto"))],
            vec![
                Attribute::FontName(Some("Noto Sans")),
                Attribute::FontName(Some("Roboto")),
            ],
        ),
        (
            "font-null-overwrite",
            vec![],
            vec![
                Attribute::FontName(Some("Roboto")),
                Attribute::FontName(None),
            ],
        ),
        (
            "foreground-overwrite",
            vec![Attribute::Foreground {
                color: 0xff26_2626,
                color_type: 0,
            }],
            vec![
                Attribute::Foreground {
                    color: 0xff12_3456,
                    color_type: 0,
                },
                Attribute::Foreground {
                    color: 0xff26_2626,
                    color_type: 0,
                },
            ],
        ),
        (
            "math-answer-ignored",
            vec![Attribute::Foreground {
                color: 0xff26_2626,
                color_type: 0,
            }],
            vec![Attribute::Foreground {
                color: 0xff26_2626,
                color_type: 1,
            }],
        ),
        (
            "composing-background-preserves-background",
            vec![Attribute::ComposingBackground(0xff12_3456)],
            vec![
                Attribute::Background(0xff65_4321),
                Attribute::ComposingBackground(0xff12_3456),
            ],
        ),
        (
            "object-skips-background",
            vec![Attribute::Object(true)],
            vec![
                Attribute::Object(true),
                Attribute::Background(0xff12_3456),
                Attribute::ComposingBackground(0xff65_4321),
            ],
        ),
        (
            "flags-independent",
            vec![Attribute::Object(true)],
            vec![Attribute::Hyperlink(3), Attribute::Object(true)],
        ),
        (
            "hyperlink-reset",
            vec![],
            vec![Attribute::Hyperlink(3), Attribute::Hyperlink(10)],
        ),
    ] {
        cases.push(Case::new(name, left, right));
    }
    for (name, color, underline, enabled, strike) in [
        ("color", 0xff12_3456, 0xff00_0000, false, false),
        ("underline", 0xff00_0000, 0xff12_3456, false, false),
        ("enabled", 0xff00_0000, 0xff00_0000, true, false),
        ("strike", 0xff00_0000, 0xff00_0000, false, true),
    ] {
        let left = Attribute::Correction {
            color: 0xff00_0000,
            underline: 0xff00_0000,
            enabled: false,
            strike: false,
        };
        let right = Attribute::Correction {
            color,
            underline,
            enabled,
            strike,
        };
        cases.push(Case::new(
            format!("correction-{name}-identity"),
            vec![left],
            vec![right],
        ));
    }
    for (name, left, right) in [
        (
            "background-zero-reset",
            vec![],
            vec![Attribute::Background(0xff12_3456), Attribute::Background(0)],
        ),
        (
            "composing-background-zero-reset",
            vec![],
            vec![
                Attribute::ComposingBackground(0xff12_3456),
                Attribute::ComposingBackground(0),
            ],
        ),
        (
            "object-flag-reset",
            vec![],
            vec![Attribute::Object(true), Attribute::Object(false)],
        ),
        (
            "font-utf16-equal",
            vec![Attribute::FontName(Some("字体𝄞"))],
            vec![Attribute::FontName(Some("字体𝄞"))],
        ),
        (
            "composing-tag-preserves-background",
            vec![Attribute::Background(0x1925_2525)],
            vec![
                Attribute::ComposingTag(true),
                Attribute::ComposingTag(false),
            ],
        ),
        (
            "suggestion-overwrite",
            vec![Attribute::Suggestion(0xff12_3456)],
            vec![
                Attribute::Suggestion(0xff65_4321),
                Attribute::Suggestion(0xff12_3456),
            ],
        ),
        (
            "correction-disable-preserves-strike",
            vec![Attribute::Correction {
                color: 0xff12_3456,
                underline: 0xff65_4321,
                enabled: false,
                strike: false,
            }],
            vec![
                Attribute::Correction {
                    color: 0xff12_3456,
                    underline: 0xff65_4321,
                    enabled: true,
                    strike: true,
                },
                Attribute::Correction {
                    color: 0xff12_3456,
                    underline: 0xff65_4321,
                    enabled: false,
                    strike: false,
                },
            ],
        ),
    ] {
        cases.push(Case::new(name, left, right));
    }
    for (name, delta, scale, size) in [
        ("negative-clamp", -20.0, 1.0, 3.0),
        ("zero-clamp", -3.0, 2.0, 3.0),
        ("fractional-scale", 2.25, 1.125, 19.5),
        ("scaled-default-equals-explicit", 3.0, 1.5, 17.0),
    ] {
        let mut case = Case::new(name, vec![], vec![Attribute::FontSize(size)]);
        case.delta = delta;
        case.scale = scale;
        cases.push(case);
    }
    let mut theme = Case::new(
        "supplied-theme-conversion",
        vec![],
        vec![
            Attribute::Foreground {
                color: 0xff12_3456,
                color_type: 0,
            },
            Attribute::Background(0x8012_3456),
            Attribute::ComposingBackground(0x8065_4321),
            Attribute::Suggestion(0xff12_3456),
            Attribute::Correction {
                color: 0xff65_4321,
                underline: 0xffab_cdef,
                enabled: true,
                strike: false,
            },
            Attribute::ComposingTag(true),
        ],
    );
    theme.theme_xor = 0x00ff_ffff;
    cases.push(theme);
    cases
}

pub(super) fn capture(machine: &mut Machine, base: &Path, widget: &Path, text: &Path) {
    load(machine, base, widget, text);
    let mut theme = ThemeHook::new(machine);
    let output: Vec<_> = cases()
        .into_iter()
        .map(|case| {
            let expected = case.fixture(machine, &mut theme, 0);
            for fill in [0xa5, 0xff] {
                assert_eq!(
                    case.fixture(machine, &mut theme, fill),
                    expected,
                    "memory fill changed {}",
                    case.name
                );
            }
            expected
        })
        .collect();
    println!(
        "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"model_library_sha256\":\"{LIBRARY_SHA256}\",\"base_library_sha256\":\"{BASE_SHA256}\",\"widget_library_sha256\":\"{WIDGET_SHA256}\",\"text_library_sha256\":\"{TEXT_SHA256}\",\"memory_fills\":[0,165,255],\"constructor\":\"0xc0d38\",\"object_default_window\":[\"0xd4fdc\",\"0xd5004\"],\"convert_span\":\"0xd7ba8\",\"different\":\"0x8db40\",\"capture_boundary\":\"Native default vector allocation/initialization, object default size/color assignment window, complete ObjectTextLayout convertTextSpanImpl, actual Model span getters, Text flags setters, Base string construction/copy and Text member comparison execute unchanged. Source span types and concrete property buffers, layout delta/scale and theme color interface inputs are supplied. Theme mapping is explicitly color XOR theme_xor, not Samsung theme behavior. Allocation/deletion/memset are host supplied. Named members exclude uninitialized padding. Two spans are independently constructed; attribute lists specify producer call order, not native ComponentText FindSpans traversal. No serialized span decoding, caret/range selection, shaping, font selection, run emission, Composer clipping or rendering executes.\",\"cases\":[\n{}\n]}}",
        output.join(",\n")
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub(in crate::render) enum NativeEntryKind {
    Ordinary = 0,
    Space = 1,
    Tab = 2,
    Unowned = 3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::render) struct NativeEntryFacts {
    pub kind: NativeEntryKind,
    pub font_size: f32,
    pub height: f32,
    pub drawable: bool,
}

impl NativeEntryFacts {
    pub(super) fn measured(unit: u16, font_size: f32, owns_glyphs: bool) -> Self {
        if !owns_glyphs {
            return Self {
                kind: NativeEntryKind::Unowned,
                font_size: 0.0,
                height: 0.0,
                drawable: false,
            };
        }
        Self {
            kind: match unit {
                0x20 => NativeEntryKind::Space,
                0x09 => NativeEntryKind::Tab,
                _ => NativeEntryKind::Ordinary,
            },
            font_size,
            height: font_size,
            drawable: true,
        }
    }
}

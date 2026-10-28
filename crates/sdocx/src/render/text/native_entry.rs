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

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;

    #[test]
    fn scaled_ascii_entry_facts_match_native_cell_controls() {
        let receipt: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../../../conformance/native-resolved-entry-facts-51-150.json"
        ))
        .unwrap();
        let mut checked = 0;
        for case in receipt["cases"].as_array().unwrap() {
            let text = case["native_text_utf8"].as_str().unwrap();
            for entry in case["measured_entries"].as_array().unwrap() {
                let owner = entry["utf16_slot"].as_u64().unwrap() as usize;
                let unit = text.as_bytes()[owner];
                if unit == b'\n' {
                    assert_eq!(entry["kind"], 4);
                    assert_eq!(entry["drawable"], false);
                    continue;
                }
                let size = f32::from_bits(entry["font_size_bits"].as_u64().unwrap() as u32);
                let facts = NativeEntryFacts::measured(u16::from(unit), size, true);
                assert_eq!(facts.kind as u32, entry["kind"].as_u64().unwrap() as u32);
                assert_eq!(
                    facts.font_size.to_bits(),
                    entry["font_size_bits"].as_u64().unwrap() as u32
                );
                assert_eq!(
                    facts.height.to_bits(),
                    entry["font_height_bits"].as_u64().unwrap() as u32
                );
                assert_eq!(facts.drawable, entry["drawable"].as_bool().unwrap());
                checked += 1;
            }
        }
        assert_eq!(checked, 42);
    }
}

//! Native acquisition inputs from original stored objects, without style edits.
use sdocx::{ObjectType, ParseOptions, StoredObject};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt::Write;

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}

fn walk<'a>(objects: &'a [StoredObject], output: &mut Vec<&'a StoredObject>) {
    for object in objects {
        if object.object_type == ObjectType::Stroke {
            output.push(object);
        }
        walk(&object.children, output);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("expected SDOCX path and optional stored stroke indices")?;
    let selected = args
        .map(|value| value.parse::<usize>())
        .collect::<Result<BTreeSet<_>, _>>()?;
    let options = ParseOptions {
        retain_page_sources: true,
        retain_note_source: true,
        ..Default::default()
    };
    let parsed = sdocx::parse_detailed_with_options(&path, &options)?;
    let note = parsed.note.as_ref().ok_or("missing note")?;
    let note_bytes = note
        .source_bytes
        .as_ref()
        .ok_or("missing original note bytes")?;
    let table = note
        .metadata(note_bytes)?
        .string_table
        .ok_or("missing StringID table")?;
    let mut manager = u16::try_from(table.entries.len())?.to_le_bytes().to_vec();
    for entry in &table.entries {
        manager.extend(entry.id.to_le_bytes());
        let units: Vec<_> = entry.text.encode_utf16().collect();
        manager.extend(u16::try_from(units.len())?.to_le_bytes());
        manager.extend(units.into_iter().flat_map(u16::to_le_bytes));
    }
    manager.extend(&table.trailing_data);
    let matches: Vec<_> = note_bytes
        .windows(manager.len())
        .enumerate()
        .filter_map(|(i, b)| (b == manager).then_some(i))
        .collect();
    if matches.len() != 1 {
        return Err("StringID table does not join uniquely to original note bytes".into());
    }
    let resources = sdocx::StrokeResources::new(&table);
    let mut rows = Vec::new();
    let mut index = 0;
    for (page_index, page) in parsed.stored_pages.iter().enumerate() {
        let bytes = page
            .source_bytes
            .as_ref()
            .ok_or("missing original page bytes")?;
        for (layer_index, layer) in page.page.layers.layers.iter().enumerate() {
            let mut objects = Vec::new();
            walk(&layer.objects, &mut objects);
            for object in objects {
                if selected.is_empty() || selected.contains(&index) {
                    let mut stroke = object.decode_stroke(bytes, &options.limits)?;
                    resources.resolve(&mut stroke);
                    let mut row = serde_json::json!({"index":index,"page_index":page_index,"layer_index":layer_index,"archive_entry":page.archive_entry,"page_width":page.page.header.width,"orientation":page.page.header.orientation,"payload_offset":object.payload_offset,"payload_size":object.payload_size,"bbox":stroke.bbox,"point_count":stroke.points.len(),"rendering":stroke.rendering,"pen_width":stroke.pen_width});
                    if !selected.is_empty() {
                        let payload = object
                            .payload(bytes)
                            .ok_or("invalid source object bounds")?;
                        row["payload_hex"] = hex(payload).into();
                        row["payload_sha256"] = format!("{:x}", Sha256::digest(payload)).into();
                        row["stroke"] = serde_json::to_value(stroke)?;
                    }
                    rows.push(row);
                }
                index += 1;
            }
        }
    }
    if !selected.is_empty() && rows.len() != selected.len() {
        return Err("requested stored stroke index absent".into());
    }
    let output = serde_json::json!({"schema":"sdocx-original-saved-ink-inputs-v1","document_sha256":format!("{:x}",Sha256::digest(std::fs::read(&path)?)),"manager_hex":hex(&manager),"manager_sha256":format!("{:x}",Sha256::digest(&manager)),"manager_note_offset":matches[0],"stored_stroke_count":index,"strokes":rows});
    serde_json::to_writer(std::io::stdout().lock(), &output)?;
    Ok(())
}

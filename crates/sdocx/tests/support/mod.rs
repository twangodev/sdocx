use std::io::{Cursor, Write};

pub fn object(kind: u8, payload: &[u8], children: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = vec![kind];
    bytes.extend_from_slice(&(children.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&((payload.len() + 32) as u32).to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes.extend_from_slice(&[0xaa; 32]);
    for child in children {
        bytes.extend(child);
    }
    bytes
}

pub fn page(layers: &[Vec<Vec<u8>>], field_mask: u32, properties: &[u8]) -> Vec<u8> {
    // One-byte property mask and five-byte field mask deliberately move the
    // fixed page fields away from their old absolute offsets.
    let mut bytes = vec![0; 8];
    bytes.extend_from_slice(&[1, 0, 5]);
    bytes.extend_from_slice(&field_mask.to_le_bytes());
    bytes.push(0);
    for value in [0_u32, 1080, 1527, 0, 0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&4_u16.to_le_bytes());
    for c in "page".encode_utf16() {
        bytes.extend_from_slice(&c.to_le_bytes());
    }
    bytes.extend_from_slice(&0_i64.to_le_bytes());
    bytes.extend_from_slice(&5500_u32.to_le_bytes());
    bytes.extend_from_slice(&4000_u32.to_le_bytes());
    let flexible_offset = bytes.len() as u32;
    bytes[4..8].copy_from_slice(&flexible_offset.to_le_bytes());
    bytes.extend_from_slice(properties);
    let layer_offset = bytes.len() as u32;
    bytes[..4].copy_from_slice(&layer_offset.to_le_bytes());
    bytes.extend_from_slice(&(layers.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    for (number, objects) in layers.iter().enumerate() {
        // Variable-size layer masks, with a bounded extension after its number.
        bytes.extend_from_slice(&20_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&[2, 2, 0, 3, 0, 0, 0]);
        bytes.extend_from_slice(&(number as u32).to_le_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&(objects.len() as u32).to_le_bytes());
        for object in objects {
            bytes.extend(object);
        }
        bytes.extend_from_slice(&[0xbb; 32]);
    }
    bytes.extend_from_slice(&[0xcc; 32]);
    bytes.extend_from_slice(b"Page for SAMSUNG S-Pen SDK");
    bytes
}

pub fn archive(page: &[u8]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file("page.page", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.write_all(page).unwrap();
    writer.finish().unwrap().into_inner()
}

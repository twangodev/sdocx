#[doc(hidden)]
pub fn pencil198(s: &Stroke, translation: [f32;2], grain_offset:[f32;2]) -> Vec<Vec<[u8;4]>> {
 let records = ink::pencil2::prepare_stroke(s,1.).unwrap();
 use sha2::{Digest, Sha256};
 let bytes: Vec<u8> = records.iter().flat_map(|r| r.lanes.iter().flat_map(|v| v.to_le_bytes())).collect();
 let expected = if translation == [6., 9.] { "be8483b83e8918f18ffd4b9f23377a28b0bec8b095b58e6c201ac26c566828a8" } else { "ab8c231108800d1a9437783f27d12c57e59c4d81a00538268343f662a1c64389" };
 assert_eq!(format!("{:x}", Sha256::digest(&bytes)), expected, "source-generated records differ from native stream");
 let ctx = ink::pencil2::device_raster::Context { translation, grain_offset, ..Default::default() };
 records.iter().map(|r| ink::pencil2::device_raster::prepare(&[r.lanes],ctx).ok().unwrap().render(None).unwrap()).collect()
}

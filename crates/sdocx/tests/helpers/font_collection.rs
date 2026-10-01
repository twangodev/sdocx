pub fn collection(fonts: &[&[u8]]) -> Vec<u8> {
    let mut output = b"ttcf\0\x01\0\0".to_vec();
    output.extend_from_slice(&u32::try_from(fonts.len()).unwrap().to_be_bytes());
    output.resize(12 + fonts.len() * 4, 0);
    for (index, font) in fonts.iter().enumerate() {
        output.resize(output.len().next_multiple_of(4), 0);
        let offset = output.len();
        output[12 + index * 4..16 + index * 4]
            .copy_from_slice(&u32::try_from(offset).unwrap().to_be_bytes());
        output.extend_from_slice(font);
        let count = u16::from_be_bytes(font[4..6].try_into().unwrap());
        for table in 0..usize::from(count) {
            let position = offset + 12 + table * 16 + 8;
            let original = u32::from_be_bytes(output[position..position + 4].try_into().unwrap());
            output[position..position + 4]
                .copy_from_slice(&(original + u32::try_from(offset).unwrap()).to_be_bytes());
        }
    }
    output
}

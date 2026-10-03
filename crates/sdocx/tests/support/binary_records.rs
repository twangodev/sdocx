pub fn frame(kind: i16, fields: &[u8], fixed: &[u8], flexible: &[u8]) -> Vec<u8> {
    let offset = 12 + fields.len() + fixed.len();
    [
        ((offset + flexible.len()) as u32).to_le_bytes().to_vec(),
        kind.to_le_bytes().to_vec(),
        (offset as u32).to_le_bytes().to_vec(),
        vec![0, fields.len() as u8],
        fields.to_vec(),
        fixed.to_vec(),
        flexible.to_vec(),
    ]
    .concat()
}

pub fn sized(data: &[u8]) -> Vec<u8> {
    [(data.len() as u32).to_le_bytes().as_slice(), data].concat()
}

pub fn text_object(base: &[u8], common_present: bool, payload: &[u8]) -> Vec<u8> {
    [
        base.to_vec(),
        frame(6, &[], &[], &[]),
        frame(7, &[u8::from(common_present)], &[], payload),
    ]
    .concat()
}

pub fn single_cell_table(base: &[u8], content: &[u8], width: f32, height: f32) -> Vec<u8> {
    let mut cell = vec![0; 6];
    cell.extend([0_u32, 1, 1, 0].into_iter().flat_map(u32::to_le_bytes));
    cell.extend(
        [0.0_f64, 0.0, f64::from(width), f64::from(height)]
            .into_iter()
            .flat_map(f64::to_le_bytes),
    );
    cell.push(0);
    cell.extend(sized(content));
    let mut row = vec![0; 6];
    row.extend(height.to_le_bytes());
    row.extend([0_u32, 1].into_iter().flat_map(u32::to_le_bytes));
    row.extend(sized(&cell));
    let mut flexible = 1_u32.to_le_bytes().to_vec();
    flexible.extend(width.to_le_bytes());
    flexible.extend(1_u32.to_le_bytes());
    flexible.extend(sized(&row));
    [base.to_vec(), frame(22, &[12], &[], &flexible)].concat()
}

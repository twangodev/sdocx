use super::*;

pub(super) fn capture(machine: &Machine) {
    let e = machine.engine;
    let data = MODEL + 0xb000;
    let end = 0x2db4f8;
    let original_log = read_bytes(e, 0x47ac10, 4);
    write(e, 0x47ac10, &0xd65f03c0_u32.to_le_bytes());
    let bounds = [10.25_f32, -5.5, 50.75, 80.0];
    let mut cases = Vec::new();
    for (orientation, owner, dimension, scale, result_owner) in [
        (0_u64, [100_i32, 200], 200_u64, 2.0_f32, [200_i32, 400]),
        (1, [100, 200], 400, 2.0, [200, 400]),
        (0, [0, 200], 150, 1.0, [150, 200]),
        (1, [100, 0], 150, 1.0, [100, 150]),
        (0, [100, 200], 100, 1.0, [100, 200]),
        (0, [127, 200], 333, 333.0_f32 / 127.0, [333, 524]),
        (1, [200, 127], 333, 333.0_f32 / 127.0, [524, 333]),
    ] {
        write(e, data, &[0; 256]);
        for (i, value) in bounds.iter().enumerate() {
            write(e, data + 8 + 4 * i as u64, &value.to_le_bytes());
        }
        for (i, value) in owner.iter().enumerate() {
            write(e, data + 176 + 4 * i as u64, &value.to_le_bytes());
        }
        register(e, REGISTER_X0 + 21, dimension);
        register(e, REGISTER_X0 + 22, orientation);
        register(e, REGISTER_X0 + 27, data);
        check(unsafe { uc_emu_start(e, 0x2db434, end, 1_000_000, 1000) });
        assert_eq!(read_register(e, 260), end);
        let actual = std::array::from_fn::<_, 4, _>(|i| read_float(e, data + 8 + 4 * i as u64));
        let actual_owner = std::array::from_fn::<_, 2, _>(|i| read_u32(e, data + 176 + 4 * i as u64) as i32);
        assert_eq!(actual, bounds.map(|v| v * scale));
        assert_eq!(actual_owner, result_owner);
        assert_eq!(read_float(e, data + 184), scale);
        cases.push(format!("{{\"orientation\":{orientation},\"owner\":{owner:?},\"load_dimension\":{dimension},\"bounds\":{bounds:?},\"scale\":{scale:?},\"result_bounds\":{actual:?},\"result_owner\":{actual_owner:?}}}"));
    }
    write(e, 0x47ac10, &original_log);
    println!("{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"model_sha256\":\"{LIBRARY_SHA256}\",\"start\":\"0x2db434\",\"stop\":\"0x2db4f8\",\"boundary\":\"injected base data and registers; android log stubbed to return\",\"cases\":[{}]}}", cases.join(","));
}

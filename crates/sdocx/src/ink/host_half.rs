//! Portable binary16 rounding for captured host shader arithmetic.

fn round_shift_even(v: u32, s: u32) -> u32 {
    if s == 0 {
        return v;
    }
    if s >= 32 {
        return 0;
    }
    let b = v >> s;
    let r = v & ((1 << s) - 1);
    let h = 1 << (s - 1);
    b + u32::from(r > h || (r == h && b & 1 != 0))
}
fn f32_to_f16_rne(x: f32) -> u16 {
    let b = x.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let e = ((b >> 23) & 255) as i32;
    let f = b & 0x7fffff;
    if e == 255 {
        return sign | if f == 0 { 0x7c00 } else { 0x7e00 };
    }
    if e == 0 {
        return sign;
    }
    let e = e - 127;
    let sig = 0x800000 | f;
    if e > 15 {
        return sign | 0x7c00;
    }
    if e >= -14 {
        let mut r = round_shift_even(sig, 13);
        let mut he = e + 15;
        if r == 0x800 {
            r = 0x400;
            he += 1
        }
        if he >= 31 {
            return sign | 0x7c00;
        }
        return sign | ((he as u16) << 10) | (r as u16 & 0x3ff);
    }
    if e < -25 {
        return sign;
    }
    sign | round_shift_even(sig, (-e - 1) as u32) as u16
}
pub(crate) fn half_rne(x: f32) -> f32 {
    f16_to_f32(f32_to_f16_rne(x))
}

fn f16_to_f32(b: u16) -> f32 {
    let s = ((b & 0x8000) as u32) << 16;
    let e = ((b >> 10) & 31) as i32;
    let f = (b & 1023) as u32;
    let bits = match e {
        0 if f == 0 => s,
        0 => {
            let mut m = f;
            let mut q = -14;
            while m & 0x400 == 0 {
                m <<= 1;
                q -= 1
            }
            m &= 1023;
            s | (((q + 127) as u32) << 23) | (m << 13)
        }
        31 => s | 0x7f800000 | (f << 13),
        _ => s | (((e - 15 + 127) as u32) << 23) | (f << 13),
    };
    f32::from_bits(bits)
}
pub(crate) fn half_mul(a: f32, b: f32) -> f32 {
    half_rne(a * b)
}
pub(crate) fn half_add(a: f32, b: f32) -> f32 {
    half_rne(a + b)
}

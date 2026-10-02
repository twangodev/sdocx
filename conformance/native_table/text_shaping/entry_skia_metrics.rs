use super::*;

pub(super) fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for (name, paint_size) in [
        ("to_normal_p1712", 1712.0_f32),
        ("to_normal_p1712_25", 1712.25),
        ("to_normal_p1712_49", 1712.49),
        ("to_normal_p1712_5", 1712.5),
        ("to_normal_p1712_51", 1712.51),
        ("to_normal_p1712_75", 1712.75),
        ("to_normal_p1713", 1713.0),
    ] {
        let mut case = Case::regular(name, "To", paint_size / 100.0);
        case.supplied_paint_size = Some(paint_size);
        cases.push(case);
    }
    for (name, text, rtl, flags) in [
        ("marks_normal_p1712_5", "x\u{327}\u{301}y", false, 0x20000),
        (
            "marks_rtl_normal_p1712_5",
            "x\u{327}\u{301}y",
            true,
            0x20000,
        ),
        ("to_rtl_normal_p1712_5", "To", true, 0x20000),
        ("to_unhinted_p1712_5", "To", false, 0),
    ] {
        let mut case = Case::regular(name, text, 17.125);
        case.rtl = rtl;
        case.flags = flags;
        case.supplied_paint_size = Some(1712.5);
        cases.push(case);
    }
    cases
}

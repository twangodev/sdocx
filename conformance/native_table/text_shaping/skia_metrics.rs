use super::*;

const SKIA: u64 = 0x0600_0000;
const HOOKS: [u64; 8] = [
    0x27d4b4, 0x27db34, 0x27db38, 0x27db6c, 0x27dfe8, 0x27dfec, 0x27e35c, 0x27e380,
];

struct ScalerConfig {
    backend_size_bits: [u32; 2],
    residual_bits: [u32; 4],
    matrix: [i64; 4],
    record_flags: u16,
    mask_format: u8,
    load_flags: u32,
    linear: bool,
    ppem: [u16; 2],
    scale: [i64; 2],
}
impl ScalerConfig {
    fn json(&self) -> String {
        format!(
            "{{\"backend_size_bits\":{:?},\"residual_bits\":{:?},\"matrix\":{:?},\"record_flags\":{},\"mask_format\":{},\"load_flags\":{},\"linear\":{},\"ppem\":{:?},\"scale\":{:?}}}",
            self.backend_size_bits,
            self.residual_bits,
            self.matrix,
            self.record_flags,
            self.mask_format,
            self.load_flags,
            self.linear,
            self.ppem,
            self.scale
        )
    }
}
#[derive(Clone)]
struct Outline {
    points: Vec<[i64; 2]>,
    tags: Vec<u8>,
    contours: Vec<i16>,
}
impl Outline {
    fn read(engine: Engine, address: u64) -> Self {
        let count = i16::from_le_bytes(bytes(engine, address + 2, 2).try_into().unwrap());
        let contour_count = i16::from_le_bytes(bytes(engine, address, 2).try_into().unwrap());
        assert!((0..=4096).contains(&count) && (0..=4096).contains(&contour_count));
        let pointer = read_u64(engine, address + 8);
        let contours = read_u64(engine, address + 24);
        Self {
            points: (0..count)
                .map(|i| {
                    std::array::from_fn(|j| {
                        read_u64(engine, pointer + i as u64 * 16 + j as u64 * 8) as i64
                    })
                })
                .collect(),
            tags: bytes(engine, read_u64(engine, address + 16), count as usize),
            contours: (0..contour_count)
                .map(|i| {
                    i16::from_le_bytes(
                        bytes(engine, contours + i as u64 * 2, 2)
                            .try_into()
                            .unwrap(),
                    )
                })
                .collect(),
        }
    }
    fn json(&self) -> String {
        format!(
            "{{\"points\":{:?},\"tags\":{:?},\"contours\":{:?}}}",
            self.points, self.tags, self.contours
        )
    }
}
struct Advance {
    config: usize,
    glyph: u32,
    flags: u32,
    address: u64,
    source_fixed: i32,
    before: Option<[i32; 2]>,
    after: [i32; 2],
}
impl Advance {
    fn json(&self) -> String {
        format!(
            "{{\"config\":{},\"glyph\":{},\"flags\":{},\"source_fixed\":{},\"before\":{},\"after\":{:?}}}",
            self.config,
            self.glyph,
            self.flags,
            self.source_fixed,
            cached_json(self.before),
            self.after
        )
    }
}
struct Load {
    config: usize,
    glyph: u32,
    flags: u32,
    address: u64,
    outline_address: u64,
    before_outline: Option<Outline>,
    after_outline: Option<Outline>,
    linear_fixed: i64,
    slot_advance: [i64; 2],
    before: Option<[i32; 2]>,
    after: [i32; 2],
    linear_store: bool,
}
impl Load {
    fn json(&self) -> String {
        format!(
            "{{\"config\":{},\"glyph\":{},\"flags\":{},\"before_outline\":{},\"after_outline\":{},\"linear_fixed\":{},\"slot_advance\":{:?},\"before\":{},\"after\":{:?},\"linear_store\":{}}}",
            self.config,
            self.glyph,
            self.flags,
            self.before_outline.as_ref().unwrap().json(),
            self.after_outline.as_ref().unwrap().json(),
            self.linear_fixed,
            self.slot_advance,
            cached_json(self.before),
            self.after,
            self.linear_store
        )
    }
}
#[derive(Default)]
pub(super) struct SkiaTrace {
    configs: Vec<ScalerConfig>,
    context_configs: BTreeMap<u64, usize>,
    initialized_advances: BTreeMap<(u64, u32), [i32; 2]>,
    advances: Vec<Advance>,
    advance_pending: Option<Advance>,
    loads: Vec<Load>,
    load_pending: Option<Load>,
}
impl SkiaTrace {
    fn initialized(&self, engine: Engine, glyph: u64) -> Option<[i32; 2]> {
        let key = (glyph, read_u32(engine, glyph + 24) & 0xffffff);
        self.initialized_advances.get(&key).map(|expected| {
            let actual = cached(engine, glyph);
            assert_eq!(
                &actual, expected,
                "cached metric changed without observed store"
            );
            actual
        })
    }
    pub(super) fn json(&self) -> String {
        assert!(self.advance_pending.is_none() && self.load_pending.is_none());
        assert!(!self.configs.is_empty() && !self.loads.is_empty());
        format!(
            "{{\"configs\":[{}],\"advances\":[{}],\"loads\":[{}]}}",
            self.configs
                .iter()
                .map(ScalerConfig::json)
                .collect::<Vec<_>>()
                .join(","),
            self.advances
                .iter()
                .map(Advance::json)
                .collect::<Vec<_>>()
                .join(","),
            self.loads
                .iter()
                .map(Load::json)
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}
fn cached_json(value: Option<[i32; 2]>) -> String {
    value.map_or_else(|| "null".to_owned(), |value| format!("{value:?}"))
}
fn cached(engine: Engine, glyph: u64) -> [i32; 2] {
    [
        read_u32(engine, glyph + 16) as i32,
        read_u32(engine, glyph + 20) as i32,
    ]
}
unsafe extern "C" fn trace_skia(engine: Engine, address: u64, _: u32, data: *mut c_void) {
    let state = unsafe { &mut *data.cast::<ShapeTrace>() }
        .skia
        .as_mut()
        .unwrap();
    match address - SKIA {
        0x27d4b4 => {
            let ctx = read_register(engine, REGISTER_X0 + 19);
            let face = read_u64(engine, ctx + 208);
            let size = read_u64(engine, face + 160);
            state.context_configs.insert(ctx, state.configs.len());
            state.configs.push(ScalerConfig {
                backend_size_bits: [read_u32(engine, ctx + 304), read_u32(engine, ctx + 308)],
                residual_bits: [0, 1, 3, 4].map(|i| read_u32(engine, ctx + 228 + i * 4)),
                matrix: std::array::from_fn(|i| read_u64(engine, ctx + 272 + i as u64 * 8) as i64),
                record_flags: u16::from_le_bytes(bytes(engine, ctx + 62, 2).try_into().unwrap()),
                mask_format: bytes(engine, ctx + 60, 1)[0],
                load_flags: read_u32(engine, ctx + 312),
                linear: bytes(engine, ctx + 316, 1)[0] != 0,
                ppem: [24, 26]
                    .map(|i| u16::from_le_bytes(bytes(engine, size + i, 2).try_into().unwrap())),
                scale: [
                    read_u64(engine, size + 32) as i64,
                    read_u64(engine, size + 40) as i64,
                ],
            });
        }
        0x27db34 => {
            assert!(state.advance_pending.is_none());
            let ctx = read_register(engine, REGISTER_X0 + 19);
            let glyph = read_register(engine, REGISTER_X0 + 20);
            state.advance_pending = Some(Advance {
                config: state.context_configs[&ctx],
                glyph: read_u32(engine, glyph + 24) & 0xffffff,
                flags: read_register(engine, REGISTER_X0 + 2) as u32,
                address: glyph,
                source_fixed: 0,
                before: state.initialized(engine, glyph),
                after: [0; 2],
            });
        }
        0x27db38 => {
            assert_eq!(
                read_register(engine, REGISTER_X0),
                0,
                "native FT_Get_Advance failure"
            );
            state.advance_pending.as_mut().unwrap().source_fixed =
                read_u32(engine, read_register(engine, REGISTER_SP) + 8) as i32;
        }
        0x27db6c => {
            let mut advance = state.advance_pending.take().unwrap();
            advance.after = cached(engine, read_register(engine, REGISTER_X0 + 20));
            state
                .initialized_advances
                .insert((advance.address, advance.glyph), advance.after);
            state.advances.push(advance);
        }
        0x27dfe8 => {
            assert!(state.load_pending.is_none());
            let ctx = read_register(engine, REGISTER_X0 + 20);
            let glyph = read_register(engine, REGISTER_X0 + 19);
            let face = read_u64(engine, ctx + 208);
            state.load_pending = Some(Load {
                config: state.context_configs[&ctx],
                glyph: read_u32(engine, glyph + 24) & 0xffffff,
                flags: read_register(engine, REGISTER_X0 + 2) as u32,
                address: glyph,
                outline_address: read_u64(engine, face + 152) + 200,
                before_outline: None,
                after_outline: None,
                linear_fixed: 0,
                slot_advance: [0; 2],
                before: state.initialized(engine, glyph),
                after: [0; 2],
                linear_store: false,
            });
        }
        0xfecb0 => {
            if let Some(pending) = &mut state.load_pending
                && read_register(engine, REGISTER_X0) == pending.outline_address
            {
                assert!(pending.before_outline.is_none());
                let matrix = read_register(engine, REGISTER_X0 + 1);
                assert_eq!(
                    std::array::from_fn::<i64, 4, _>(
                        |i| read_u64(engine, matrix + i as u64 * 8) as i64
                    ),
                    state.configs[pending.config].matrix
                );
                pending.before_outline = Some(Outline::read(engine, pending.outline_address));
            }
        }
        0x27dfec => {
            assert_eq!(
                read_register(engine, REGISTER_X0),
                0,
                "native FT_Load_Glyph failure"
            );
            let pending = state.load_pending.as_mut().unwrap();
            let after = Outline::read(engine, pending.outline_address);
            if pending.before_outline.is_none() {
                pending.before_outline = Some(after.clone());
            }
            pending.after_outline = Some(after);
            pending.linear_fixed = read_u64(engine, pending.outline_address - 200 + 112) as i64;
            pending.slot_advance = [
                read_u64(engine, pending.outline_address - 200 + 128) as i64,
                read_u64(engine, pending.outline_address - 200 + 136) as i64,
            ];
        }
        0x27e35c | 0x27e380 => {
            let mut pending = state.load_pending.take().unwrap();
            pending.after = cached(engine, read_register(engine, REGISTER_X0 + 19));
            pending.linear_store = address - SKIA == 0x27e35c;
            state
                .initialized_advances
                .insert((pending.address, pending.glyph), pending.after);
            state.loads.push(pending);
        }
        _ => unreachable!(),
    }
}
pub(super) fn add_hooks(recorder: &mut TraceRecorder) {
    for offset in HOOKS.into_iter().chain([0xfecb0]) {
        let mut hook = 0;
        check(unsafe {
            uc_hook_add(
                recorder.engine,
                &mut hook,
                4,
                trace_skia as *mut c_void,
                ptr::from_mut(recorder.state.as_mut()).cast(),
                SKIA + offset,
                SKIA + offset,
            )
        });
        recorder.hooks.push(hook);
    }
}
pub(super) fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for (name, size, skew, flags) in [
        ("normal_zero", 17.0, 0.0, 0x20000),
        ("unhinted_zero", 17.0, 0.0, 0),
        ("linear_zero", 17.0, 0.0, 0x20040),
        ("negative_quarter", 17.0, -0.25, 0x20000),
        ("positive_quarter", 17.0, 0.25, 0x20000),
        ("negative_fractional", 17.125, -0.1234567, 0x20000),
        ("positive_fractional", 17.125, 0.1234567, 0x20000),
        (
            "fused_fractional",
            17.125,
            f32::from_bits(0xbc0000f5),
            0x20000,
        ),
        ("negative_large", 23.0, -0.25, 0x20000),
        ("positive_large", 23.0, 0.25, 0x20000),
        ("negative_linear", 17.0, -0.25, 0x20040),
        ("positive_unhinted", 17.0, 0.25, 0),
        ("skew_below_threshold", 19.86, -0.25, 0x20000),
        ("skew_above_threshold", 19.87, -0.25, 0x20000),
        ("fractional_size", 17.12345, -0.25, 0x20000),
        ("tiny_negative", 0.01, -0.25, 0x20000),
        ("tiny_positive", 0.01, 0.25, 0x20000),
        ("tiny_skew", 17.0, -0.0000001, 0x20000),
    ] {
        let mut case = Case::regular(name, "AVx\u{327}\u{301}y", size);
        case.skew = skew;
        case.flags = flags;
        cases.push(case);
    }
    cases
}

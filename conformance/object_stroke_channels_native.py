"""Replay native ObjectStrokeBinaryHandler channel decoding for one saved frame.

The frame is embedded in the fixture, and the fixture retains the original
payload provenance and frame offset. This directly calls the native stroke
handler; it does not invoke the outer ObjectBaseBinaryHandler/SDK loader.
"""
import argparse
import hashlib
import json
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "conformance/object-stroke-channels-native.json"
MODEL_SHA256 = "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a"
MODEL_BASE = 0x4000000
PINNED_DSOS = {
    "FountainPen": "1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec",
    "PenCommon": "afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d",
    "Base": "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
    "Model": MODEL_SHA256,
    "MontblancCalligraphyPen": "75aa5312fba02e21ff01d713187d2fd8d015ac4781e2d58da85ae700114e6f42",
}

FRAME_OFFSET = 137
FRAME_LENGTH = 2514
POINT_COUNT = 204
NEW_APPLY = "SPen::ObjectStrokeBinaryHandler::NewApplyBinary(unsigned char*, int, unsigned int, float)"
GETTERS = {
    "points": ("SPen::ObjectStroke::GetPoint() const", 8),
    "pressures": ("SPen::ObjectStroke::GetPressure() const", 4),
    "timestamps": ("SPen::ObjectStroke::GetTimeStamp() const", 4),
    "tilts": ("SPen::ObjectStroke::GetTilt() const", 4),
    "orientations": ("SPen::ObjectStroke::GetOrientation() const", 4),
}


def f32(x):
    return struct.unpack("<f", struct.pack("<f", x))[0]


def signed_delta(word):
    return (-1.0 if word & 0x8000 else 1.0) * (word & 0x7fff) / 4096.0


def independent_decode(frame):
    """Decode the encoded point and channel stream in its stored order."""
    count = struct.unpack_from("<H", frame, 18)[0]
    if count != POINT_COUNT:
        raise ValueError(f"unexpected point count {count}")
    cursor = 20
    x, y = struct.unpack_from("<dd", frame, cursor)
    cursor += 16
    points = [(f32(x), f32(y))]
    for _ in range(1, count):
        dx, dy = struct.unpack_from("<HH", frame, cursor)
        cursor += 4
        x += (-1.0 if dx & 0x8000 else 1.0) * (dx & 0x7fff) / 32.0
        y += (-1.0 if dy & 0x8000 else 1.0) * (dy & 0x7fff) / 32.0
        # This frame matches a double accumulator followed by f32 getter
        # storage; it does not distinguish every possible XY arithmetic path.
        points.append((f32(x), f32(y)))
    arrays = {"points": b"".join(struct.pack("<2f", *point) for point in points)}

    def take_float_channel(at):
        value = struct.unpack_from("<f", frame, at)[0]
        values = [value]
        at += 4
        for _ in range(1, count):
            word = struct.unpack_from("<H", frame, at)[0]
            at += 2
            value = f32(value + signed_delta(word))
            values.append(value)
        data = b"".join(struct.pack("<f", item) for item in values)
        return data, at

    arrays["pressures"], cursor = take_float_channel(cursor)
    timestamp = struct.unpack_from("<i", frame, cursor)[0]
    cursor += 4
    timestamps = [timestamp]
    for _ in range(1, count):
        timestamp += struct.unpack_from("<H", frame, cursor)[0]
        cursor += 2
        timestamps.append(timestamp)
    arrays["timestamps"] = b"".join(struct.pack("<i", item) for item in timestamps)
    arrays["tilts"], cursor = take_float_channel(cursor)
    arrays["orientations"], cursor = take_float_channel(cursor)
    declared_length = struct.unpack_from("<I", frame, 0)[0]
    if cursor > declared_length:
        raise ValueError(f"independent decoder exceeded declared frame: {cursor}")
    return arrays, cursor


def native_capture(frame, mode=0):
    # Importing this wrapper maps five pinned SPen DSOs and runs their helper
    # setup. The helpers and binaries are hashed into the output fixture.
    sys.path.insert(0, str(ROOT / "conformance"))
    import calligraphy_saved_wrapper_native as wrapper
    n = wrapper.n
    libdir = ROOT / "scratch/apk-analysis-native/arm64-v8a"
    for alloc_name in ("operator new(unsigned long, std::nothrow_t const&)", "operator new[](unsigned long, std::nothrow_t const&)"):
        n.callbacks[alloc_name] = lambda _name=alloc_name: n.x(0, n.alloc(n.x(0)))
    n.callbacks["operator delete(void*, std::nothrow_t const&)"] = lambda: None
    mapped_dsos = {}
    for name, expected_sha in PINNED_DSOS.items():
        path = libdir / f"libSPen{name}.so"
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if digest != expected_sha:
            raise RuntimeError(f"unverified mapped DSO {name}: {digest}")
        mapped_dsos[name] = {"path": str(path.relative_to(ROOT)), "sha256": digest}
    if n.syms.get(NEW_APPLY) is None:
        raise RuntimeError(f"missing native entry {NEW_APPLY}")
    consumed_getters = [getter for getter, _ in GETTERS.values()] + [
        "SPen::ObjectStroke::GetPointCount() const",
        "SPen::ObjectStroke::IsCurveEnabled() const",
        "SPen::ObjectStroke::GetToolType() const",
        "SPen::ObjectStroke::GetStrokeType() const",
        "SPen::ObjectStroke::GetColor() const",
        "SPen::ObjectStroke::GetPenSize() const",
        "SPen::ObjectStroke::IsFixedWidthEnabled() const",
        "SPen::ObjectStroke::GetFixedWidth() const",
        "SPen::ObjectStroke::IsShape() const",
        "SPen::ObjectStroke::GetLineType() const",
        "SPen::ObjectStroke::IsRainbowEffectEnabled() const",
        "SPen::ObjectStroke::GetRainbowDistance() const",
        "SPen::ObjectStroke::GetRainbowOffset() const",
    ]
    for name in [NEW_APPLY, *consumed_getters]:
        if name not in n.syms:
            raise RuntimeError(f"missing native symbol: {name}")
        if name in n.callbacks:
            raise RuntimeError(f"native call unexpectedly overridden by host callback: {name}")

    native_calls = []
    def call_native(name, args=(), floats=()):
        address = n.syms[name]
        record = {"symbol": name, "entry_address": hex(address),
                  "rva": hex(address - MODEL_BASE),
                  "arguments_x": [hex(value & 0xffffffffffffffff) for value in args],
                  "arguments_s0_f32_bits": struct.pack("<f", floats[0]).hex() if floats else None}
        n.call(name, list(args), list(floats))
        record["return_x0"] = hex(n.x(0))
        from unicorn import arm64_const as R
        record["return_s0_f32_bits"] = struct.pack("<I", n.u.reg_read(R.UC_ARM64_REG_S0)).hex()
        native_calls.append(record)
        return record

    obj, impl, handler = n.alloc(0x100), n.alloc(0x400), n.alloc(0x100)
    n.call("SPen::ObjectStroke::ObjectStroke()", [obj])
    n.call("SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)", [impl, obj])
    n.put(obj + 32, "Q", impl)
    n.call("SPen::ObjectStrokeBinaryHandler::ObjectStrokeBinaryHandler(SPen::ObjectStrokeImpl*)", [handler, impl])
    if n.get(handler + 8, "Q")[0] != impl:
        raise RuntimeError("native handler did not retain the expected Impl receiver")
    source = n.alloc(len(frame))
    n.u.mem_write(source, frame)
    apply_call = call_native(NEW_APPLY, (handler, source, len(frame), mode), (1.0,))
    consumed = n.x(0) & 0xffffffff
    count_call = call_native("SPen::ObjectStroke::GetPointCount() const", (obj,))
    count = n.x(0) & 0xffffffff
    arrays = {}
    for key, (getter, elem_size) in GETTERS.items():
        call_native(getter, (obj,))
        ptr = n.x(0)
        data = bytes(n.u.mem_read(ptr, count * elem_size)) if count else b""
        arrays[key] = {"count": count, "element_size": elem_size,
                       "data_hex": data.hex(), "sha256": hashlib.sha256(data).hexdigest()}
    def gi(name):
        call_native(name, (obj,)); return n.x(0) & 0xffffffff
    def gf(name):
        call_native(name, (obj,)); from unicorn import arm64_const as R
        return struct.pack("<I", n.u.reg_read(R.UC_ARM64_REG_S0)).hex()
    state = {
        "curve_enabled": gi("SPen::ObjectStroke::IsCurveEnabled() const"),
        "point_count": count,
        "tool_type": gi("SPen::ObjectStroke::GetToolType() const"),
        "stroke_type": gi("SPen::ObjectStroke::GetStrokeType() const"),
        "color_argb": gi("SPen::ObjectStroke::GetColor() const"),
        "size_bits": gf("SPen::ObjectStroke::GetPenSize() const"),
        "fixed_width_enabled": gi("SPen::ObjectStroke::IsFixedWidthEnabled() const"),
        "fixed_width_bits": gf("SPen::ObjectStroke::GetFixedWidth() const"),
        "shape": gi("SPen::ObjectStroke::IsShape() const"),
        "line_type": gi("SPen::ObjectStroke::GetLineType() const"),
        "rainbow_enabled": gi("SPen::ObjectStroke::IsRainbowEffectEnabled() const"),
        "rainbow_distance": gi("SPen::ObjectStroke::GetRainbowDistance() const"),
        "rainbow_offset_bits": gf("SPen::ObjectStroke::GetRainbowOffset() const"),
    }
    support_dir = ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin"
    import shutil
    tools = {}
    for tool_path in (Path(shutil.which("nm")), support_dir / "aarch64-linux-gnu-objdump"):
        tools[str(tool_path)] = hashlib.sha256(tool_path.read_bytes()).hexdigest()
    helper_files = (Path(__file__), ROOT / "conformance/calligraphy_saved_wrapper_native.py",
                    ROOT / "conformance/calligraphy_saved_native.py", ROOT / "conformance/fountain_native.py")
    helpers = {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
               for path in helper_files}
    return {"consumed_bytes": consumed, "native_state": state, "arrays": arrays,
            "receiver_impl_pointer_after_ctor": hex(n.get(handler + 8, "Q")[0]),
            "handler_symbol": NEW_APPLY,
            "native_calls": native_calls,
            "native_entries": {name: {"address": hex(n.syms[name]), "rva": hex(n.syms[name] - MODEL_BASE)}
                               for name in [NEW_APPLY, *consumed_getters]},
            "mapped_dsos": mapped_dsos, "helper_sha256": helpers, "tool_sha256": tools,
            "new_apply_return_x0": apply_call["return_x0"],
            "getter_call_count": len(consumed_getters)}


def make_synthetic_frame(base_frame, control):
    frame = bytearray(base_frame)
    count = struct.unpack_from("<H", frame, 18)[0]
    pressure_at = 20 + 16 + (count - 1) * 4
    struct.pack_into("<f", frame, pressure_at, control["initial_f32"])
    codes = control["delta_words_le"]
    if len(codes) != count - 1:
        raise ValueError("synthetic pressure delta length mismatch")
    for i, word in enumerate(codes):
        struct.pack_into("<H", frame, pressure_at + 4 + 2 * i, word)
    return bytes(frame)


def make_synthetic_xy_frame(base_frame, control):
    frame = bytearray(base_frame)
    count = struct.unpack_from("<H", frame, 18)[0]
    x_words = [int.from_bytes(bytes.fromhex(control["x_delta_words_le_hex"])[i:i + 2], "little")
               for i in range(0, 2 * (count - 1), 2)]
    y_words = [int.from_bytes(bytes.fromhex(control["y_delta_words_le_hex"])[i:i + 2], "little")
               for i in range(0, 2 * (count - 1), 2)]
    if len(x_words) != count - 1 or len(y_words) != count - 1:
        raise ValueError("synthetic XY delta length mismatch")
    struct.pack_into("<dd", frame, 20, struct.unpack("<d", bytes.fromhex(control["initial_x_f64_bits"]))[0],
                     struct.unpack("<d", bytes.fromhex(control["initial_y_f64_bits"]))[0])
    for index, (x_word, y_word) in enumerate(zip(x_words, y_words)):
        struct.pack_into("<HH", frame, 36 + index * 4, x_word, y_word)
    return bytes(frame)


def point_models(frame):
    count = struct.unpack_from("<H", frame, 18)[0]
    x, y = struct.unpack_from("<dd", frame, 20)
    f_x, f_y = f32(x), f32(y)
    float_values = [(f_x, f_y)]
    double_values = [(f32(x), f32(y))]
    at = 36
    for _ in range(1, count):
        x_word, y_word = struct.unpack_from("<HH", frame, at)
        at += 4
        dx = (-1.0 if x_word & 0x8000 else 1.0) * (x_word & 0x7fff) / 32.0
        dy = (-1.0 if y_word & 0x8000 else 1.0) * (y_word & 0x7fff) / 32.0
        f_x, f_y = f32(f_x + dx), f32(f_y + dy)
        x, y = x + dx, y + dy
        float_values.append((f_x, f_y))
        double_values.append((f32(x), f32(y)))
    pack = lambda rows: b"".join(struct.pack("<ff", px, py) for px, py in rows)
    return pack(float_values), pack(double_values), float_values


def uncompressed_frame_from_source(source_frame, point_overrides):
    count = struct.unpack_from("<H", source_frame, 18)[0]
    x, y = struct.unpack_from("<dd", source_frame, 20)
    raw_points = [(x, y)]
    at = 36
    for _ in range(1, count):
        x_word, y_word = struct.unpack_from("<HH", source_frame, at)
        at += 4
        x += (-1.0 if x_word & 0x8000 else 1.0) * (x_word & 0x7fff) / 32.0
        y += (-1.0 if y_word & 0x8000 else 1.0) * (y_word & 0x7fff) / 32.0
        raw_points.append((x, y))
    arrays, _ = independent_decode(source_frame)
    for index, row in enumerate(point_overrides):
        raw_points[index] = tuple(row)
    stream = b"".join(struct.pack("<dd", *point) for point in raw_points)
    stream += arrays["pressures"] + arrays["timestamps"] + arrays["tilts"] + arrays["orientations"]
    # The compressed frame's post-channel tail consists of tool type plus the
    # 24-byte flexible style record. It is retained byte-for-byte.
    style_tail = source_frame[2488:]
    if len(style_tail) != 26:
        raise ValueError("unexpected saved style tail extent")
    out = bytearray(source_frame[:20]) + stream + style_tail
    out[11] &= 0xfe
    struct.pack_into("<I", out, 0, len(out))
    struct.pack_into("<I", out, 6, len(out) - 24)
    return bytes(out), raw_points


def expected_pressure(frame, round_each=True):
    count = struct.unpack_from("<H", frame, 18)[0]
    at = 20 + 16 + (count - 1) * 4
    value = struct.unpack_from("<f", frame, at)[0]
    out = [value]
    for _ in range(1, count):
        word = struct.unpack_from("<H", frame, at + 4 + 2 * (_ - 1))[0]
        value = value + signed_delta(word)
        if round_each:
            value = f32(value)
        out.append(value)
    return b"".join(struct.pack("<f", x) for x in out)


def generate(template):
    payload = bytes.fromhex(template["source"]["payload_hex"])
    payload_sha = hashlib.sha256(payload).hexdigest()
    if payload_sha != template["source"]["payload_sha256"]:
        raise ValueError("embedded original payload SHA mismatch")
    frame = payload[FRAME_OFFSET:FRAME_OFFSET + FRAME_LENGTH]
    if len(frame) != FRAME_LENGTH or struct.unpack_from("<I", frame)[0] != FRAME_LENGTH:
        raise ValueError("source frame size/declared length mismatch")
    if hashlib.sha256(frame).hexdigest() != template["source"]["frame_sha256"]:
        raise ValueError("embedded frame SHA mismatch")
    decoded, decoded_end = independent_decode(frame)
    native = native_capture(frame)
    if native["consumed_bytes"] != FRAME_LENGTH:
        raise ValueError(f"NewApplyBinary consumed {native['consumed_bytes']}, expected {FRAME_LENGTH}")
    if native["native_state"] != template["expected_native_state"]:
        raise ValueError(f"native getters changed: {native['native_state']}")
    for key, data in decoded.items():
        if bytes.fromhex(native["arrays"][key]["data_hex"]) != data:
            raise ValueError(f"native channel differs from independent f32/raw decoder: {key}")
    expected_synthetic = []
    for control in template.get("synthetic_control_definition", template.get("synthetic_pressure_controls", [])):
        synth = make_synthetic_frame(frame, control)
        result = native_capture(synth)
        pressure = bytes.fromhex(result["arrays"]["pressures"]["data_hex"])
        modeled = expected_pressure(synth)
        modeled_f64 = expected_pressure(synth, round_each=False)
        if pressure != modeled:
            raise ValueError(f"native synthetic pressure recurrence mismatch: {control['id']}")
        expected_synthetic.append({"id": control["id"], "frame_sha256": hashlib.sha256(synth).hexdigest(),
                                   "consumed_bytes": result["consumed_bytes"],
                                   "pressure_data_hex": pressure.hex(),
                                   "pressure_sha256": hashlib.sha256(pressure).hexdigest(),
                                   "native_equals_f32_recurrence": pressure == modeled,
                                   "native_differs_from_f64_accumulation": pressure != modeled_f64,
                                   "new_apply_entry_return": result["native_calls"][0]})
        if pressure == modeled_f64:
            raise ValueError(f"synthetic control failed to distinguish f32 recurrence: {control['id']}")
    arrays = {}
    for key, raw in decoded.items():
        arrays[key] = {"count": POINT_COUNT, "element_size": GETTERS[key][1],
                       "independent_raw_decode_sha256": hashlib.sha256(raw).hexdigest(),
                       **native["arrays"][key]}

    xy_cases = []
    for control in template.get("synthetic_xy_control_definitions", []):
        xy_frame = make_synthetic_xy_frame(frame, control)
        xy_capture = native_capture(xy_frame)
        actual_points = bytes.fromhex(xy_capture["arrays"]["points"]["data_hex"])
        f32_points, f64_points, float_rows = point_models(xy_frame)
        if xy_capture["consumed_bytes"] != FRAME_LENGTH:
            raise ValueError(f"synthetic XY NewApply consumed wrong frame length: {control['id']}")
        if actual_points != f32_points:
            raise ValueError(f"native synthetic XY did not match f32 recurrence: {control['id']}")
        first_diff_f64 = next((i for i in range(POINT_COUNT)
                               if actual_points[i * 8:i * 8 + 8] != f64_points[i * 8:i * 8 + 8]), None)
        first_x_diff_f64 = next((i for i in range(POINT_COUNT)
                                 if actual_points[i * 8:i * 8 + 4] != f64_points[i * 8:i * 8 + 4]), None)
        first_y_diff_f64 = next((i for i in range(POINT_COUNT)
                                 if actual_points[i * 8 + 4:i * 8 + 8] != f64_points[i * 8 + 4:i * 8 + 8]), None)
        native_rows = list(struct.iter_unpack("<ff", actual_points))
        lifted_f64 = b"".join(struct.pack("<dd", float(x), float(y)) for x, y in native_rows)
        xy_cases.append({"id": control["id"],
                         "scope": "synthetic direct native handler input derived from the unchanged real frame; not a second saved-corpus capture",
                         "frame_length": len(xy_frame),
                         "frame_sha256": hashlib.sha256(xy_frame).hexdigest(),
                         "frame_hex": xy_frame.hex(),
                         "initial_x_f64_bits": control["initial_x_f64_bits"],
                         "initial_y_f64_bits": control["initial_y_f64_bits"],
                         "x_delta_words_le_hex": control["x_delta_words_le_hex"],
                         "y_delta_words_le_hex": control["y_delta_words_le_hex"],
                         "new_apply_return_bytes": xy_capture["consumed_bytes"],
                         "new_apply_entry_return": xy_capture["native_calls"][0],
                         "points_data_hex": actual_points.hex(),
                         "points_sha256": hashlib.sha256(actual_points).hexdigest(),
                         "points_lifted_f64_bits_hex": lifted_f64.hex(),
                         "points_lifted_f64_sha256": hashlib.sha256(lifted_f64).hexdigest(),
                         "native_matches_f32_step_model": actual_points == f32_points,
                         "native_matches_double_accumulation_f32_store_model": actual_points == f64_points,
                         "first_differing_point_from_double_model": first_diff_f64,
                         "first_x_differing_point_from_double_model": first_x_diff_f64,
                         "first_y_differing_point_from_double_model": first_y_diff_f64,
                         "sample_x_native": [float_rows[i][0] for i in (0, 1, 2, 3, 4, 5, 6, 7, 8, 16, 32, 64, 128, 203)],
                         "sample_y_native": [float_rows[i][1] for i in (0, 1, 2, 3, 4, 5, 6, 7, 8, 16, 32, 64, 128, 203)],
                         "source_point_channel_start": 20,
                         "native_point_element_size": 8})

    uncompressed = None
    uncompressed_control = template.get("uncompressed_point_control_definition")
    if uncompressed_control:
        overrides = [tuple(struct.unpack("<dd", bytes.fromhex(row)))
                     for row in uncompressed_control["first_point_pairs_f64_bits"]]
        uncompressed_frame, source_points = uncompressed_frame_from_source(frame, overrides)
        captured = native_capture(uncompressed_frame)
        native_points = bytes.fromhex(captured["arrays"]["points"]["data_hex"])
        expected_points = b"".join(struct.pack("<ff", f32(x), f32(y)) for x, y in source_points)
        if captured["consumed_bytes"] != len(uncompressed_frame):
            raise ValueError("native uncompressed NewApply did not consume its full frame")
        if native_points != expected_points:
            raise ValueError("native uncompressed f64 point inputs did not store as independent f32 points")
        native_rows = list(struct.iter_unpack("<ff", native_points))
        lifted_f64 = b"".join(struct.pack("<dd", float(x), float(y)) for x, y in native_rows)
        uncompressed = {"id": uncompressed_control["id"],
                        "scope": "synthetic direct native handler input; native uncompressed point elements are f64, getter storage is PointF f32; this does not establish whether the public parser should preserve source precision or mirror the native getter",
                        "frame_length": len(uncompressed_frame),
                        "frame_sha256": hashlib.sha256(uncompressed_frame).hexdigest(),
                        "frame_hex": uncompressed_frame.hex(),
                        "point_count": POINT_COUNT,
                        "first_input_points_f64_bits": uncompressed_control["first_point_pairs_f64_bits"],
                        "native_return_bytes": captured["consumed_bytes"],
                        "native_entry_return": captured["native_calls"][0],
                        "points_data_hex": native_points.hex(),
                        "points_sha256": hashlib.sha256(native_points).hexdigest(),
                        "points_lifted_f64_bits_hex": lifted_f64.hex(),
                        "points_lifted_f64_sha256": hashlib.sha256(lifted_f64).hexdigest(),
                        "native_matches_independent_f32_store_per_f64_input": native_points == expected_points,
                        "native_getter_state": captured["native_state"],
                        "layout": "uncompressed per-point <f64 x, f64 y>; frame flex offset updated to style record; tool/style tail retained"}

    return {
        "schema": "sdocx-object-stroke-channels-native-v1",
        "runner": {"path": "conformance/object_stroke_channels_native.py",
                   "sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()},
        "native_libraries": native["mapped_dsos"],
        "executed_helpers": native["helper_sha256"],
        "tool_binaries": native["tool_sha256"],
        "source": template["source"],
        "expected_native_state": template["expected_native_state"],
        "native_call": {"symbol": NEW_APPLY, "data_length_argument": FRAME_LENGTH,
                        "mode": 0, "scale_f32_bits": "0000803f", "return_bytes_consumed": native["consumed_bytes"],
                        "entry_address": native["native_entries"][NEW_APPLY]["address"],
                        "rva": native["native_entries"][NEW_APPLY]["rva"],
                        "entry_return_evidence": native["native_calls"][0],
                        "scope": "direct ObjectStrokeBinaryHandler::NewApplyBinary on the real type-1 stroke frame; outer ObjectBaseBinaryHandler and SDK document loading are excluded"},
        "native_receiver": {"handler_impl_pointer_offset": 8,
                             "receiver_impl_pointer_after_ctor": native["receiver_impl_pointer_after_ctor"]},
        "native_getters": native["native_state"],
        "native_entry_return_evidence": native["native_calls"],
        "native_entries": native["native_entries"],
        "callback_exclusion": "NewApplyBinary, ObjectStroke::GetPointCount, and all recorded ObjectStroke getters are asserted present in the native symbol map and absent from the host callback map before invocation",
        "channels": arrays,
        "independent_raw_decode": {"channel_cursor_end": decoded_end,
                                   "frame_declared_length": FRAME_LENGTH,
                                   "trailing_frame_bytes_after_channels": FRAME_LENGTH - decoded_end,
                                   "float_delta_semantics": "f32(previous + signed_u16_magnitude / 4096) after each sample",
                                   "xy_delta_semantics": "This frame matches f64 accumulation of signed deltas / 32 followed by f32 getter storage; a general native XY arithmetic contract is not established."},
        "synthetic_pressure_controls": expected_synthetic,
        "synthetic_control_definition": template.get("synthetic_control_definition", template.get("synthetic_pressure_controls", [])),
        "synthetic_xy_controls": xy_cases,
        "synthetic_xy_control_definitions": template.get("synthetic_xy_control_definitions", []),
        "synthetic_payload_recipe": "For each synthetic frame, concatenate the unchanged original source payload prefix before source.frame_offset with the complete frame_hex field; channel witnesses do not depend on ignored tmp files.",
        "uncompressed_point_control": uncompressed,
        "uncompressed_point_control_definition": uncompressed_control,
    }


def main():
    p = argparse.ArgumentParser(description=__doc__)
    m = p.add_mutually_exclusive_group(required=True)
    m.add_argument("--write-fixture", action="store_true")
    m.add_argument("--check-fixture", action="store_true")
    a = p.parse_args()
    template = json.loads(FIXTURE.read_text())
    actual = generate(template)
    if a.write_fixture:
        FIXTURE.write_text(json.dumps(actual, indent=2, sort_keys=True) + "\n")
        print("wrote conformance/object-stroke-channels-native.json")
    else:
        if json.loads(FIXTURE.read_text()) != actual:
            raise SystemExit("native ObjectStroke channel fixture mismatch")
        print("verified real native frame, pressure controls, XY float controls, and uncompressed f64 input witness")

if __name__ == "__main__":
    main()

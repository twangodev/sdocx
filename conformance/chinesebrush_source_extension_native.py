"""Replay the 24 adversarial ChineseBrush source cases independently of the 52-case base fixture.

The canonical 52-case runner and fixture are pinned and left unchanged. This
extension freshly executes each added ObjectStroke case through that runner's
hash-pinned native caller adapter and compares the complete capture to this
fixture.
"""
from contextlib import redirect_stdout
from pathlib import Path
import argparse
import hashlib
import importlib.util
import io
import json
import os
import shutil
import struct
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
BASE_RUNNER = ROOT / "conformance/chinesebrush_source_native.py"
BASE_RUNNER_SHA256 = "af89a061c317e23aff6e0fafefccd25a88da98d4db80a59e88765886c668be7f"
BASE_FIXTURE = ROOT / "conformance/chinesebrush-source-native.json"
BASE_FIXTURE_SHA256 = "7f353fa59050d6d571c1065ac49fde95a3086bb80bd38a333da08affc7a1ca74"
CHINESE_CALLER_SHA256 = "576846a4a060e3ee11eede9a3b9c626bcdb7b96a07d83a8305f119b628f3b27a"
BRUSHPEN_CALLER_SHA256 = "66f4790e8eeb1ca867f3273a796303b6ffb108286916e1e245fc063cbd744ae0"
HELPER_PINS = {
    "conformance/oblique_canvas_native.py": "1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2",
    "conformance/elf_plt.py": "816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b",
    "conformance/fountain_native.py": "40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30",
}
RUNTIME_PINS = {
    "scratch/apk-analysis-runtime/python/unicorn/__init__.py": "9ad32123bca674cf205c308fd4d0d2a4db877c9dc2afac4feab7c84c8cf7043b",
    "scratch/apk-analysis-runtime/python/unicorn/unicorn_py3/unicorn.py": "62ffe84341f4288e309040e06555b81798031817d32fd19a945e250813f872f0",
    "scratch/apk-analysis-runtime/python/unicorn/arm64_const.py": "92f91a2b0a36cd34deee16db4f8da4ac0c9ae35c3ea1089296e5ef9d170a5be9",
    "scratch/apk-analysis-runtime/python/unicorn/lib/libunicorn.so.2": "ddb196ec82b52e502c18e4a34478bf7b9f61c83c2ebaa95c74d8ded45a95da9c",
    "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump": "b9ae08c9c02c91ced02b80a140c937ba119fe073872fff701f67477bf597a5eb",
    "/usr/bin/nm": "ddb55c0685721bf494aa2d090056b8b32a40614ad4464b305896f527d882c3c1",
    "/usr/bin/c++filt": "7d54ea6797a5e8c69255f711b79577efd9934c3502c6edee4bc25b7dc7d97bd2",
}
FIXTURE = ROOT / "conformance/chinesebrush-source-extension-native.json"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source(points, pressure, timestamps, *, size=3.144, tool=2, width=1920, height=1080):
    count = len(points)
    return {
            "points": points,
            "size": size,
            "tool_type": tool,
            "stroke_type": 0,
            "pressure": pressure,
            "timestamps": timestamps,
            "tilt": [0.12 + i * 0.01 for i in range(count)],
            "orientation": [0.2 + i * 0.02 for i in range(count)],
            "curve_enabled": True,
    }


def case(selector, version, points, pressure, timestamps, *, size=3.144, tool=2, width=1920, height=1080):
    spec = source(points, pressure, timestamps, size=size, tool=tool)
    return selector, spec, width, height, version


COMMON = [[0.0, 0.0], [6.0, 3.0], [9.0, 12.0], [17.0, 13.0], [23.0, 15.0], [31.0, 18.0], [36.0, 28.0], [45.0, 34.0], [50.0, 41.0], [59.0, 43.0], [64.0, 51.0], [73.0, 58.0]]
P456 = [0.25, 0.45, 0.65, 0.85, 0.25, 0.45, 0.65, 0.85, 0.25, 0.45, 0.65, 0.85]
T13 = [200 + 13 * i for i in range(12)]
BASE5 = [[0.0, 0.0], [6.0, 3.0], [9.0, 12.0], [17.0, 13.0], [23.0, 15.0]]
BASE5P = [0.3, 0.55, 0.75, 0.35, 0.95]
CASES = {
    "adv-duplicate-near-gate-v2": case("two", 2, [[0, 0], [0, 0], [3.99, 0], [4, 0], [4.01, 0], [4.01, 0], [8, 0]], [.4, .5, .6, .7, .8, .9, 1], [0, 10, 20, 30, 40, 50, 60]),
    "adv-factor-035-above-0x3eb33336-v2": case("two", 2, [[0, 0], [10, 0], [20, 0], [27.631248474121094, 0]], [.85] * 4, [100, 200, 300, 4300]),
    "adv-factor-035-above-0x3eb33336-v3": case("three", 3, [[0, 0], [10, 0], [20, 0], [27.631248474121094, 0]], [.85] * 4, [100, 200, 300, 4300]),
    "adv-factor-035-near-above-0x3eb33334-v2": case("two", 2, [[0, 0], [10, 0], [20, 0], [27.631250381469727, 0]], [.85] * 4, [100, 200, 300, 4300]),
    "adv-factor-035-near-above-0x3eb33334-v3": case("three", 3, [[0, 0], [10, 0], [20, 0], [27.631250381469727, 0]], [.85] * 4, [100, 200, 300, 4300]),
    "adv-n12-v2": case("two", 2, COMMON, P456, T13),
    "adv-n2-v1": case("one", 1, COMMON[:2], [.3, .9], [100, 111]),
    "adv-n4-v2": case("two", 2, [[0, 0], [5, 2], [9, -1], [14, 7]], [.2, .4, .7, 1.2], [0, 10, 23, 41]),
    "adv-n6-v3": case("three", 3, COMMON[:6], P456[:6], T13[:6]),
    "adv-n9-v3": case("three", 3, COMMON[:9], P456[:9], T13[:9]),
    "adv-pressure-edges-v3": case("three", 3, COMMON[:9], [-.2, 0, .01, .5, 1, 1.2, 2, -1, .65], T13[:9]),
    "adv-size-0_4-v2": case("two", 2, BASE5, P456[:5], T13[:5], size=.4),
    "adv-size-800-v3": case("three", 3, BASE5, P456[:5], T13[:5], size=800),
    "adv-speed-25_4375-scale1-v2": case("two", 2, [[0, 0], [10, 0], [20, 0], [45.4375, 0]], [.85] * 4, [100, 10100, 20100, 30100], width=1920, height=1440),
    "adv-speed-25_4375-scale1-v3": case("three", 3, [[0, 0], [10, 0], [20, 0], [45.4375, 0]], [.85] * 4, [100, 10100, 20100, 30100], width=1920, height=1440),
    "adv-speed-3999-v2": case("two", 2, [[0, 0], [10, 0], [20, 0], [27.631250381469727, 0]], [.25, .45, .65, .85], [100, 200, 300, 4299]),
    "adv-speed-3999-v3": case("three", 3, [[0, 0], [10, 0], [20, 0], [27.631250381469727, 0]], [.25, .45, .65, .85], [100, 200, 300, 4299]),
    "adv-speed-4000-v2": case("two", 2, [[0, 0], [10, 0], [20, 0], [27.631250381469727, 0]], [.25, .45, .65, .85], [100, 200, 300, 4300]),
    "adv-speed-4000-v3": case("three", 3, [[0, 0], [10, 0], [20, 0], [27.631250381469727, 0]], [.25, .45, .65, .85], [100, 200, 300, 4300]),
    "adv-tool0-v2": case("two", 2, COMMON[:6], P456[:6], T13[:6], tool=0),
    "adv-tool3-v3": case("three", 3, COMMON[:6], P456[:6], T13[:6], tool=3),
    "adv-v1-factor-near-035": case("one", 1, [[0, 0], [10, 0], [20, 0], [45.4375, 0]], [.85] * 4, [200, 213, 226, 239]),
    "adv-v2-factor-interior": case("two", 2, BASE5, BASE5P, [1000, 4000, 9000, 14000, 17000]),
    "adv-v3-factor-interior": case("three", 3, BASE5, BASE5P, [1000, 4000, 9000, 14000, 17000]),
}

EXPECTED_STREAM_SHA256 = {
    "adv-duplicate-near-gate-v2": "f89f46272efa4648b2b452370f2c54cecfd002bc8247976a1d8e7358c0e5758e",
    "adv-factor-035-above-0x3eb33336-v2": "3f91dd7ca50682e5e9db7b28ddd49e5d9817d75a862d6ddb6d68e1faea23353b",
    "adv-factor-035-above-0x3eb33336-v3": "3f91dd7ca50682e5e9db7b28ddd49e5d9817d75a862d6ddb6d68e1faea23353b",
    "adv-factor-035-near-above-0x3eb33334-v2": "36b48337e59921343a70766663ef8f2bff4aedf5e6380d32a93e414e893a0c90",
    "adv-factor-035-near-above-0x3eb33334-v3": "36b48337e59921343a70766663ef8f2bff4aedf5e6380d32a93e414e893a0c90",
    "adv-n12-v2": "a16788f42b5b5b88b70ba9d42bf1cfbbf3d287b717f9050db3cd78a7c9d66344",
    "adv-n2-v1": "00c12fee50bb45c9a7a024b995ed074184f03a149fab0a06b471395ee1605267",
    "adv-n4-v2": "2d62b0299c094bf626e0b5058cf45dc48720adb83f252f543cf54394c119eaa7",
    "adv-n6-v3": "3ef035e98c477d092ff3fefab99ae6bbc8b5181d23571d03c16b9275b3bfb204",
    "adv-n9-v3": "43456bc17af4ff8d46f43a2ffb8910f6af852778a734694ef577e48d7d446631",
    "adv-pressure-edges-v3": "cb075d8b0f9d374090003b40b7ef39bab965f5874adc170c00f5ce9936cf2b99",
    "adv-size-0_4-v2": "b4636fc03f340b10381544410ae1a3c61d5bbf34a03bedec57c3e8a114e12d6e",
    "adv-size-800-v3": "3fa216188aa45f1a01c475a21df0659daed67fe2ae6ba3ab15588bec43fface9",
    "adv-speed-25_4375-scale1-v2": "eae368208173b8dadd19df696b9847e3900792e7a860c817409febadce5565f0",
    "adv-speed-25_4375-scale1-v3": "eae368208173b8dadd19df696b9847e3900792e7a860c817409febadce5565f0",
    "adv-speed-3999-v2": "a357c3a92ff743bb1558849d18d83c9c5b053f7a77e9764d81db67014eeb2d4b",
    "adv-speed-3999-v3": "a357c3a92ff743bb1558849d18d83c9c5b053f7a77e9764d81db67014eeb2d4b",
    "adv-speed-4000-v2": "0d7da23c98b74561830b565e843e6aef11837c8afd3b486ff87ff2483b40e430",
    "adv-speed-4000-v3": "0d7da23c98b74561830b565e843e6aef11837c8afd3b486ff87ff2483b40e430",
    "adv-tool0-v2": "3ef035e98c477d092ff3fefab99ae6bbc8b5181d23571d03c16b9275b3bfb204",
    "adv-tool3-v3": "3ef035e98c477d092ff3fefab99ae6bbc8b5181d23571d03c16b9275b3bfb204",
    "adv-v1-factor-near-035": "355460e95744463b41e703c3e0eac90152fa40094d97b499fbc748a3f79112fb",
    "adv-v2-factor-interior": "5f9b53b73a77a8bb0cff79b684215c15d3102cf74e54ec29059adeebe923c5d7",
    "adv-v3-factor-interior": "5f9b53b73a77a8bb0cff79b684215c15d3102cf74e54ec29059adeebe923c5d7",
}


def validate_pins():
    expected = {
        "conformance/chinesebrush_source_native.py": BASE_RUNNER_SHA256,
        "conformance/chinesebrush-source-native.json": BASE_FIXTURE_SHA256,
        "conformance/chinesebrush_saved_caller_native.py": CHINESE_CALLER_SHA256,
        "conformance/brushpen_saved_caller_native.py": BRUSHPEN_CALLER_SHA256,
        **HELPER_PINS,
        **RUNTIME_PINS,
    }
    for rel, digest in expected.items():
        path = Path(rel) if rel.startswith("/") else ROOT / rel
        if sha(path) != digest:
            raise SystemExit(f"ChineseBrush source-extension pin changed: {rel}")
    if len(json.loads(BASE_FIXTURE.read_text())["cases"]) != 52:
        raise SystemExit("pinned ChineseBrush base fixture does not contain 52 cases")
    for tool in ("nm", "c++filt"):
        if Path(shutil.which(tool) or "/missing").resolve() != Path("/usr/bin", tool).resolve():
            raise SystemExit(f"Use the pinned /usr/bin/{tool} on PATH")
    unicorn_spec = importlib.util.find_spec("unicorn")
    if unicorn_spec is None or Path(unicorn_spec.origin).resolve() != (ROOT / "scratch/apk-analysis-runtime/python/unicorn/__init__.py").resolve():
        raise SystemExit("Use the project-local pinned Unicorn runtime")


def load_base_runner():
    spec = importlib.util.spec_from_file_location("chinesebrush_source_native_pinned", BASE_RUNNER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def check_native_input(case_id, value, source_spec, selector):
    """Bind the declared source to executed Model getters and callback registers."""
    def require(condition, field):
        if not condition:
            raise RuntimeError(f"{case_id}: native {field} differs from declared input")

    count = len(source_spec["points"])
    input_record = value["input"]
    channels = {
        "points": (8, "f", [v for point in source_spec["points"] for v in point]),
        "pressures": (4, "f", source_spec["pressure"]),
        "timestamps": (4, "i", source_spec["timestamps"]),
        "tilts": (4, "f", source_spec["tilt"]),
        "orientations": (4, "f", source_spec["orientation"]),
    }
    for name, (element_size, encoding, values) in channels.items():
        require(len(values) == count * (2 if name == "points" else 1), name + " input extent")
        expected = {
            "count": count,
            "element_size": element_size,
            "data_hex": struct.pack("<" + encoding * len(values), *values).hex(),
        }
        require(input_record["source_arrays"][name] == expected, name + " getter bytes/count/element size")

    size_bits = struct.pack("<f", source_spec["size"]).hex()
    model = input_record["native_model_getter_snapshot"]
    require(model == {
        "pen_name": "com.samsung.android.sdk.pen.pen.preload.ChineseBrush",
        "advanced_setting": {"one": "1;", "two": "2;", "three": "3;"}[selector],
        "advanced_setting_getter_called": True,
        "size": struct.unpack("<f", bytes.fromhex(size_bits))[0],
        "size_level": 30,
        "color_argb_u32": "0xff6cdd8f",
        "tool_type": source_spec["tool_type"],
    }, "Model metadata getters")
    snapshots = input_record["native_object_stroke_snapshots"]
    require(len(snapshots) == 1, "ObjectStroke snapshot count")
    for field, expected in {
        "label": "before_drawObjectStroke", "size_bits": size_bits,
        "point_count": count, "curve_enabled": int(source_spec["curve_enabled"]),
        "tool_type": source_spec["tool_type"], "stroke_type": source_spec["stroke_type"],
    }.items():
        require(snapshots[0][field] == expected, field + " getter")
    require(input_record["size_f32_bits"] == size_bits, "source size bits")
    effective_size = min(800.0, max(0.4, struct.unpack("<f", bytes.fromhex(size_bits))[0]))
    require(value["native_selection"]["size_f32_bits"] == struct.pack("<f", effective_size).hex(), "selected Pen size")

    producer = value["producer"]
    rows = producer["append_rows"]
    require(len(rows) == producer["callback_count"], "callback count")
    for i, row in enumerate(rows):
        require(row["before_count_bytes"] == i * 16 and row["after_count_bytes"] == (i + 1) * 16, "append offsets")
        delta = bytes.fromhex(row["delta_bytes"])
        vector = bytes.fromhex(row["vector4_arg_bytes"])
        require(len(delta) == 16, "stored RTV1 record is four float32 lanes / 16 bytes")
        require(len(row["float_args"]) == 5, "diagnostic FP register snapshot")
        require(len(vector) == 16 and vector[:12] == delta[:12], "Vector4 callback input")
        scalar = bytes.fromhex(row["scalar_s0_f32_bits"])
        require(len(scalar) == 4, "separate scalar callback input is one float32")
        require(int(row["float_args"][0], 16) == int.from_bytes(scalar, "little"), "scalar callback input register bits")
    raw = bytes.fromhex(producer["buffer_hex"])
    require(raw == b"".join(bytes.fromhex(row["delta_bytes"]) for row in rows), "complete append stream")
    require(len(raw) == producer["buffer_bytes"] == len(rows) * 16, "buffer extent")
    require(hashlib.sha256(raw).hexdigest() == producer["buffer_sha256"], "buffer hash")


def capture(case_id):
    runner = load_base_runner()
    runner.SOURCE_CASES = CASES
    output = io.StringIO()
    with redirect_stdout(output):
        runner.capture(case_id)
    value = json.loads(output.getvalue())
    selector, source_spec, width, height, version = CASES[case_id]
    check_native_input(case_id, value, source_spec, selector)
    if value["native_selection"]["version"] != version:
        raise RuntimeError(f"{case_id}: selected version mismatch")
    if value["profile"]["width"] != width or value["profile"]["height"] != height:
        raise RuntimeError(f"{case_id}: display profile mismatch")
    actual_stream = value["producer"]["buffer_sha256"]
    if actual_stream != EXPECTED_STREAM_SHA256[case_id]:
        raise RuntimeError(f"{case_id}: native stream changed: {actual_stream}")
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--write-fixture", action="store_true")
    mode.add_argument("--check-fixture", action="store_true")
    parser.add_argument("--_capture", choices=tuple(CASES))
    args = parser.parse_args()
    validate_pins()
    if args._capture:
        print(json.dumps(capture(args._capture), sort_keys=True))
        return

    captures = {}
    index = []
    for case_id, (selector, spec, width, height, version) in CASES.items():
        result = subprocess.run(
            [sys.executable, str(Path(__file__).resolve()), "--_capture", case_id],
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=True,
        )
        value = json.loads(result.stdout)
        captures[case_id] = value
        index.append({
            "id": case_id,
            "version": version,
            "curve_enabled": spec["curve_enabled"],
            "callback_count": value["producer"]["callback_count"],
            "stream_sha256": value["producer"]["buffer_sha256"],
        })
    fixture = {
        "schema": "chinesebrush-source-extension-native-v1",
        "provenance": {
            "runner_sha256": sha(Path(__file__)),
            "baseline_runner_sha256": BASE_RUNNER_SHA256,
            "baseline_fixture_sha256": BASE_FIXTURE_SHA256,
            "chinesebrush_saved_caller_sha256": CHINESE_CALLER_SHA256,
            "brushpen_saved_caller_sha256": BRUSHPEN_CALLER_SHA256,
            "helper_source_sha256": HELPER_PINS,
            "runtime_tool_sha256": RUNTIME_PINS,
        },
        "cases": captures,
        "case_index": index,
        "boundary": "Fresh native replay of 24 synthetic adversarial ObjectStroke sources, combined with the separately pinned 52-case base fixture for a total 76-case source corpus. The one-event rejection remains a separate boundary and is not counted as a generated stream. Device deserialization, GLES rendering, and pixel parity are outside this oracle.",
    }
    encoded = json.dumps(fixture, indent=2, sort_keys=True) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)} ({len(captures)} extension cases)")
    elif not FIXTURE.exists() or json.loads(FIXTURE.read_text()) != fixture:
        raise SystemExit("ChineseBrush extension fixture mismatch; use --write-fixture to intentionally refresh")
    else:
        print("verified 52 pinned baseline + 24 fresh adversarial ChineseBrush cases (76 total)")


if __name__ == "__main__":
    main()

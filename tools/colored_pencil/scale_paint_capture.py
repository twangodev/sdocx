"""Join captured native display scales and source records to native depth paint."""

import ast
import hashlib
from pathlib import Path

import depth_capture as depth

ROOT = Path(__file__).resolve().parents[2]
DEPTH = ROOT / "tools/colored_pencil/depth_capture.py"
DEPTH_SHA = "008ba076b589f4e5b0578fa158c4a3c95b2889dd18ac059c334ff1d65988ee6b"
SCALES = ROOT / "conformance/colored-pencil-v4-background-scale-native.json"
PRIOR_DEPTH = ROOT / "conformance/colored-pencil-v4-depth-native.json"

SETUP = """    scales = json.loads(SCALES.read_bytes())
    grid = json.loads(PRIOR_DEPTH.read_bytes())['depth_inputs']['grid']
    patterns = {'grid': (grid['width'], grid['height'], bytes.fromhex(grid['rgba_hex']))}
    streams, metadata = {}, {}
    for case in scales['cases']:
        row = next(row for row in geometry['cases'] if row['id'] == case['id'])
        stream = bytes.fromhex(row['stream_hex'])
        assert sha(stream) == case['geometry_stream_sha256']
        background_scale = struct.unpack('<f', bytes.fromhex(case['background_scale_f32_bits']))[0]
        name = f"{row['id']}_grid_display"
        streams[name] = stream
        metadata[name] = {'source': row['id'], 'depth': 'grid', 'background_scale': background_scale, 'size': struct.unpack('<f', bytes.fromhex(row['pen_data']['size_f32_bits']))[0]}
"""


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    source = DEPTH.read_text()
    assert sha(source.encode()) == DEPTH_SHA
    tree = ast.parse(source)
    function = next(
        n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "main"
    )
    body = ast.get_source_segment(source, function)
    start = body.index("    patterns = {")
    stop = body.index("    tree = ast.parse(BATCH.read_text())", start)
    body = body[:start] + SETUP + body[stop:]
    for old, new in [
        (
            "sdocx-colored-pencil-v4-depth-native-v1",
            "sdocx-colored-pencil-v4-display-depth-native-v1",
        ),
        (
            "tools/colored_pencil/depth_capture.py",
            "tools/colored_pencil/scale_paint_capture.py",
        ),
        ('assert len(receipt["cases"]) == 360', 'assert len(receipt["cases"]) == 96'),
        (
            "conformance/colored-pencil-v4-depth-native.json",
            "conformance/colored-pencil-v4-display-depth-native.json",
        ),
    ]:
        assert body.count(old) == 1
        body = body.replace(old, new)
    target = '            "geometry_receipt_sha256": sha(GEOMETRY.read_bytes()),\n'
    assert body.count(target) == 1
    body = body.replace(
        target,
        target
        + '            "scale_producer_receipt_sha256": sha(SCALES.read_bytes()),\n            "depth_acquisition_script_sha256": DEPTH_SHA,\n            "depth_input_receipt_sha256": sha(PRIOR_DEPTH.read_bytes()),\n',
    )
    original_scope = next(
        ast.literal_eval(value)
        for node in ast.walk(function)
        if isinstance(node, ast.Dict)
        for key, value in zip(node.keys, node.values)
        if isinstance(key, ast.Constant) and key.value == "scope"
    )
    assert body.count(original_scope) == 1
    body = body.replace(
        original_scope,
        "Native display-dependent source records and native-produced background scales, joined as explicit replay inputs to the pinned unmodified RTV1 depth paint route. Reuses the prior native grid bitmap bytes and verifies fresh native bitmap upload, setter readback, unit-1 binding, scale uniforms and every actual prior upload/readback. Drawing/selected Redraw scale production is independently captured; this is a producer-to-paint join, not a full native Drawing paint pass, SDK deserialization or device parity. No native fields, sampler, shader or uniform overrides are added.",
    )
    scope = dict(
        vars(depth),
        __file__=str(Path(__file__).resolve()),
        SCALES=SCALES,
        PRIOR_DEPTH=PRIOR_DEPTH,
        DEPTH_SHA=DEPTH_SHA,
    )
    exec(compile(body, str(DEPTH), "exec"), scope)
    scope["main"]()


if __name__ == "__main__":
    main()

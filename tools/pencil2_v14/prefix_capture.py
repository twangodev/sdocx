"""Isolate page-origin 2x residuals with unchanged native Update/Draw prefixes."""

import hashlib
import json
import os
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = (
    ROOT
    / "tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/capture_candidate.py"
)
SOURCE_SHA = "34d5874cb86e27b619c01e9902b0aacf743619be560d8a4ca5ee65789ca8193d"
PIXELS = [(11, 190), (23, 190), (9, 187), (21, 187)]
no_linear = "--no-linear" in sys.argv[1:]
if no_linear:
    os.environ["LP_PERF"] = "no_rast_linear"
else:
    assert not os.environ.get("LP_PERF"), (
        "default capture requires default Mesa performance flags"
    )
colored = (ROOT / "conformance/pencil2-v14-color-80336699-native.bin").read_bytes()[
    :4704
]
second = (ROOT / "conformance/pencil2-v14-color-ffdd4411-native.bin").read_bytes()
black = (ROOT / "conformance/pencil2-v14-native.bin").read_bytes()
sources = [colored, second, black[14112:18816], black[18816:23520], black[23520:28224]]
mixed = b"".join(sources[i % 5][i * 56 : (i + 1) * 56] for i in range(84))
streams = {
    "colored": colored,
    "mixed": mixed,
    "mixed_reversed": b"".join(
        reversed([mixed[i : i + 56] for i in range(0, 4704, 56)])
    ),
}
INJECT = """    prefix_pixel=C.create_string_buffer(4)
    p.u.mem_write(matrix,struct.pack('<16f',2,0,0,0, 0,-2,0,0, 0,0,1,0, 0,192,0,1))
    for name, stream in PREFIX_STREAMS.items():
        trace=[]
        for end in range(1,85):
            prefix=bytearray(stream)
            for i in range(end,84): struct.pack_into('<f',prefix,i*56+16,0.)
            p.u.mem_write(vector_begin,bytes(prefix))
            p.call(RT_UPDATE,[rt,0],instruction_limit=5_000_000)
            p.u.mem_write(vector_begin,actual_geometry)
            p.call(ACTIVATE_SUBBITMAP_RT,[subbitmap,0],instruction_limit=5_000_000)
            f['glViewport'](0,0,width,height)
            f['glClearColor'](0.,0.,0.,0.); f['glClear'](0x00004000)
            f['glBindFramebuffer'](0x8d40,0)
            p.call(RT_DRAW,[rt,subbitmap,matrix],instruction_limit=8_000_000)
            f['glBindFramebuffer'](0x8d40,draw_target_fbo)
            values=[]
            for x,y in PREFIX_PIXELS:
                f['glReadPixels'](x,y,1,1,0x1908,0x1401,prefix_pixel)
                values.append(list(prefix_pixel.raw))
            if f['glGetError']():raise RuntimeError('prefix GL error')
            trace.append(values)
        f['glReadPixels'](0,0,width,height,0x1908,0x1401,pixels)
        PREFIX_RESULTS[name]={'prefix_rgba':trace,'full_sha256':hashlib.sha256(pixels.raw).hexdigest()}
        print(name,trace[-1],flush=True)
    p.u.mem_write(vector_begin,actual_geometry)
    p.call(RT_UPDATE,[rt,0],instruction_limit=5_000_000)
"""
source = SOURCE.read_text()
assert hashlib.sha256(source.encode()).hexdigest() == SOURCE_SHA
needle = '    graphics=p.get(rt+200,"Q")[0]'
assert source.count(needle) == 1
namespace = {
    "__name__": "pencil2_prefix_capture",
    "__file__": str(SOURCE),
    "PREFIX_STREAMS": streams,
    "PREFIX_PIXELS": PIXELS,
    "PREFIX_RESULTS": {},
}
exec(compile(source.replace(needle, INJECT + needle), str(SOURCE), "exec"), namespace)
with tempfile.TemporaryDirectory(prefix="sdocx-pencil2-prefix-") as directory:
    baseline = namespace["capture"](Path(directory))
page = ROOT / "conformance/pencil2-v14-page-context-native.json"
expected = json.loads(page.read_text())["results"]
for name, result in namespace["PREFIX_RESULTS"].items():
    assert result["full_sha256"] == expected[name + "_top_scale2"]["sha256"]
receipt = {
    "schema": "sdocx-pencil2-v14-page-prefix-native-v1",
    "frame": [192, 192],
    "pixels_bottom_origin": PIXELS,
    "context": {"scale": 2, "y_up": False, "geometry_inverse_scale": 1},
    "capture": {
        "script": "tools/pencil2_v14/prefix_capture.py",
        "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "base_source_sha256": SOURCE_SHA,
        "page_receipt_sha256": hashlib.sha256(page.read_bytes()).hexdigest(),
        "baseline_sha256": baseline["mesa"]["readback_sha256"],
        "dso_pins": namespace["DSO_PINS"],
    },
    "input_sha256": {
        name: hashlib.sha256(data).hexdigest() for name, data in streams.items()
    },
    "results": namespace["PREFIX_RESULTS"],
    "scope": "Three synthetic native-record streams, 84 prefixes each, four RGBA8 pixels. Later records retain all fields except zero alpha at the explicit GPU-upload boundary. Native Update, Draw, shaders and textures unchanged; full-prefix frame hashes must match prior uninstrumented captures. Host Mesa; not saved-note or device evidence.",
}
output = ROOT / "conformance/pencil2-v14-page-prefix-native.json"
if no_linear:
    original = output
    reference = json.loads(original.read_text())
    assert receipt["results"] == reference["results"]
    receipt["schema"] = "sdocx-pencil2-v14-page-no-linear-native-v1"
    receipt["capture"]["environment"] = {"LP_PERF": "no_rast_linear"}
    receipt["capture"]["prefix_receipt_sha256"] = hashlib.sha256(
        original.read_bytes()
    ).hexdigest()
    receipt["results"] = {
        name: {"full_sha256": data["full_sha256"], "all_84_prefixes_unchanged": True}
        for name, data in receipt["results"].items()
    }
    receipt["scope"] = (
        "Same native inputs, Update/Draw and four sampled pixels as the default prefix capture. Disabling Mesa's linear rasterizer leaves all 252 prefixes and all three full-frame hashes unchanged. This rules out that optional rasterizer as the cause of these residuals; not a universal pipeline or device claim."
    )
    output = ROOT / "conformance/pencil2-v14-page-no-linear-native.json"
output.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
print(output, output.stat().st_size)

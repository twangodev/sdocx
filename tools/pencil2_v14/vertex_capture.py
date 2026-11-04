"""Observe native clip positions without changing Pencil2 shader sources."""

import ast
import base64
import hashlib
import json
import tempfile
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PREFIX = ROOT / "tools/pencil2_v14/prefix_capture.py"
SOURCE = (
    ROOT
    / "tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/capture_candidate.py"
)
SOURCE_SHA = "34d5874cb86e27b619c01e9902b0aacf743619be560d8a4ca5ee65789ca8193d"
controller = PREFIX.read_text()
# Parse constants without running the acquisition controller twice.
injection = next(
    ast.literal_eval(n.value)
    for n in ast.parse(controller).body
    if isinstance(n, ast.Assign)
    and any(isinstance(t, ast.Name) and t.id == "INJECT" for t in n.targets)
)
link = """        def feedback_link(self):
            names=(C.c_char_p*1)(b'gl_Position')
            self._cfunc('glTransformFeedbackVaryings',None,[C.c_uint,C.c_int,C.POINTER(C.c_char_p),C.c_uint])(self.x(0),1,names,0x8c8c)
            self._gl('glLinkProgram',1)

"""
begin = """            if end==84:
                tf=C.c_uint()
                p._cfunc('glGenBuffers',None,[C.c_int,C.POINTER(C.c_uint)])(1,C.byref(tf))
                p._cfunc('glBindBuffer',None,[C.c_uint,C.c_uint])(0x8c8e,tf.value)
                p._cfunc('glBufferData',None,[C.c_uint,C.c_ssize_t,C.c_void_p,C.c_uint])(0x8c8e,84*6*16,None,0x88e1)
                p._cfunc('glBindBufferBase',None,[C.c_uint,C.c_uint,C.c_uint])(0x8c8e,0,tf.value)
                p.feedback_active=True
"""
end = """            if end==84:
                p.feedback_active=False
                mapped=p._cfunc('glMapBufferRange',C.c_void_p,[C.c_uint,C.c_ssize_t,C.c_ssize_t,C.c_uint])(0x8c8e,0,84*6*16,1)
                if not mapped:raise RuntimeError('feedback map failed')
                FEEDBACK[name]=C.string_at(mapped,84*6*16)
                if not p._cfunc('glUnmapBuffer',C.c_ubyte,[C.c_uint])(0x8c8e):raise RuntimeError('feedback unmap failed')
                p._cfunc('glBindBufferBase',None,[C.c_uint,C.c_uint,C.c_uint])(0x8c8e,0,0)
                p._cfunc('glBindBuffer',None,[C.c_uint,C.c_uint])(0x8c8e,0)
                p._cfunc('glDeleteBuffers',None,[C.c_int,C.POINTER(C.c_uint)])(1,C.byref(tf))
"""
needle = "            p.call(RT_DRAW,[rt,subbitmap,matrix],instruction_limit=8_000_000)"
assert injection.count(needle) == 1
injection = injection.replace(needle, begin + needle + "\n" + end)
source = SOURCE.read_text()
assert hashlib.sha256(source.encode()).hexdigest() == SOURCE_SHA
source = source.replace(
    "            result = fn(*values)",
    """            if name=='glDrawArraysInstanced' and getattr(self,'feedback_active',False):
                self._cfunc('glBeginTransformFeedback',None,[C.c_uint])(4)
            result = fn(*values)
            if name=='glDrawArraysInstanced' and getattr(self,'feedback_active',False):
                self._cfunc('glEndTransformFeedback',None,[])()""",
)
source = source.replace(
    "        def _install_gl(self):", link + "        def _install_gl(self):"
)
source = source.replace(
    '"glLinkProgram": lambda: self._gl("glLinkProgram", 1)',
    '"glLinkProgram": self.feedback_link',
)
# Reuse input construction without executing the prefix capture.
setup = controller[: controller.index("INJECT =")]
namespace = {"__name__": "pencil2_vertex_capture", "__file__": str(PREFIX)}
exec(compile(setup, str(PREFIX), "exec"), namespace)
namespace.update(
    __file__=str(SOURCE),
    PREFIX_STREAMS=namespace["streams"],
    PREFIX_PIXELS=namespace["PIXELS"],
    PREFIX_RESULTS={},
    FEEDBACK={},
)
needle = '    graphics=p.get(rt+200,"Q")[0]'
assert source.count(needle) == 1
exec(
    compile(source.replace(needle, injection + needle), str(SOURCE), "exec"), namespace
)
with tempfile.TemporaryDirectory(prefix="sdocx-pencil2-feedback-") as directory:
    baseline = namespace["capture"](Path(directory))
original = ROOT / "conformance/pencil2-v14-page-prefix-native.json"
reference = json.loads(original.read_text())
valid = namespace["PREFIX_RESULTS"] == reference["results"]
comparison = {
    name: {
        "inactive_83_prefixes_unchanged": data["prefix_rgba"][:83]
        == reference["results"][name]["prefix_rgba"][:83],
        "changed_prefix_ordinals": [
            i + 1
            for i, (a, b) in enumerate(
                zip(data["prefix_rgba"], reference["results"][name]["prefix_rgba"])
            )
            if a != b
        ],
        "changed_prefix_pixels": sum(
            a != b
            for row, native in zip(
                data["prefix_rgba"], reference["results"][name]["prefix_rgba"]
            )
            for a, b in zip(row, native)
        ),
        "original_full_sha256": reference["results"][name]["full_sha256"],
        "feedback_full_sha256": data["full_sha256"],
    }
    for name, data in namespace["PREFIX_RESULTS"].items()
}
receipt = {
    "schema": "sdocx-pencil2-v14-page-vertex-native-v1",
    "capture": {
        "script": "tools/pencil2_v14/vertex_capture.py",
        "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "prefix_controller_sha256": hashlib.sha256(PREFIX.read_bytes()).hexdigest(),
        "base_source_sha256": SOURCE_SHA,
        "prefix_receipt_sha256": hashlib.sha256(original.read_bytes()).hexdigest(),
        "baseline_sha256": baseline["mesa"]["readback_sha256"],
    },
    "valid_as_original_pipeline_oracle": valid,
    "comparison": comparison,
    "prefix_results": namespace["PREFIX_RESULTS"],
    "fields": ["clipX", "clipY", "clipZ", "clipW"],
    "vertices_per_record": 6,
    "records": 84,
    "results": {
        name: {
            "sha256": hashlib.sha256(data).hexdigest(),
            "f32_zlib_base64": base64.b64encode(zlib.compress(data, 9)).decode(),
        }
        for name, data in namespace["FEEDBACK"].items()
    },
    "scope": "Transform feedback captures gl_Position from unchanged native shader sources, prior to clipping and viewport. Feedback remains inactive for prefixes 1-83, which match the original observations. Enabling feedback only for prefix 84 changes the original pixels and full-frame hashes; this is a rejected oracle for original clipping/interpolation. Prefix and full-frame comparisons are retained as negative evidence. Synthetic host-Mesa record replay; not saved-note or device evidence.",
}
output = ROOT / "conformance/pencil2-v14-page-vertex-native.json"
output.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
print(output, output.stat().st_size)

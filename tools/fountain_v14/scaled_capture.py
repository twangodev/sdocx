"""Join persisted native Drawing's scaled source to unchanged native RT paint."""

import ast
import hashlib
import json
from pathlib import Path

import capture

ROOT = Path(__file__).resolve().parents[2]
BASE = ROOT / "tools/fountain_v14/capture.py"
BASE_SHA = "282e77a424a1694b63b46f4c9a8d08bee376ffdbce31b5d848d851e3fc405bd8"
DRAWING = ROOT / "conformance/fountain-v14-drawing-scale-native.json"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    source = BASE.read_text()
    assert sha(source.encode()) == BASE_SHA
    drawing = json.loads(DRAWING.read_bytes())
    arguments = {
        scale: next(
            row for row in drawing["cases"] if row["id"] == f"top_{scale}_crop"
        )["addpoint_arguments"]
        for scale in [1, 2]
    }
    injection = capture.INJECT
    old = "  for vals in caller_inputs:p.call(add,[rt],vals)"
    assert injection.count(old) == 1
    injection = injection.replace(
        old, "  for vals in SCALED_ARGUMENTS[scale]:p.call(add,[rt],vals)"
    )
    old = "if len(stream)!=24*36:"
    assert injection.count(old) == 1
    injection = injection.replace(
        old, "if len(stream)!=len(SCALED_ARGUMENTS[scale])*36:"
    )
    function = next(
        node
        for node in ast.parse(source).body
        if isinstance(node, ast.FunctionDef) and node.name == "main"
    )
    body = ast.get_source_segment(source, function)
    replacements = [
        ('"NEW_CASES": {},', '"NEW_CASES": {}, "SCALED_ARGUMENTS": SCALED_ARGUMENTS,'),
        ("tools/fountain_v14/capture.py", "tools/fountain_v14/scaled_capture.py"),
        (
            "sdocx-fountain-v14-real-source-rgba-native-v1",
            "sdocx-fountain-v14-scaled-source-rgba-native-v1",
        ),
        (
            "conformance/fountain-v14-real-source-rgba-native.json",
            "conformance/fountain-v14-scaled-source-rgba-native.json",
        ),
        (
            '"native_helper_sha256": HELPER_SHA,',
            '"native_helper_sha256": HELPER_SHA, "source_capture_script_sha256": BASE_SHA, "drawing_scale_receipt_sha256": sha(DRAWING.read_bytes()),',
        ),
    ]
    for old, new in replacements:
        assert body.count(old) == 1, old
        body = body.replace(old, new)
    old_scope = next(
        ast.literal_eval(value)
        for node in ast.walk(function)
        if isinstance(node, ast.Dict)
        for key, value in zip(node.keys, node.values)
        if isinstance(key, ast.Constant) and key.value == "scope"
    )
    body = body.replace(
        old_scope,
        "Actual persisted native Model/Drawing source sampling at matching 1x/2x matrices is joined through its complete six-float callback tuples to the pinned unmodified RTV4 paint route. The real saved source emits 24 stamps at 1x and 49 at 2x; source arrays are unchanged. Native RT AddPoint/Update/Draw, inverse scale and three actual verified priors execute with unchanged shader/sampler paths. Both origins and three RT color/alpha choices. Explicit enhanced-AA/redraw=true and fresh private coverage. No geometry buffer, native field, shader, uniform or sampler rewrite. Producer and paint remain separate captures, not one full native Drawing-to-GPU saved page or device observation.",
    )
    scope = dict(
        vars(capture),
        __file__=str(Path(__file__).resolve()),
        INJECT=injection,
        SCALED_ARGUMENTS=arguments,
        BASE_SHA=BASE_SHA,
        DRAWING=DRAWING,
    )
    exec(compile(body, str(BASE), "exec"), scope)
    scope["main"]()


if __name__ == "__main__":
    main()

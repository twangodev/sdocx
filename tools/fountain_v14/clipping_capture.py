"""Capture native Fountain viewport clipping of a real scaled saved source."""

import ast
from pathlib import Path

import capture
import scaled_capture

ROOT = Path(__file__).resolve().parents[2]
SCALED = ROOT / "tools/fountain_v14/scaled_capture.py"
SCALED_SHA = "a5095c045af30b390bd93790a8dd0822154c2aa7e4fbf15257e91066937bf592"

VIEWS = "[('left',[924,1435-32//scale]),('right',[924-64//scale,1435-32//scale]),('top',[921-32//scale,1440]),('bottom',[921-32//scale,1440-64//scale]),('left_top',[924,1440]),('right_top',[924-64//scale,1440]),('left_bottom',[924,1440-64//scale]),('right_bottom',[924-64//scale,1440-64//scale]),('outside',[0,0])]"


def main():
    source = SCALED.read_text()
    assert scaled_capture.sha(source.encode()) == SCALED_SHA
    injection = capture.INJECT
    anchor = "  source_origin=[921-32//scale,1435-32//scale]\n"
    start = injection.index(anchor)
    tail = injection[start + len(anchor) :]
    injection = (
        injection[:start]
        + f"  for view,source_origin in {VIEWS}:\n"
        + "\n".join(" " + line if line else line for line in tail.splitlines())
        + "\n"
    )
    for old, new in [
        ("[0xff000000,0x8044aaee,0x00c04020]", "[0xff000000,0x8044aaee]"),
        (
            "name=f'{origin}_{scale}_{argb:08x}_{prior_name}'",
            "name=f'{view}_{origin}_{scale}_{argb:08x}_{prior_name}'",
        ),
        (
            "={'origin':origin,'scale':scale",
            "={'view':view,'origin':origin,'scale':scale",
        ),
    ]:
        assert injection.count(old) == 1, old
        injection = injection.replace(old, new)
    capture.INJECT = injection
    function = next(
        node
        for node in ast.parse(source).body
        if isinstance(node, ast.FunctionDef) and node.name == "main"
    )
    body = ast.get_source_segment(source, function)
    for old, new in [
        (
            "tools/fountain_v14/scaled_capture.py",
            "tools/fountain_v14/clipping_capture.py",
        ),
        (
            "sdocx-fountain-v14-scaled-source-rgba-native-v1",
            "sdocx-fountain-v14-clipping-native-v1",
        ),
        (
            "conformance/fountain-v14-scaled-source-rgba-native.json",
            "conformance/fountain-v14-clipping-native.json",
        ),
    ]:
        assert body.count(old) == 1, old
        body = body.replace(old, new)
    anchor = "    for old, new in replacements:\n"
    assert body.count(anchor) == 1
    body = body.replace(
        anchor,
        "    replacements.append(('assert len(receipt[\"cases\"]) == 36', 'assert len(receipt[\"cases\"]) == 216'))\n"
        + anchor,
    )
    old_scope = next(
        node.args[1].value
        for node in ast.walk(function)
        if isinstance(node, ast.Call)
        and isinstance(node.func, ast.Attribute)
        and node.func.attr == "replace"
        and len(node.args) == 2
        and isinstance(node.args[0], ast.Name)
        and node.args[0].id == "old_scope"
        and isinstance(node.args[1], ast.Constant)
    )
    body = body.replace(
        old_scope,
        "Native Fountain RTV4 viewport clipping with unchanged real saved source and Drawing-matched 24/49-stamp streams. Four edges, four corners and fully outside at both origins and 1x/2x, two RT colors and three actual verified priors. Projection matrices are public Draw inputs; native AddPoint/Update/Draw and shaders/samplers remain unmodified. Masks and output RGBA come from actual target readback. Same original baseline and host boundaries as the pinned scaled-source acquisition. Explicit fresh enhanced-AA/redraw context; separate source and paint captures, not one full Drawing-to-GPU page or device validation.",
    )
    scope = dict(vars(scaled_capture), __file__=str(Path(__file__).resolve()))
    exec(compile(body, str(SCALED), "exec"), scope)
    scope["main"]()


if __name__ == "__main__":
    main()

"""Observe native Drawing/GL/RT depth-map lifetime across repeated redraws."""

import hashlib
import json
from pathlib import Path

import context_capture as acquire

ROOT = Path(__file__).resolve().parents[2]
ACQUIRE = ROOT / "tools/colored_pencil/context_capture.py"
ACQUIRE_SHA = "8798fd6426c76c7660484eccdb95f91a24ed803a09361f606b5466765bf5db47"
GEOMETRY = ROOT / "conformance/colored-pencil-v4-source-context-native.json"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    assert sha(ACQUIRE.read_bytes()) == ACQUIRE_SHA
    case = next(row for row in acquire.controls() if row["id"] == "size_8")
    namespace, capture = acquire.run_geometry(case)
    original = next(
        row
        for row in json.loads(GEOMETRY.read_bytes())["cases"]
        if row["id"] == case["id"]
    )
    assert acquire.compact(case, capture) == original
    p = namespace["p"]
    registers = namespace["R"]
    gl, rt = namespace["selected_drawable"], namespace["selected_rt"]
    adapters, stubs, references, events = {}, {}, {}, []
    specifications = [
        ("A", 31, 17, True),
        ("B", 1, 1, True),
        ("no_texture", 31, 17, False),
    ]
    for index, (name, width, height, valid) in enumerate(specifications):
        bitmap, vtable = p.alloc(160), p.alloc(192)
        p.put(bitmap, "Q", vtable)
        adapters[bitmap] = {
            "name": name,
            "width": width,
            "height": height,
            "texture": 0x40001000 + index * 16 if valid else 0,
        }
        references[bitmap] = 1
        for slot, method in [
            (72, "width"),
            (80, "height"),
            (128, "retain"),
            (136, "release"),
            (144, "texture"),
        ]:
            stub = p.alloc(4)
            p.u.mem_write(stub, bytes.fromhex("c0035fd6"))
            p.put(vtable + slot, "Q", stub)
            stubs[stub] = method
    by_name = {row["name"]: pointer for pointer, row in adapters.items()}

    def label(pointer):
        return adapters[pointer]["name"] if pointer else None

    def snapshot():
        return {
            "drawing": label(p.get(p.drawing + 48, "Q")[0]),
            "gl": label(p.get(gl + 280, "Q")[0]),
            "rt": label(p.get(rt + 248, "Q")[0]),
            "rt_dimensions": list(p.get(rt + 256, "2f")),
            "references": {
                row["name"]: references[pointer] for pointer, row in adapters.items()
            },
        }

    methods = {
        p.syms[signature]: name
        for signature, name in [
            ("SPen::ObjectDrawing::SetDepthmap(SPen::ISPBitmap*)", "drawing_set"),
            (
                "SPen::DrawingUtil::SetPenDepthmapBitmap(SPen::ISPBitmap*, SPen::IPen*, SPen::StrokeType)",
                "drawing_handoff",
            ),
            (
                "SPen::ColoredPencilStrokeDrawableGLV4::SetDepthMapBitmap(SPen::ISPBitmap*)",
                "gl_set",
            ),
            (
                "SPen::PenReturnCallback::SetDepthMapBitmap(SPen::ISPBitmap*)",
                "callback_set",
            ),
            (
                "SPen::ColoredPencilStrokeDrawableRTV1::SetDepthMapBitmap(SPen::ISPBitmap*)",
                "rt_set",
            ),
            ("SPen::ColoredPencilStrokeDrawableRTV1::Clear()", "rt_clear"),
        ]
    }

    def observe(u, address, size, data):
        if address in stubs:
            bitmap = p.x(0)
            adapter = adapters[bitmap]
            method = stubs[address]
            value = adapter.get(method, 0)
            if method == "retain":
                references[bitmap] += 1
                value = references[bitmap]
            elif method == "release":
                references[bitmap] -= 1
                value = references[bitmap]
                assert value >= 1, "host owner must remain alive"
            events.append(
                {
                    "kind": "host_bitmap_" + method,
                    "bitmap": adapter["name"],
                    "result": value,
                    "phase": namespace["phase"],
                }
            )
            p.x(0, value)
            u.reg_write(
                registers.UC_ARM64_REG_PC, u.reg_read(registers.UC_ARM64_REG_LR)
            )
        elif address in methods:
            name = methods[address]
            argument = p.x(0) if name == "drawing_handoff" else p.x(1)
            events.append(
                {
                    "kind": name,
                    "phase": namespace["phase"],
                    "argument": None if name == "rt_clear" else label(argument),
                    "queue_depth": len(p._task_returns),
                    "before": snapshot(),
                }
            )

    p.u.hook_add(namespace["UC_HOOK_CODE"], observe)
    initial = snapshot()
    assert initial["drawing"] is initial["gl"] is initial["rt"] is None
    rows = []
    drawing_method = "SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)"
    actions = [
        ("drawing_A", "drawing", "A"),
        ("drawing_null", "drawing", None),
        ("gl_null", "gl", None),
        ("rt_null", "rt", None),
        ("rt_clear", "clear", None),
        ("drawing_B", "drawing", "B"),
        ("drawing_no_texture", "drawing", "no_texture"),
        ("drawing_null_after_invalid", "drawing", None),
        ("gl_A", "gl", "A"),
        ("repeat_A", "drawing", "A"),
    ]
    factory_count = len(namespace["factory_created_pens"])
    for phase, target, name in actions:
        namespace["phase"] = phase
        start = len(events)
        bitmap = by_name[name] if name else 0
        if target == "drawing":
            p.call(
                "SPen::ObjectDrawing::SetDepthmap(SPen::ISPBitmap*)",
                [p.drawing, bitmap],
            )
        elif target == "gl":
            p.call(
                "SPen::ColoredPencilStrokeDrawableGLV4::SetDepthMapBitmap(SPen::ISPBitmap*)",
                [gl, bitmap],
            )
        elif target == "rt":
            p.call(
                "SPen::ColoredPencilStrokeDrawableRTV1::SetDepthMapBitmap(SPen::ISPBitmap*)",
                [rt, bitmap],
            )
        else:
            p.call("SPen::ColoredPencilStrokeDrawableRTV1::Clear()", [rt])
        after_setter = snapshot()
        if target == "drawing":
            p.call(
                drawing_method,
                [p.drawing, p.draw_canvas, p.pen_canvas, namespace["obj"], 1],
                limit=5_000_000,
            )
        else:
            namespace["call_address"](
                namespace["selected_object_redraw_target"],
                [gl + 8, namespace["obj"], namespace["rect"], namespace["rect"]],
            )
        assert len(namespace["factory_created_pens"]) == factory_count
        stream = namespace["captured_final_vectors"][phase]
        assert stream["sha256"] == original["stream_sha256"]
        rows.append(
            {
                "id": phase,
                "target": target,
                "argument": name,
                "after_setter": after_setter,
                "after_redraw": snapshot(),
                "geometry_sha256": stream["sha256"],
                "events": events[start:],
            }
        )
        print(phase, rows[-1]["after_redraw"], flush=True)
    receipt = {
        "schema": "sdocx-colored-pencil-v4-depth-lifecycle-native-v1",
        "capture": {
            "script": "tools/colored_pencil/lifecycle_capture.py",
            "script_sha256": sha(Path(__file__).read_bytes()),
            "geometry_acquisition_sha256": ACQUIRE_SHA,
            "geometry_receipt_sha256": sha(GEOMETRY.read_bytes()),
        },
        "scope": "Native ObjectDrawing::SetDepthmap, cached PenManager selection, DrawingUtil handoff, GLV4 depth validation and PenReturnCallback across repeated Drawing/selected redraw. The original geometry shell acknowledges the callback render task without executing it: RT depth remains null, and its null setter/Clear are only exercised directly. Caller bitmap objects are explicit host ISPBitmap adapters with width/height/texture-availability and reference-accounting leaves; texture tokens are opaque and never sampled. No native fields, methods, instructions or scheduler state are patched. Original geometry is required unchanged for every action. This establishes Drawing/GL/callback pointer state and host reference accounting, not queued GL-to-RT submission, bitmap pixels, full native Drawing paint, SDK deserialization or device lifecycle.",
        "libraries": capture["libraries"],
        "initial": initial,
        "cached_pen_reused": True,
        "host_bitmaps": [
            {k: v for k, v in row.items() if k != "texture"}
            | {"texture_available": bool(row["texture"])}
            for row in adapters.values()
        ],
        "cases": rows,
    }
    output = ROOT / "conformance/colored-pencil-v4-depth-lifecycle-native.json"
    temporary = output.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
    temporary.replace(output)
    print(output, output.stat().st_size)


if __name__ == "__main__":
    main()

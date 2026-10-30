"""Trace persisted native Fountain Drawing scale through real canvas methods."""

import hashlib
import json
from pathlib import Path
import struct
import sys

ROOT = Path(__file__).resolve().parents[2]
HELPER = (
    ROOT
    / "tmp/conformance-cleanup/native-capture-a241793/conformance/fountain_persisted_drawing_native.py"
)
HELPER_SHA = "9e0ef723a936b697c65b2ed0c476123287430fc2ff9d66b41945c2815ca5f59a"

SETUP = """ # Native setters establish the supplied canvas context before Drawing.
 _matrix=p.alloc(36);p.put(_matrix,'9f',*supplied_matrix)
 for _canvas in (p.draw_canvas,p.pen_canvas):
  call('SPen::SPPenCanvas::SetMatrix(SPen::Matrix3<float> const&)',[_canvas,_matrix])
 _matrix_queries_before=p.get_matrix_calls
"""

SNAPSHOT = """ _rt=p.get(drawable+16,'Q')[0]
 _native_matrix=p.alloc(36);p.u.reg_write(R.UC_ARM64_REG_X8,_native_matrix)
 call('SPen::SPPenCanvas::GetModelMatrix()',[p.pen_canvas])
 _matrix_bytes=bytes(p.u.mem_read(_native_matrix,36))
 if _matrix_bytes!=struct.pack('<9f',*supplied_matrix):raise RuntimeError('native canvas matrix roundtrip differs')
 SCALE_RESULTS[label]={'matrix':supplied_matrix,'matrix_f32_hex':_matrix_bytes.hex(),'gl_inverse_scale':list(p.get(drawable+336,'2f')),'rt_inverse_scale':list(p.get(_rt+264,'2f')),'repeat_distance':p.get(drawable+96,'f')[0],'repeat_f32_hex':bytes(p.u.mem_read(drawable+96,4)).hex(),'drawing_matrix_queries':p.get_matrix_calls-_matrix_queries_before-1,'native_state':_native_state,'point_count':model_count,'apply_consumed':_consumed,'native_pen_name':_native_names['SPen::ObjectStroke::GetPenName() const'],'native_setting':advanced_value,'source_arrays':model_arrays.copy(),'addpoint_arguments':[[struct.unpack('<f',bytes.fromhex(w))[0] for w in row['float_words_s0_s5']] for row in add_calls]}
 # Keep array pointers private; the original record below captures complete getter bytes.
"""


def sha(data):
    return hashlib.sha256(data).hexdigest()


def cases():
    return [
        ("identity", [1, 0, 0, 0, 1, 0, 0, 0, 1]),
        ("translation", [1, 0, 0, 0, 1, 0, 10, -20, 1]),
        ("uniform_half", [0.5, 0, 0, 0, 0.5, 0, 10, -20, 1]),
        ("uniform_4", [4, 0, 0, 0, 4, 0, 10, -20, 1]),
        ("top_1_crop", [1, 0, 0, 0, -1, 0, -889, 1467, 1]),
        ("top_2_crop", [2, 0, 0, 0, -2, 0, -1810, 2902, 1]),
        ("bottom_1_crop", [1, 0, 0, 0, 1, 0, -889, -1403, 1]),
        ("bottom_2_crop", [2, 0, 0, 0, 2, 0, -1810, -2838, 1]),
        ("rotation_2", [0, 2, 0, -2, 0, 0, 10, -20, 1]),
        ("anisotropic_2_1", [2, 0, 0, 0, 1, 0, 10, -20, 1]),
    ]


def main():
    source = HELPER.read_text()
    assert sha(source.encode()) == HELPER_SHA
    source = source[: source.index('records = namespace["CAPTURED_RECORDS"]')]
    anchor = "old_argv = sys.argv\n"
    assert source.count(anchor) == 1
    adaptation = """
_loop="for label,profile_width,advanced_value in [('saved_page_width',saved_page_width,'14;'),('host_width_control',1920,'14;')]:"
if adapted.count(_loop)!=1:raise RuntimeError('persisted loop anchor drift')
adapted=adapted.replace(_loop,"for label,supplied_matrix in SCALE_CASES:\\n profile_width=saved_page_width;advanced_value='14;'")
_draw=" rc=call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[drawing,p.draw_canvas,p.pen_canvas,obj,1])"
if adapted.count(_draw)!=1:raise RuntimeError('Drawing anchor drift')
adapted=adapted.replace(_draw,SETUP+_draw)
_record=" records.append({'case':label,"
if adapted.count(_record)!=1:raise RuntimeError('record anchor drift')
adapted=adapted.replace(_record,SNAPSHOT+_record)
"""
    source = source.replace(anchor, adaptation + anchor)
    source = source.replace(
        '"saved_page_width": saved_page_width}',
        '"saved_page_width": saved_page_width,"SCALE_CASES":SCALE_CASES,"SETUP":SETUP,"SNAPSHOT":SNAPSHOT,"SCALE_RESULTS":SCALE_RESULTS}',
    )
    scope = {
        "__name__": "fountain_drawing_scale_capture",
        "__file__": str(HELPER),
        "SCALE_CASES": cases(),
        "SETUP": SETUP,
        "SNAPSHOT": SNAPSHOT,
        "SCALE_RESULTS": {},
    }
    sys.argv = [str(HELPER)]
    exec(compile(source, str(HELPER), "exec"), scope)
    records = scope["namespace"]["CAPTURED_RECORDS"]
    reference_path = ROOT / "conformance/fountain-v14-real-source-rgba-native.json"
    reference = json.loads(reference_path.read_bytes())
    expected = b"".join(
        struct.pack("<6f", *row) for row in reference["addpoint_arguments"]
    )
    rows = []
    arrays = None
    for record in records:
        row = scope["SCALE_RESULTS"][record["case"]]
        row.pop("source_arrays")
        if arrays is None:
            arrays = record["source_arrays"]
        assert arrays == record["source_arrays"]
        arguments = b"".join(
            struct.pack("<6f", *values) for values in row.pop("addpoint_arguments")
        )
        row["canonical_arguments_match"] = arguments == expected
        assert (
            row["native_pen_name"]
            == "com.samsung.android.sdk.pen.pen.preload.FountainPen"
        )
        assert row["native_setting"] == "14;" and row["point_count"] == 161
        assert row["gl_inverse_scale"] == row["rt_inverse_scale"]
        assert row["drawing_matrix_queries"] >= 1
        rows.append(
            {
                "id": record["case"],
                **row,
                "addpoint_count": len(arguments) // 24,
                "addpoint_sha256": sha(arguments),
                "addpoint_arguments": [
                    list(v) for v in struct.iter_unpack("<6f", arguments)
                ],
            }
        )
    receipt = {
        "schema": "sdocx-fountain-v14-persisted-drawing-scale-native-v1",
        "capture": {
            "script": "tools/fountain_v14/drawing_scale_capture.py",
            "script_sha256": sha(Path(__file__).read_bytes()),
            "native_helper_sha256": HELPER_SHA,
            "paint_receipt_sha256": sha(reference_path.read_bytes()),
        },
        "runtime": {
            "python": scope["python_runtime"],
            "unicorn": scope["unicorn_runtime"],
            "tools": scope["executed_tools"],
        },
        "source_fixture_sha256": scope["SOURCE_FIXTURE_SHA256"],
        "native_sources": scope["base_assignments"]["LIBRARY_SHA256"],
        "source_arrays": arrays,
        "cases": rows,
        "scope": "Original persisted object payload and StringID table applied by native Model, OnAttach, native Drawing/GLV14 saved redraw, native Graphics SetMatrix/GetModelMatrix and GL-to-RT inverse-scale stores. Source getter bytes remain unchanged while ten matrices vary. Native repeat spacing, stamp arguments and canonical equality are observed rather than forced. Existing context, queue, loader and callback collection are explicit host boundaries. Stops at AddPoint callback arguments and RT scale fields; no GPU paint, full page loader or device assertion. Nonuniform/rotated matrix controls establish scale/geometry only and are not admitted paint transforms.",
    }
    output = ROOT / "conformance/fountain-v14-drawing-scale-native.json"
    output.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
    print(output, len(rows), output.stat().st_size)


if __name__ == "__main__":
    main()

"""Capture or check the nine synthetic Pencil2 viewport shader controls.

--check verifies retained evidence without loading native libraries. Capture
requires the pinned local native libraries and emulator used by the base runner.
"""

import argparse
import ctypes
import gzip
import hashlib
import json
import struct
import sys
import tempfile
import textwrap
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BASE = ROOT / "tools/pencil2_v14/context-base-capture.py.gz"
SOURCE = (
    ROOT
    / "tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/capture_candidate.py"
)
SOURCE_SHA = "34d5874cb86e27b619c01e9902b0aacf743619be560d8a4ca5ee65789ca8193d"
FIXTURE = ROOT / "conformance/pencil2-v14-viewport-shaders-native.json"
MODES = (
    "grain",
    "mask",
    "mask_coord",
    "mask_coord_fraction",
    "mask_lod",
    "mask_sample",
    "mask_texcoord_bits_x",
    "mask_texcoord_bits_y",
    "mask_texcoord_fraction",
)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def load_streams():
    colored = (ROOT / "conformance/pencil2-v14-color-80336699-native.bin").read_bytes()[
        :4704
    ]
    second = (ROOT / "conformance/pencil2-v14-color-ffdd4411-native.bin").read_bytes()
    black = (ROOT / "conformance/pencil2-v14-native.bin").read_bytes()
    sources = [
        colored,
        second,
        black[14112:18816],
        black[18816:23520],
        black[23520:28224],
    ]
    mixed = b"".join(sources[i % 5][i * 56 : (i + 1) * 56] for i in range(84))
    streams = {"mixed": mixed}
    for ordinal in range(1, 85):
        stream = bytearray(mixed)
        for i in range(84):
            if i != ordinal - 1:
                struct.pack_into("<f", stream, i * 56 + 16, 0.0)
        streams[f"record_{ordinal}"] = bytes(stream)
    return streams


INJECT = """    for name, stream in EDGE_STREAMS.items():
        p.u.mem_write(vector_begin,stream)
        p.call(RT_UPDATE,[rt,0],instruction_limit=5_000_000)
        p.u.mem_write(vector_begin,actual_geometry)
        for dy in (0,1):
            transform=[2,0,0,0, 0,-2,0,0, 0,0,1,0, 0,192,0,1]
            p.u.mem_write(matrix,struct.pack('<16f',*transform))
            p.call(ACTIVATE_SUBBITMAP_RT,[subbitmap,0],instruction_limit=5_000_000)
            f['glViewport'](0,0,width,height*(1+dy))
            CLIP_MODE[0]=dy
            f['glClearColor'](0.,0.,0.,0.); f['glClear'](0x00004000)
            f['glBindFramebuffer'](0x8d40,0)
            event_start=len(p.gl_events)
            p.call(RT_DRAW,[rt,subbitmap,matrix],instruction_limit=8_000_000)
            viewport=(C.c_int*4)()
            p._cfunc('glGetIntegerv',None,[C.c_uint,C.POINTER(C.c_int)])(0x0ba2,viewport)
            events=[e for e in p.gl_events[event_start:] if e['name']=='glViewport']
            matrices=[e for e in p.gl_events[event_start:] if e['name']=='glUniformMatrix4fv' and e['location']==0 and e['count']==1]
            assert len(matrices)==1, (name,dy,matrices)
            assert list(viewport)==[0,0,width,height*(1+dy)], (name,dy,list(viewport),events)
            f['glBindFramebuffer'](0x8d40,draw_target_fbo)
            f['glReadPixels'](0,0,width,height,0x1908,0x1401,pixels)
            if f['glGetError'](): raise RuntimeError('edge control GL error')
            frame=pixels.raw
            sparse=[[i//4%width,i//4//width,*frame[i:i+4]] for i in range(0,len(frame),4) if frame[i+3]]
            EDGE_RESULTS[name+':'+str(dy)]={'sha256':hashlib.sha256(frame).hexdigest(),'matrix':transform,'viewport_after_draw':list(viewport),'draw_viewport_events':events,'uploaded_projection':matrices[0]['values'],'pixels':sparse}
            print(name,dy,len(sparse),flush=True)
    CLIP_MODE[0]=0
    p.u.mem_write(vector_begin,actual_geometry)
    p.call(RT_UPDATE,[rt,0],instruction_limit=5_000_000)
"""

SHADER_TEMPLATE = """                marker = b"alpha = texture(sampler0, grainTexCoord).r * secAlpha;"
                if marker in raw:
                    assert raw.count(marker) == 1
                    original = raw
                    target = b"if (alpha == 0.0) discard;"
                    assert raw.count(target) == 3
                    start = raw.index(target)
                    raw = raw[:start] + b"if (secAlpha == 0.0) discard; FragColor = vec4(fragCoord.xy, 0.0, 1.0); }"
                    SHADER_RECEIPT.append({"mode": SHADER_MODE,
                        "original_sha256": hashlib.sha256(original).hexdigest(),
                        "uploaded_sha256": hashlib.sha256(raw).hexdigest(),
                        "original_length": len(original), "uploaded_length": len(raw),
                        "original_source": original.decode("utf-8"),
                        "uploaded_source": raw.decode("utf-8"),
                        "explicit_lengths": bool(lens), "needle_count": 1,
                        "replacement_count": 1})
                raws.append(raw); bufs.append(C.c_char_p(raw))"""

SAMPLE_EDIT = """(b"texture(sampler0, grainTexCoord).r" if SHADER_MODE == "grain"
                              else b"texture(sampler1, pointTexCoord).r")
                    replacement = b"0.5" if SHADER_MODE == "grain" else b"1.0"
                    assert raw.count(target) == 1
                    raw = raw.replace(target, replacement, 1)
                    assert target not in raw
"""

SHADER_NEEDLE = "                raws.append(raw); bufs.append(C.c_char_p(raw))"
LENGTH_NEEDLE = (
    'lengths=(C.c_int*count)(*(self.get(lens,"i"*count) if lens else [-1]*count))'
)
LENGTH_REPLACEMENT = "lengths=(C.c_int*count)(*(len(raw) for raw in raws))"
MATRIX_NEEDLE = (
    '            vals=self.get(ptr,"f"*(16*count)); arr=(C.c_float*len(vals))(*vals)'
)
MATRIX_REPLACEMENT = """            vals=self.get(ptr,"f"*(16*count))
            if CLIP_MODE[0] and loc == 0 and count == 1:
                vals=list(vals); vals[5]*=0.5; vals[13]=vals[13]*0.5-0.5
            arr=(C.c_float*len(vals))(*vals)"""
VIEWPORT_NEEDLE = """            values = [self.x(i) for i in range(argc)]
            if argtypes is None:"""
VIEWPORT_REPLACEMENT = """            values = [self.x(i) for i in range(argc)]
            if name == "glViewport" and CLIP_MODE[0]:
                assert values == [0,0,192,192], values
                values[3] *= 2
            if argtypes is None:"""
NEEDLE = '    graphics=p.get(rt+200,"Q")[0]'


def shader_patch(mode):
    if mode not in MODES:
        raise ValueError(f"unknown shader mode: {mode}")
    patch = SHADER_TEMPLATE
    if mode in ("grain", "mask"):
        start = patch.index("                    target = ")
        end = patch.index("                    SHADER_RECEIPT")
        return (
            patch[:start] + "                    target = " + SAMPLE_EDIT + patch[end:]
        )
    expressions = {
        "mask_coord": "vec4(fragCoord.xy, 0.0, 1.0)",
        "mask_coord_fraction": "vec4(fract(fragCoord.xy * 256.0), 0.0, 1.0)",
        "mask_texcoord_fraction": "vec4(fract(pointTexCoord * 256.0), 0.0, 1.0)",
        "mask_lod": "vec4(alpha, alpha, alpha, 1.0)",
        "mask_sample": "vec4(alpha, alpha, alpha, 1.0)",
    }
    if mode.startswith("mask_texcoord_bits_"):
        component = mode[-1]
        patch = patch.replace(
            "FragColor = vec4(fragCoord.xy, 0.0, 1.0);",
            f"highp uint bits = floatBitsToUint(pointTexCoord.{component}); "
            "FragColor = vec4(float(bits & 255u), float((bits >> 8u) & 255u), "
            "float((bits >> 16u) & 255u), 255.0) / 255.0;",
        )
    else:
        patch = patch.replace("vec4(fragCoord.xy, 0.0, 1.0)", expressions[mode])
    if mode == "mask_lod":
        patch = patch.replace(
            "                    target = ",
            '                    raw = raw.replace(b"texture(sampler1, pointTexCoord).r", '
            'b"textureLod(sampler1, pointTexCoord, 0.0).r")\n                    target = ',
        )
    return patch


def instrument_source(source, mode):
    if sha(source.encode()) != SOURCE_SHA:
        raise ValueError("base capture source hash mismatch")
    inject = INJECT
    if mode.startswith("mask_texcoord_bits_"):
        anchor = "            assert len(matrices)==1"
        extra = """            tile_location=p.uniform_locations.get('numTilesOfPoints')
            tile_uploads=[e for e in p.gl_events[event_start:] if e['name']=='glUniform1f' and e['location']==tile_location]
            if SHADER_MODE.endswith('_y'):
                assert tile_location is not None and tile_uploads, (name,dy,tile_location,tile_uploads)
"""
        inject = inject.replace(anchor, extra + anchor)
        inject = inject.replace(
            "'pixels':sparse",
            "'num_tiles_location':tile_location,'num_tiles_uploads':tile_uploads,'pixels':sparse",
        )
    for needle, replacement in (
        (SHADER_NEEDLE, shader_patch(mode)),
        (LENGTH_NEEDLE, LENGTH_REPLACEMENT),
        (MATRIX_NEEDLE, MATRIX_REPLACEMENT),
        (VIEWPORT_NEEDLE, VIEWPORT_REPLACEMENT),
        (NEEDLE, inject + NEEDLE),
    ):
        if source.count(needle) != 1:
            raise ValueError(f"expected one source anchor: {needle!r}")
        source = source.replace(needle, replacement)
    return source


def check_evidence():
    bundle = json.loads(FIXTURE.read_text())
    cases = bundle["cases"]
    streams = {name: sha(data) for name, data in load_streams().items()}
    source = gzip.decompress(BASE.read_bytes()).decode()
    for mode in MODES:
        receipt = cases[mode]
        assert streams == receipt["inputs"], mode
        shader = receipt["shader"]
        assert sha(shader["original_source"].encode()) == shader["original_sha256"], (
            mode
        )
        ns = {
            "raw": shader["original_source"].encode(),
            "lens": shader["explicit_lengths"],
            "SHADER_MODE": mode,
            "SHADER_RECEIPT": [],
            "raws": [],
            "bufs": [],
            "hashlib": hashlib,
            "C": ctypes,
        }
        exec(textwrap.dedent(shader_patch(mode)), ns)
        assert ns["SHADER_RECEIPT"] == [shader], mode
        assert ns["raws"] == [shader["uploaded_source"].encode()], mode
        program = instrument_source(source, mode)
        assert sha(program.encode()) == bundle["capture_program_sha256"][mode], mode
        compile(program, str(SOURCE), "exec")
        assert len(receipt["results"]) == 2 * len(streams), mode
    print(
        f"{len(MODES)} modes: retained input hashes, exact shader bytes, and source checks passed"
    )


def capture(mode):
    source = gzip.decompress(BASE.read_bytes()).decode()
    streams = load_streams()
    ns = {
        "__name__": "edge_interior_capture",
        "__file__": str(SOURCE),
        "EDGE_STREAMS": streams,
        "EDGE_RESULTS": {},
        "CLIP_MODE": [0],
        "SHADER_MODE": mode,
        "SHADER_RECEIPT": [],
    }
    exec(compile(instrument_source(source, mode), str(SOURCE), "exec"), ns)
    with tempfile.TemporaryDirectory(prefix="sdocx-pencil2-edge-") as directory:
        baseline = ns["capture"](Path(directory))
    assert len(ns["SHADER_RECEIPT"]) == 1, ns["SHADER_RECEIPT"]
    historical = json.loads(FIXTURE.read_text())["cases"][mode]
    baseline_key = (
        "baseline_sha256"
        if mode in ("grain", "mask")
        else "instrumented_baseline_sha256"
    )
    receipt = {
        "schema": historical["schema"],
        "frame": [192, 192],
        "coordinates": historical["coordinates"],
        "scope": historical["scope"],
        "capture": {
            "script": str(Path(__file__).resolve().relative_to(ROOT)),
            "script_sha256": sha(Path(__file__).read_bytes()),
            "base_source_sha256": SOURCE_SHA,
            baseline_key: baseline["mesa"]["readback_sha256"],
            "dso_pins": ns["DSO_PINS"],
        },
        "shader": ns["SHADER_RECEIPT"][0],
        "inputs": {name: sha(data) for name, data in streams.items()},
        "results": ns["EDGE_RESULTS"],
        "shader_mode": mode,
        "comparison": {},
    }
    for name in streams:
        left = {tuple(p[:2]): p[2:] for p in receipt["results"][name + ":0"]["pixels"]}
        right = {tuple(p[:2]): p[2:] for p in receipt["results"][name + ":1"]["pixels"]}
        differences = []
        for xy in sorted(left.keys() | right.keys()):
            a = left.get(xy, [0, 0, 0, 0])
            b = right.get(xy, [0, 0, 0, 0])
            if a != b:
                differences.append([*xy, a, b])
        if differences:
            receipt["comparison"][name] = differences
    for frame in receipt["results"].values():
        frame.pop("pixels")
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument(
        "--check",
        action="store_true",
        help="verify retained evidence without native execution",
    )
    action.add_argument(
        "--mode", choices=MODES, help="capture one native shader control"
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="new capture output; existing files are never overwritten",
    )
    args = parser.parse_args()
    if sys.flags.optimize:
        parser.error("run without Python optimization; capture requires assertions")
    if args.check:
        if args.output is not None:
            parser.error("--output requires --mode")
        check_evidence()
    else:
        if args.output is None:
            parser.error("--mode requires --output")
        if args.output.exists():
            parser.error(f"output already exists: {args.output}")
        receipt = capture(args.mode)
        with args.output.open("x") as output:
            output.write(json.dumps(receipt, separators=(",", ":")) + "\n")
        print(args.output)


if __name__ == "__main__":
    main()

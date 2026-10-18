"""Replay shipped Oblique RTV1 shaders over native triangle fixtures on host GLES.

This does not invoke Samsung's RendererFactory, RTV1::Draw, or an Android GPU.
"""
import argparse
import ctypes as C
import hashlib
import importlib.util
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LIBRARY = ROOT / 'scratch/apk-analysis-native/arm64-v8a/libSPenObliquePen.so'
REDRAW = ROOT / 'conformance/oblique-redraw.json'
CORE = ROOT / 'conformance/oblique-v4-core.json'
OUTPUT = ROOT / 'conformance/oblique-paint-v4.json'
LIBRARY_SHA256 = '9b76bdb709db9f56c5d35a1a615fe41c57ceef73c559c91efaf5bf436f90e0ee'
WIDTH, HEIGHT = 64, 64
COLOR = (0.16, 0.42, 0.78, 0.55)
BACKGROUND = (0.9, 0.8, 0.7, 1.0)
PROBES = ((16, 16), (24, 20), (32, 32), (40, 40), (48, 44))
SHADER_ADDRESSES = {
    'quad_vertex': 0x22ea4,
    'composite_fragment': 0x22f59,
    'mask_vertex': 0x2307c,
    'mask_fragment': 0x2316d,
    'stroke_vertex': 0x23247,
    'stroke_fragment': 0x23338,
}

_spec = importlib.util.spec_from_file_location('inkpen_paint_native', ROOT / 'conformance/inkpen_paint_native.py')
_inkpaint = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_inkpaint)
_inkpaint.WIDTH, _inkpaint.HEIGHT = WIDTH, HEIGHT


def _shader_source(library, address):
    return _inkpaint.read_va(library, address, 8192).split(b'\0', 1)[0].decode()


def _fixture_cases():
    redraw_bytes, core_bytes = REDRAW.read_bytes(), CORE.read_bytes()
    redraw, core = json.loads(redraw_bytes), json.loads(core_bytes)
    cases = []
    for case in redraw['cases']:
        native = case['native']
        cases.append({
            'id': f"saved:{case['id']}",
            'vertices': native['callback_triples'],
            'sample_ends_vector3': native['sample_ends_vector3'],
            'terminal_start_vector3': native['terminal_start_vector3'],
            'terminal_end_vector3': native['terminal_end_vector3'],
        })
    for case in core['cases']:
        cases.append({
            'id': f"mesh:{case['id']}",
            'vertices': case['native_triangle_vertices'],
            'sample_ends_vector3': None,
            'terminal_start_vector3': None,
            'terminal_end_vector3': None,
        })
    return cases, hashlib.sha256(redraw_bytes).hexdigest(), hashlib.sha256(core_bytes).hexdigest()


def _barycentric_check(vertices, rgba):
    """Compare interior pixel centers with max of affine vertex-z fields."""
    checked = mismatches = 0
    for py in range(HEIGHT):
        for px in range(WIDTH):
            sx, sy = px + 0.5, py + 0.5
            expected = 0.0
            has_interior = False
            near_edge = False
            for i in range(0, len(vertices), 3):
                a, b, c = vertices[i:i + 3]
                den = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1])
                if abs(den) < 1e-12:
                    continue
                u = ((b[1] - c[1]) * (sx - c[0]) + (c[0] - b[0]) * (sy - c[1])) / den
                v = ((c[1] - a[1]) * (sx - c[0]) + (a[0] - c[0]) * (sy - c[1])) / den
                w = 1.0 - u - v
                low, high = min(u, v, w), max(u, v, w)
                if low >= 0.0:
                    expected = max(expected, u * a[2] + v * b[2] + w * c[2])
                    has_interior |= low > 0.02
                if low >= -0.02 and high <= 1.02 and (low <= 0.02 or high >= 0.98):
                    near_edge = True
            # Avoid edge-rule disagreements, including edges of overlapping triangles.
            if has_interior and not near_edge:
                checked += 1
                observed = rgba[(py * WIDTH + px) * 4]
                if abs(observed - round(expected * 255)) > 1:
                    mismatches += 1
    return checked, mismatches


def _capture(library_path):
    library = library_path.read_bytes()
    library_sha = hashlib.sha256(library).hexdigest()
    if library_sha != LIBRARY_SHA256:
        raise ValueError(f'Oblique library SHA mismatch: {library_sha}')
    shaders = {key: _shader_source(library, address) for key, address in SHADER_ADDRESSES.items()}
    gles = _inkpaint.GLES()
    f = gles.f
    programs = {
        'mask': gles.program(shaders['mask_vertex'], shaders['mask_fragment']),
        'stroke': gles.program(shaders['stroke_vertex'], shaders['stroke_fragment']),
        'composite': gles.program(shaders['quad_vertex'], shaders['composite_fragment'], ((0, 'aVertex'),)),
    }
    projection = (C.c_float * 16)(
        2 / WIDTH, 0, 0, 0, 0, 2 / HEIGHT, 0, 0,
        0, 0, 1, 0, -1, -1, 0, 1,
    )
    cases, redraw_sha, core_sha = _fixture_cases()
    output_cases = []

    for case in cases:
        vertices = case['vertices']
        if not vertices or len(vertices) % 3:
            raise ValueError(f"{case['id']}: expected a nonempty Vector3 triangle list")
        vao = C.c_uint()
        f['glGenVertexArrays'](1, C.byref(vao))
        f['glBindVertexArray'](vao.value)
        values = [value for vertex in vertices for value in vertex]
        _inkpaint.GLES.buffer(gles, 0x8892, values)
        f['glEnableVertexAttribArray'](0)
        f['glVertexAttribPointer'](0, 3, 0x1406, 0, 0, None)

        _, mask_fbo = gles.target()
        f['glBindFramebuffer'](0x8D40, mask_fbo)
        f['glViewport'](0, 0, WIDTH, HEIGHT)
        for cap in (0x0B71, 0x0B44, 0x0BD0):
            f['glDisable'](cap)
        f['glClearColor'](0, 0, 0, 0)
        f['glClear'](0x4000)
        f['glUseProgram'](programs['mask'])
        f['glUniformMatrix4fv'](f['glGetUniformLocation'](programs['mask'], b'uProjectionMatrix'), 1, 0, projection)
        f['glUniform1i'](f['glGetUniformLocation'](programs['mask'], b'uIsInverseAlpha'), 0)
        f['glEnable'](0x0BE2)
        f['glBlendEquationSeparate'](0x8008, 0x8008)  # GL_MAX, recovered shared SDK enum mapping
        f['glBlendFuncSeparate'](1, 1, 1, 1)
        f['glDrawArrays'](0x0004, 0, len(vertices))
        mask = gles.read()
        checked, mismatches = _barycentric_check(vertices, mask)

        # Exercise the alternate colorized stroke shader with the same mesh and MAX state.
        _, stroke_fbo = gles.target()
        f['glBindFramebuffer'](0x8D40, stroke_fbo)
        f['glViewport'](0, 0, WIDTH, HEIGHT)
        f['glClearColor'](0, 0, 0, 0)
        f['glClear'](0x4000)
        f['glUseProgram'](programs['stroke'])
        f['glUniformMatrix4fv'](f['glGetUniformLocation'](programs['stroke'], b'uProjectionMatrix'), 1, 0, projection)
        f['glUniform4f'](f['glGetUniformLocation'](programs['stroke'], b'uInputColor'), *COLOR)
        f['glEnable'](0x0BE2)
        f['glBlendEquationSeparate'](0x8008, 0x8008)
        f['glBlendFuncSeparate'](1, 1, 1, 1)
        f['glDrawArrays'](0x0004, 0, len(vertices))
        stroke_pixels = gles.read()

        mask_texture, _ = gles.target()
        f['glBindTexture'](0x0DE1, mask_texture)
        raw_mask = C.create_string_buffer(mask)
        f['glTexImage2D'](0x0DE1, 0, 0x8058, WIDTH, HEIGHT, 0, 0x1908, 0x1401, C.cast(raw_mask, C.c_void_p))
        _, composite_fbo = gles.target()
        f['glBindFramebuffer'](0x8D40, composite_fbo)
        f['glViewport'](0, 0, WIDTH, HEIGHT)
        f['glDisable'](0x0BE2)
        f['glClearColor'](*BACKGROUND)
        f['glClear'](0x4000)
        _inkpaint.GLES.buffer(gles, 0x8892, [-1, -1, 1, -1, -1, 1, 1, 1])
        f['glEnableVertexAttribArray'](0)
        f['glVertexAttribPointer'](0, 2, 0x1406, 0, 0, None)
        f['glUseProgram'](programs['composite'])
        f['glActiveTexture'](0x84C0)
        f['glBindTexture'](0x0DE1, mask_texture)
        f['glUniform1i'](f['glGetUniformLocation'](programs['composite'], b'uSrcTexture'), 0)
        f['glUniform4f'](f['glGetUniformLocation'](programs['composite'], b'uInputColor'), *COLOR)
        f['glEnable'](0x0BE2)
        f['glBlendEquationSeparate'](0x8006, 0x8006)
        f['glBlendFuncSeparate'](1, 0x0303, 1, 0x0303)
        f['glDrawArrays'](0x0005, 0, 4)
        composite = gles.read()
        if f['glGetError']():
            raise RuntimeError(f"{case['id']}: GLES error")

        output_cases.append({
            'id': case['id'],
            'vertex_count': len(vertices),
            'vertex_f32_sha256': hashlib.sha256(b''.join(struct.pack('<f', value) for row in vertices for value in row)).hexdigest(),
            'sample_ends_vector3': case['sample_ends_vector3'],
            'terminal_start_vector3': case['terminal_start_vector3'],
            'terminal_end_vector3': case['terminal_end_vector3'],
            'mask_rgba8_sha256': hashlib.sha256(mask).hexdigest(),
            'mask_red_nonzero_pixels': sum(mask[i] != 0 for i in range(0, len(mask), 4)),
            'mask_red_max': max(mask[0::4]),
            'stroke_max_rgba8_sha256': hashlib.sha256(stroke_pixels).hexdigest(),
            'composite_rgba8_sha256': hashlib.sha256(composite).hexdigest(),
            'composite_samples': [list(composite[(y * WIDTH + x) * 4:(y * WIDTH + x) * 4 + 4]) for x, y in PROBES],
            'barycentric_interior_pixels': checked,
            'barycentric_mismatches_over_1_of_255': mismatches,
        })

    return {
        'schema': 'sdocx-oblique-paint-v1',
        'provenance': 'Actual embedded Oblique RTV1 MaskShader, StrokeShader, and CompositeShader from the hash-pinned Samsung library; captured native callback meshes and direct lineToMesh/makeTriangles vertices; replayed on host surfaceless Mesa GLES.',
        'library_sha256': library_sha,
        'input_sha256': {'oblique_redraw': redraw_sha, 'oblique_v4_core': core_sha},
        'shader_virtual_addresses': {key: f'{address:#x}' for key, address in SHADER_ADDRESSES.items()},
        'shader_sha256': {key: hashlib.sha256(value.encode()).hexdigest() for key, value in shaders.items()},
        'gles_host': gles.identity,
        'host_render_profile': {
            'viewport': [WIDTH, HEIGHT],
            'projection': 'synthetic pixel-coordinate map x/y -> [-1,1]; preserves fixture geometry values',
            'color_rgba': list(COLOR),
            'background_rgba': list(BACKGROUND),
            'mask': 'embedded MaskShader; captured vec3 triangles; GL_MAX, ONE/ONE; transparent RGBA8 target',
            'stroke': 'embedded StrokeShader; captured vec3 triangles; GL_MAX, ONE/ONE; transparent RGBA8 target',
            'composite': 'embedded CompositeShader; red mask sampled once; host color/background; FUNC_ADD, ONE, ONE_MINUS_SRC_ALPHA',
            'limits': [
                'does not call Samsung RendererFactory, native RTV1::Draw, host callbacks, or Android GL',
                'projection, color, and background are explicit synthetic host settings',
                'pixel hashes depend on Mesa/GLES version and are not Samsung device pixel evidence',
                'interpolation checks exclude triangle-edge neighborhoods and compare only CPU barycentric z against mask red',
                'does not establish vector export parity or page-level compositing',
            ],
        },
        'cases': output_cases,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--library', type=Path, default=LIBRARY)
    parser.add_argument('--fixture', type=Path, default=OUTPUT)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--write-fixture', action='store_true')
    mode.add_argument('--check-fixture', action='store_true')
    args = parser.parse_args()
    result = _capture(args.library)
    if args.write_fixture:
        args.fixture.write_text(json.dumps(result, indent=2) + '\n')
    else:
        expected = json.loads(args.fixture.read_text())
        if result != expected:
            raise SystemExit('Oblique paint fixture mismatch; inspect library/input hashes and host GLES identity before updating')
    print(f"GLES {result['gles_host']}")
    print(f"cases={len(result['cases'])}; barycentric mismatches={sum(c['barycentric_mismatches_over_1_of_255'] for c in result['cases'])}")


if __name__ == '__main__':
    main()

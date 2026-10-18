#!/usr/bin/env python3
"""Run embedded Pencil3 V1 GLSL with native masks/particle fixtures on host EGL.

From the repository root:
  python3 conformance/pencil_paint_native.py --write-fixture
  python3 conformance/pencil_paint_native.py

This executes the vendor shader on host GLES. Projection, paper scale and
input color are synthetic; resulting pixels are not an Android/device oracle.
"""
import argparse
import hashlib
import json
import struct
import sys
from pathlib import Path
import ctypes as C

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'conformance'))
from inkpen_paint_native import GLES, cfunc, read_va

LIB = ROOT / 'scratch/apk-analysis-native/arm64-v8a/libSPenPencil3.so'
NATIVE_FIXTURE = ROOT / 'conformance/pencil-v1-native.json'
FIXTURE = Path(__file__).resolve().parent / 'pencil-paint-v1.json'
W = H = 128
VERTEX = 0xf9730
FRAGMENT = 0xf9948
QUAD = 0xf9690


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read_pixels(gles):
    out = C.create_string_buffer(W * H * 4)
    gles.f['glPixelStorei'](0x0D05, 1)
    gles.f['glReadPixels'](0, 0, W, H, 0x1908, 0x1401, out)
    return out.raw


def upload_texture(gles, unit, width, height, rgba, min_filter, mag_filter, wrap_s, wrap_t):
    f = gles.f
    handle = C.c_uint()
    f['glGenTextures'](1, C.byref(handle))
    f['glActiveTexture'](0x84C0 + unit)
    f['glBindTexture'](0x0DE1, handle.value)
    for pname, value in ((0x2801, min_filter), (0x2800, mag_filter),
                         (0x2802, wrap_s), (0x2803, wrap_t)):
        f['glTexParameteri'](0x0DE1, pname, value)
    buf = C.create_string_buffer(rgba)
    f['glPixelStorei'](0x0CF5, 1)
    f['glTexImage2D'](0x0DE1, 0, 0x8058, width, height, 0, 0x1908, 0x1401,
                      C.cast(buf, C.c_void_p))
    return handle.value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--library', type=Path, default=LIB)
    parser.add_argument('--input-fixture', type=Path, default=NATIVE_FIXTURE)
    parser.add_argument('--fixture', type=Path, default=FIXTURE)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--write-fixture', action='store_true')
    mode.add_argument('--check-fixture', action='store_true')
    args = parser.parse_args()
    raw = args.library.read_bytes()
    if sha(raw) != '41a995cac7a597af53359401df26187d4299221f3460c91b56540596fc73d359':
        raise ValueError('Pencil3 library does not match pinned Samsung Notes APK')
    atlas_u32 = read_va(raw, 0x12c8c, 16 * 120 * 120 * 4)
    paper_u8 = read_va(raw, 0xf3c8c, 150 * 150)
    # Native setTextures stores every mask/gray sample as an R byte followed
    # by zero G, B, and A bytes in the uploaded RGBA8 bitmap.
    atlas_rgba = b''.join(bytes((v, 0, 0, 0)) for v in atlas_u32[::4])
    paper_rgba = b''.join(bytes((v, 0, 0, 0)) for v in paper_u8)
    native_raw = args.input_fixture.read_bytes()
    if sha(native_raw) != '530b72d6891d430016d99948164ca4d85dd07f90de13b178be262c7da68d5597':
        raise ValueError('native Pencil3 particle fixture does not match the reviewed input')
    native = json.loads(native_raw)
    g = GLES()
    f = g.f
    gles_context = g.identity

    program = g.program(read_va(raw, VERTEX, 8192).split(b'\0', 1)[0].decode(),
                        read_va(raw, FRAGMENT, 8192).split(b'\0', 1)[0].decode())
    f['glUseProgram'](program)
    brush = upload_texture(g, 0, 120, 1920, atlas_rgba,
                           0x2600, 0x2600, 0x812f, 0x812f)
    paper = upload_texture(g, 1, 150, 150, paper_rgba,
                           0x2600, 0x2600, 0x2901, 0x2901)
    f['glUniform1i'](f['glGetUniformLocation'](program, b'pointPattern'), 0)
    f['glUniform1i'](f['glGetUniformLocation'](program, b'paperTexture'), 1)
    f['glUniform4f'](f['glGetUniformLocation'](program, b'inputColor'), 0.18, 0.32, 0.72, 0.8)
    f['glUniform2f'] = cfunc(g.gl, 'glUniform2f', None, [C.c_int, C.c_float, C.c_float])
    f['glUniform2f'](f['glGetUniformLocation'](program, b'uPaperScale'), 1.0, 1.0)
    # Synthetic 128x128 canvas with one pixel per coordinate unit and origin at center.
    # This is a host sampling profile, not the app's native document matrix.
    projection = (C.c_float * 16)(1/64, 0, 0, 0, 0, 1/64, 0, 0,
                                  0, 0, 1, 0, 0, 0, 0, 1)
    f['glUniformMatrix4fv'](f['glGetUniformLocation'](program, b'uProjectionMatrix'),
                            1, 0, projection)
    f['glEnable'](0x0BE2)
    f['glBlendEquationSeparate'](0x8006, 0x8006)
    f['glBlendFuncSeparate'](0x0302, 0x0303, 0x0001, 0x0303)

    quad = [struct.unpack('<4f', read_va(raw, QUAD + i * 16, 16)) for i in range(4)]
    vao = C.c_uint()
    f['glGenVertexArrays'](1, C.byref(vao))
    f['glBindVertexArray'](vao.value)
    rect_vbo = g.buffer(0x8892, [v for q in quad for v in q])
    f['glEnableVertexAttribArray'](0)
    f['glVertexAttribPointer'](0, 4, 0x1406, 0, 0, None)
    f['glVertexAttribDivisor'](0, 0)
    out_tex, out_fbo = C.c_uint(), C.c_uint()
    f['glGenTextures'](1, C.byref(out_tex))
    f['glBindTexture'](0x0DE1, out_tex.value)
    for pname, value in ((0x2801, 0x2600), (0x2800, 0x2600),
                         (0x2802, 0x812f), (0x2803, 0x812f)):
        f['glTexParameteri'](0x0DE1, pname, value)
    f['glTexImage2D'](0x0DE1, 0, 0x8058, W, H, 0, 0x1908, 0x1401, None)
    f['glGenFramebuffers'](1, C.byref(out_fbo))
    f['glBindFramebuffer'](0x8D40, out_fbo.value)
    f['glFramebufferTexture2D'](0x8D40, 0x8CE0, 0x0DE1, out_tex.value, 0)
    status = f['glCheckFramebufferStatus'](0x8D40)
    if status != 0x8CD5:
        raise RuntimeError(f'Pencil3 output framebuffer incomplete: {status:#x}')
    f['glBindFramebuffer'](0x8D40, out_fbo)
    f['glViewport'](0, 0, W, H)
    # Rebind the two source textures after target() changed active unit/bind.
    f['glActiveTexture'](0x84C0)
    f['glBindTexture'](0x0DE1, brush)
    f['glActiveTexture'](0x84C1)
    f['glBindTexture'](0x0DE1, paper)

    case_rows = []
    cases = native['saved_native_outputs']
    for case in cases:
        attrs = case['packed_attributes']
        if len(attrs) % 5:
            raise ValueError(f"{case['name']}: malformed native packed vector")
        f['glClearColor'](0, 0, 0, 0)
        f['glClear'](0x4000)
        if attrs:
            vbo = g.buffer(0x8892, attrs)
            f['glEnableVertexAttribArray'](1)
            f['glVertexAttribPointer'](1, 4, 0x1406, 0, 20, None)
            f['glVertexAttribDivisor'](1, 1)
            f['glEnableVertexAttribArray'](2)
            f['glVertexAttribPointer'](2, 1, 0x1406, 0, 20, C.c_void_p(16))
            f['glVertexAttribDivisor'](2, 1)
            f['glDrawArraysInstanced'](0x0005, 0, 4, len(attrs) // 5)
            pixels = read_pixels(g)
        else:
            pixels = read_pixels(g)
        error = f['glGetError']()
        if error:
            raise RuntimeError(f"{case['name']}: GLES error {error:#x}")
        alpha_pixels = [i for i in range(3, len(pixels), 4) if pixels[i]]
        if case['particle_count'] and not alpha_pixels:
            raise AssertionError(f"{case['name']}: particles produced no host coverage")
        if not case['particle_count'] and any(pixels):
            raise AssertionError(f"{case['name']}: empty native buffer produced host pixels")
        spot = None
        if alpha_pixels:
            alpha_offset = alpha_pixels[len(alpha_pixels) // 2]
            pixel_offset = alpha_offset - 3
            rgba = list(pixels[pixel_offset:pixel_offset + 4])
            expected_rgb = [round(channel * rgba[3]) for channel in (0.18, 0.32, 0.72)]
            errors = [abs(rgba[i] - expected_rgb[i]) for i in range(3)]
            if max(errors) > 2:
                raise AssertionError(f"{case['name']}: source-over spot check failed: {rgba} vs {expected_rgb}")
            spot = {
                'pixel_xy': [(pixel_offset // 4) % W, (pixel_offset // 4) // W],
                'rgba8': rgba, 'expected_rgb8_from_alpha': expected_rgb,
                'max_rgb_error': max(errors),
            }
        case_rows.append({
            'name': case['name'], 'native_particle_count': case['particle_count'],
            'native_attributes_sha256': sha(struct.pack('<' + 'f' * len(attrs), *attrs)) if attrs else sha(b''),
            'host_rgba8_sha256': g.image_hash(pixels),
            'host_nonzero_alpha_pixels': sum(pixels[i] != 0 for i in range(3, len(pixels), 4)),
            'analytic_source_over_spotcheck': spot,
        })
    fixture = {
        'version': 1,
        'provenance': {
            'apk_sha256': 'daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667',
            'library_sha256': sha(raw),
            'input_fixture_sha256': sha(native_raw),
            'shader_addresses': [f'{VERTEX:#x}', f'{FRAGMENT:#x}'],
            'quad_address': f'{QUAD:#x}',
            'capture_boundary': 'Embedded shader strings execute unmodified on host surfaceless EGL/GLES with DSO-extracted masks/quad and native saved post-AddPoint vectors. No Android driver or native Draw call executes.',
        },
        'library_sha256': sha(raw), 'atlas_sha256': sha(atlas_rgba), 'paper_sha256': sha(paper_rgba),
        'shader_sha256': [sha(read_va(raw, a, 8192).split(b'\0', 1)[0]) for a in (VERTEX, FRAGMENT)],
        'quad': [list(q) for q in quad], 'texture_parameters': {
            'brush': {'min_filter': 'NEAREST', 'mag_filter': 'NEAREST', 'wrap_s': 'CLAMP_TO_EDGE', 'wrap_t': 'CLAMP_TO_EDGE'},
            'paper': {'min_filter': 'NEAREST', 'mag_filter': 'NEAREST', 'wrap_s': 'REPEAT', 'wrap_t': 'REPEAT'},
        },
        'host': gles_context, 'host_profile': {
            'target': [W, H], 'projection_matrix_column_major': [1/64, 0, 0, 0, 0, 1/64, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
            'projection': 'synthetic centered pixel canvas, one pixel per coordinate unit',
            'paper_scale': [1, 1], 'input_color': [0.18, 0.32, 0.72, 0.8],
            'blend': ['FUNC_ADD', 'SRC_ALPHA', 'ONE_MINUS_SRC_ALPHA', 'ONE', 'ONE_MINUS_SRC_ALPHA'],
            'spotcheck': 'For a nonzero-alpha pixel over transparent black, shader RGB is color and blend RGB is color times source alpha; output alpha uses ONE/ONE_MINUS_SRC_ALPHA. RGB is checked against color times readback alpha within two 8-bit values to allow separate channel quantization.',
            'limitations': ['not Samsung-device GLES', 'not native Draw or app projection/target lifecycle', 'host pixels are not device parity', 'host sampling only uses recovered native mask bytes and shader', 'saved particle positions are native fixture inputs'],
        }, 'cases': case_rows,
    }
    encoded = json.dumps(fixture, indent=2, sort_keys=True) + '\n'
    if args.write_fixture:
        args.fixture.write_text(encoded)
    elif not args.fixture.exists() or json.loads(args.fixture.read_text()) != fixture:
        raise SystemExit('Pencil3 host paint fixture mismatch; inspect GLES identity and output pixels')
    print('host GLES:', json.dumps(gles_context))
    for row in case_rows:
        print(row['name'], row['native_particle_count'], row['host_rgba8_sha256'], row['host_nonzero_alpha_pixels'])


if __name__ == '__main__':
    main()

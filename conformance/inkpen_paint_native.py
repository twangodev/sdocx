"""Replay embedded InkPen RTV1 shaders on the local GLES implementation.

Shader text and base quad are read from the hash-pinned InkPen ELF. The
per-point instances come from inkpen-v4.json's native post-AddPoint vector.
This executes vendor GLSL under host EGL/GLES, not Samsung's native Draw or
an Android/device GPU context.
"""
import argparse
import ctypes as C
import hashlib
import json
import struct
from pathlib import Path

INKPEN_SHA256 = '939a7c719838306d693f96701994486b03f5c7eea35f16f25c6a89d84daba401'
INKPEN_PATH = 'scratch/apk-analysis-native/arm64-v8a/libSPenInkPen.so'
INPUT_FIXTURE = 'conformance/inkpen-v4.json'
FIXTURE = 'conformance/inkpen-paint-v4.json'
SHADERS = {
    'alpha': (0x16382, 0x164d3),
    'composite': (0x16651, 0x166eb),
    'circle': (0x167f2, 0x16943),
}
QUAD_VA = 0x16298
WIDTH, HEIGHT = 128, 32
COLOR = (0.12, 0.34, 0.82, 0.6)
BACKGROUND = (0.92, 0.86, 0.74, 1.0)


def read_va(data, address, size):
    phoff = struct.unpack_from('<Q', data, 32)[0]
    entsize, count = struct.unpack_from('<HH', data, 54)
    for i in range(count):
        p_type, _, offset, vaddr, _, filesz, _, _ = struct.unpack_from('<II6Q', data, phoff + i * entsize)
        if p_type == 1 and vaddr <= address and address + size <= vaddr + filesz:
            start = offset + address - vaddr
            return data[start:start + size]
    raise ValueError(f'ELF virtual range {address:#x}+{size:#x} is not file-backed')


def cfunc(lib, name, restype, argtypes):
    fn = getattr(lib, name)
    fn.restype, fn.argtypes = restype, argtypes
    return fn


class GLES:
    def __init__(self):
        self.egl = C.CDLL('libEGL.so.1')
        self.gl = C.CDLL('libGLESv2.so.2')
        self._init_egl()
        self._bind_gl()

    def _init_egl(self):
        e = self.egl
        get_proc = cfunc(e, 'eglGetProcAddress', C.c_void_p, [C.c_char_p])
        addr = get_proc(b'eglGetPlatformDisplayEXT')
        if not addr:
            raise RuntimeError('eglGetPlatformDisplayEXT is unavailable')
        get_platform = C.CFUNCTYPE(C.c_void_p, C.c_uint, C.c_void_p, C.POINTER(C.c_int))(addr)
        self.display = get_platform(0x31DD, None, None)  # EGL_PLATFORM_SURFACELESS_MESA
        if not self.display:
            raise RuntimeError('surfaceless EGL display unavailable')
        initialize = cfunc(e, 'eglInitialize', C.c_uint,
                           [C.c_void_p, C.POINTER(C.c_int), C.POINTER(C.c_int)])
        major, minor = C.c_int(), C.c_int()
        if not initialize(self.display, C.byref(major), C.byref(minor)):
            raise RuntimeError(f'eglInitialize failed: {e.eglGetError():#x}')
        bind_api = cfunc(e, 'eglBindAPI', C.c_uint, [C.c_uint])
        if not bind_api(0x30A0):  # EGL_OPENGL_ES_API
            raise RuntimeError(f'eglBindAPI failed: {e.eglGetError():#x}')
        attrs = (C.c_int * 13)(0x3033, 1, 0x3040, 0x40, 0x3024, 8, 0x3023, 8,
                               0x3022, 8, 0x3021, 8, 0x3038)
        config, n = C.c_void_p(), C.c_int()
        choose = cfunc(e, 'eglChooseConfig', C.c_uint,
                       [C.c_void_p, C.POINTER(C.c_int), C.POINTER(C.c_void_p), C.c_int, C.POINTER(C.c_int)])
        if not choose(self.display, attrs, C.byref(config), 1, C.byref(n)) or not n.value:
            raise RuntimeError(f'eglChooseConfig failed: {e.eglGetError():#x}')
        pbuffer_attrs = (C.c_int * 5)(0x3057, WIDTH, 0x3056, HEIGHT, 0x3038)
        create_surface = cfunc(e, 'eglCreatePbufferSurface', C.c_void_p,
                               [C.c_void_p, C.c_void_p, C.POINTER(C.c_int)])
        self.surface = create_surface(self.display, config, pbuffer_attrs)
        context_attrs = (C.c_int * 3)(0x3098, 3, 0x3038)
        create_context = cfunc(e, 'eglCreateContext', C.c_void_p,
                               [C.c_void_p, C.c_void_p, C.c_void_p, C.POINTER(C.c_int)])
        self.context = create_context(self.display, config, None, context_attrs)
        make_current = cfunc(e, 'eglMakeCurrent', C.c_uint,
                             [C.c_void_p, C.c_void_p, C.c_void_p, C.c_void_p])
        if not self.surface or not self.context or not make_current(self.display, self.surface, self.surface, self.context):
            raise RuntimeError(f'EGL surface/context setup failed: {e.eglGetError():#x}')
        self.gl_string = cfunc(self.gl, 'glGetString', C.c_char_p, [C.c_uint])
        self.identity = {
            'egl_version': f'{major.value}.{minor.value}',
            'vendor': self.gl_string(0x1F00).decode(),
            'renderer': self.gl_string(0x1F01).decode(),
            'version': self.gl_string(0x1F02).decode(),
            'shading_language': self.gl_string(0x8B8C).decode(),
        }

    def _bind_gl(self):
        g = self.gl
        specs = {
            'glCreateShader': (C.c_uint, [C.c_uint]),
            'glShaderSource': (None, [C.c_uint, C.c_int, C.POINTER(C.c_char_p), C.POINTER(C.c_int)]),
            'glCompileShader': (None, [C.c_uint]),
            'glGetShaderiv': (None, [C.c_uint, C.c_uint, C.POINTER(C.c_int)]),
            'glGetShaderInfoLog': (None, [C.c_uint, C.c_int, C.POINTER(C.c_int), C.c_char_p]),
            'glCreateProgram': (C.c_uint, []),
            'glAttachShader': (None, [C.c_uint, C.c_uint]),
            'glBindAttribLocation': (None, [C.c_uint, C.c_uint, C.c_char_p]),
            'glLinkProgram': (None, [C.c_uint]),
            'glGetProgramiv': (None, [C.c_uint, C.c_uint, C.POINTER(C.c_int)]),
            'glGetProgramInfoLog': (None, [C.c_uint, C.c_int, C.POINTER(C.c_int), C.c_char_p]),
            'glUseProgram': (None, [C.c_uint]),
            'glGetUniformLocation': (C.c_int, [C.c_uint, C.c_char_p]),
            'glUniformMatrix4fv': (None, [C.c_int, C.c_int, C.c_ubyte, C.POINTER(C.c_float)]),
            'glUniform4f': (None, [C.c_int, C.c_float, C.c_float, C.c_float, C.c_float]),
            'glUniform1i': (None, [C.c_int, C.c_int]),
            'glGenTextures': (None, [C.c_int, C.POINTER(C.c_uint)]),
            'glBindTexture': (None, [C.c_uint, C.c_uint]),
            'glTexParameteri': (None, [C.c_uint, C.c_uint, C.c_int]),
            'glTexImage2D': (None, [C.c_uint, C.c_int, C.c_int, C.c_int, C.c_int, C.c_int,
                                    C.c_uint, C.c_uint, C.c_void_p]),
            'glActiveTexture': (None, [C.c_uint]),
            'glGenFramebuffers': (None, [C.c_int, C.POINTER(C.c_uint)]),
            'glBindFramebuffer': (None, [C.c_uint, C.c_uint]),
            'glFramebufferTexture2D': (None, [C.c_uint, C.c_uint, C.c_uint, C.c_uint, C.c_int]),
            'glCheckFramebufferStatus': (C.c_uint, [C.c_uint]),
            'glGenVertexArrays': (None, [C.c_int, C.POINTER(C.c_uint)]),
            'glBindVertexArray': (None, [C.c_uint]),
            'glGenBuffers': (None, [C.c_int, C.POINTER(C.c_uint)]),
            'glBindBuffer': (None, [C.c_uint, C.c_uint]),
            'glBufferData': (None, [C.c_uint, C.c_ssize_t, C.c_void_p, C.c_uint]),
            'glEnableVertexAttribArray': (None, [C.c_uint]),
            'glVertexAttribPointer': (None, [C.c_uint, C.c_int, C.c_uint, C.c_ubyte, C.c_int, C.c_void_p]),
            'glVertexAttribDivisor': (None, [C.c_uint, C.c_uint]),
            'glDrawArrays': (None, [C.c_uint, C.c_int, C.c_int]),
            'glDrawArraysInstanced': (None, [C.c_uint, C.c_int, C.c_int, C.c_int]),
            'glViewport': (None, [C.c_int, C.c_int, C.c_int, C.c_int]),
            'glDisable': (None, [C.c_uint]),
            'glEnable': (None, [C.c_uint]),
            'glBlendEquationSeparate': (None, [C.c_uint, C.c_uint]),
            'glBlendFuncSeparate': (None, [C.c_uint, C.c_uint, C.c_uint, C.c_uint]),
            'glClearColor': (None, [C.c_float, C.c_float, C.c_float, C.c_float]),
            'glClear': (None, [C.c_uint]),
            'glPixelStorei': (None, [C.c_uint, C.c_int]),
            'glReadPixels': (None, [C.c_int, C.c_int, C.c_int, C.c_int, C.c_uint, C.c_uint, C.c_void_p]),
            'glGetError': (C.c_uint, []),
        }
        self.f = {name: cfunc(g, name, ret, args) for name, (ret, args) in specs.items()}

    def shader(self, kind, source):
        g, f = self.gl, self.f
        shader = f['glCreateShader'](kind)
        src = C.c_char_p(source.encode())
        f['glShaderSource'](shader, 1, C.byref(src), None)
        f['glCompileShader'](shader)
        status = C.c_int()
        f['glGetShaderiv'](shader, 0x8B81, C.byref(status))
        if not status.value:
            log = C.create_string_buffer(4096)
            f['glGetShaderInfoLog'](shader, len(log), None, log)
            raise RuntimeError('native GLSL compile failed: ' + log.value.decode(errors='replace'))
        return shader

    def program(self, vertex, fragment, attributes=()):
        f = self.f
        vs = self.shader(0x8B31, vertex)
        fs = self.shader(0x8B30, fragment)
        prog = f['glCreateProgram']()
        f['glAttachShader'](prog, vs)
        f['glAttachShader'](prog, fs)
        for index, name in attributes:
            f['glBindAttribLocation'](prog, index, name.encode())
        f['glLinkProgram'](prog)
        status = C.c_int()
        f['glGetProgramiv'](prog, 0x8B82, C.byref(status))
        if not status.value:
            log = C.create_string_buffer(4096)
            f['glGetProgramInfoLog'](prog, len(log), None, log)
            raise RuntimeError('native GLSL link failed: ' + log.value.decode(errors='replace'))
        return prog

    def target(self):
        f = self.f
        texture, framebuffer = C.c_uint(), C.c_uint()
        f['glGenTextures'](1, C.byref(texture))
        f['glBindTexture'](0x0DE1, texture.value)
        f['glTexParameteri'](0x0DE1, 0x2801, 0x2600)
        f['glTexParameteri'](0x0DE1, 0x2800, 0x2600)
        f['glTexParameteri'](0x0DE1, 0x2802, 0x812F)
        f['glTexParameteri'](0x0DE1, 0x2803, 0x812F)
        f['glTexImage2D'](0x0DE1, 0, 0x8058, WIDTH, HEIGHT, 0, 0x1908, 0x1401, None)
        f['glGenFramebuffers'](1, C.byref(framebuffer))
        f['glBindFramebuffer'](0x8D40, framebuffer.value)
        f['glFramebufferTexture2D'](0x8D40, 0x8CE0, 0x0DE1, texture.value, 0)
        status = f['glCheckFramebufferStatus'](0x8D40)
        if status != 0x8CD5:
            raise RuntimeError(f'RGBA8 framebuffer incomplete: {status:#x}')
        return texture.value, framebuffer.value

    def buffer(self, target, values):
        packed = (C.c_float * len(values))(*values)
        handle = C.c_uint()
        self.f['glGenBuffers'](1, C.byref(handle))
        self.f['glBindBuffer'](target, handle.value)
        self.f['glBufferData'](target, C.sizeof(packed), C.cast(packed, C.c_void_p), 0x88E4)
        return handle.value

    def read(self):
        out = C.create_string_buffer(WIDTH * HEIGHT * 4)
        self.f['glPixelStorei'](0x0D05, 1)
        self.f['glReadPixels'](0, 0, WIDTH, HEIGHT, 0x1908, 0x1401, out)
        return out.raw

    def image_hash(self, pixels):
        return hashlib.sha256(pixels).hexdigest()


def draw_records(gles, source_triples, shaders):
    f = gles.f
    gles.f['glViewport'](0, 0, WIDTH, HEIGHT)
    for cap in (0x0B71, 0x0B44, 0x0BE2, 0x0BD0):
        f['glDisable'](cap)
    gles.f['glClearColor'](0, 0, 0, 0)
    gles.f['glClear'](0x4000)
    vao = C.c_uint()
    f['glGenVertexArrays'](1, C.byref(vao))
    f['glBindVertexArray'](vao.value)
    vertices = [tuple(struct.unpack('<4f', read_va(INKPEN_BYTES, QUAD_VA + i * 16, 16))) for i in range(4)]
    rects = [v for row in vertices for v in row]
    rect_vbo = gles.buffer(0x8892, rects)
    f['glEnableVertexAttribArray'](0)
    f['glVertexAttribPointer'](0, 4, 0x1406, 0, 0, None)
    instances = [v for triple in source_triples for v in triple]
    info_vbo = gles.buffer(0x8892, instances)
    f['glEnableVertexAttribArray'](1)
    f['glVertexAttribPointer'](1, 3, 0x1406, 0, 0, None)
    f['glVertexAttribDivisor'](1, 1)
    alpha_prog = gles.program(shaders['alpha'][0], shaders['alpha'][1])
    f['glUseProgram'](alpha_prog)
    projection = (C.c_float * 16)(2.0 / WIDTH, 0, 0, 0, 0, 2.0 / HEIGHT, 0, 0,
                                  0, 0, 1, 0, -0.5, 0, 0, 1)
    loc = f['glGetUniformLocation'](alpha_prog, b'uProjectionMatrix')
    f['glUniformMatrix4fv'](loc, 1, 0, projection)
    # RT V1's MAX blend state makes overlapping point coverage a union.
    f['glEnable'](0x0BE2)
    f['glBlendEquationSeparate'](0x8008, 0x8008)
    f['glBlendFuncSeparate'](1, 1, 1, 1)
    f['glDrawArraysInstanced'](0x0005, 0, 4, len(source_triples))
    coverage = gles.read()
    if f['glGetError']():
        raise RuntimeError('GLES error after native AlphaShader draw')
    return coverage


def composite(gles, coverage_texture, mask, shaders, replace):
    f = gles.f
    _, framebuffer = gles.target()
    f['glBindFramebuffer'](0x8D40, framebuffer)
    f['glViewport'](0, 0, WIDTH, HEIGHT)
    f['glDisable'](0x0BE2)
    f['glClearColor'](*BACKGROUND)
    f['glClear'](0x4000)
    vao = C.c_uint()
    f['glGenVertexArrays'](1, C.byref(vao))
    f['glBindVertexArray'](vao.value)
    vertices = [-1, -1, 1, -1, -1, 1, 1, 1]
    vbo = gles.buffer(0x8892, vertices)
    f['glEnableVertexAttribArray'](0)
    f['glVertexAttribPointer'](0, 2, 0x1406, 0, 0, None)
    prog = gles.program(shaders['composite'][0], shaders['composite'][1], ((0, 'aVertex'),))
    f['glUseProgram'](prog)
    f['glActiveTexture'](0x84C0)
    f['glBindTexture'](0x0DE1, coverage_texture)
    f['glUniform1i'](f['glGetUniformLocation'](prog, b'uSrcTexture'), 0)
    f['glUniform4f'](f['glGetUniformLocation'](prog, b'uInputColor'), *COLOR)
    if not replace:
        # InkPen RT V1 source-over descriptor: FUNC_ADD, ONE, ONE_MINUS_SRC_ALPHA.
        f['glEnable'](0x0BE2)
        f['glBlendEquationSeparate'](0x8006, 0x8006)
        f['glBlendFuncSeparate'](1, 0x0303, 1, 0x0303)
    f['glDrawArrays'](0x0005, 0, 4)
    pixels = gles.read()
    if f['glGetError']():
        raise RuntimeError('GLES error after native CompositeShader draw')
    return pixels


def capture(library, input_fixture):
    global INKPEN_BYTES
    INKPEN_BYTES = library.read_bytes()
    if hashlib.sha256(INKPEN_BYTES).hexdigest() != INKPEN_SHA256:
        raise ValueError('InkPen ELF does not match the pinned library hash')
    sources = {}
    for key, addresses in SHADERS.items():
        sources[key] = tuple(read_va(INKPEN_BYTES, address, 8192).split(b'\0', 1)[0].decode()
                             for address in addresses)
    shader_hashes = {key: [hashlib.sha256(src.encode()).hexdigest() for src in pair]
                     for key, pair in sources.items()}
    fixture = json.loads(input_fixture.read_text())
    cases = []
    gles = GLES()
    alpha_program = gles.program(*sources['alpha'])
    composite_program = gles.program(*sources['composite'], ((0, 'aVertex'),))
    for name in ('single sample tap', 'configured resolution 1080x1920',
                 'configured resolution 1440x2560', 'configured resolution 2160x3840',
                 'no curve three sample line', 'no curve tool 1 long move',
                 'no curve tool 3 move'):
        case = next(item for item in fixture['cases'] if item['name'] == name)
        triples = case['native']['callback_triples']
        # Verify point records derive from post-AddPoint float triples and are
        # complete; z is the native stored extent (raw drawPoint radius + .5).
        if not triples or any(len(row) != 3 for row in triples):
            raise ValueError('input native RTV1 buffer is not a sequence of triples')
        coverage_texture, coverage_fbo = gles.target()
        gles.f['glBindFramebuffer'](0x8D40, coverage_fbo)
        mask = draw_records(gles, triples, sources)
        pixels_add = composite(gles, coverage_texture, mask, sources, replace=False)
        pixels_replace = composite(gles, coverage_texture, mask, sources, replace=True)
        probe_xs = [30, 32, 34, 40, 48, 56, 64, 72, 80, 88, 96]
        y = HEIGHT // 2
        alpha_row = [mask[(y * WIDTH + x) * 4 + 3] for x in probe_xs]
        rgba_probe = lambda p: [list(p[(y * WIDTH + x) * 4:(y * WIDTH + x) * 4 + 4]) for x in probe_xs]
        cases.append({
            'input_case': name,
            'curve_enabled': case['curve_enabled'],
            'screen_resolution': case['screen_resolution'],
            'screen_resolution_readback': case['native']['setting_data_screen_resolution'],
            'input_point_count': len(triples),
            'input_triples_sha256': hashlib.sha256(json.dumps(triples, separators=(',', ':')).encode()).hexdigest(),
            'input_extent_min_max': [min(v[2] for v in triples), max(v[2] for v in triples)],
            'coverage_sha256_rgba8': gles.image_hash(mask),
            'coverage_centerline_alpha_samples': alpha_row,
            'composite_source_over_sha256_rgba8': gles.image_hash(pixels_add),
            'composite_source_over_samples': rgba_probe(pixels_add),
            'composite_replace_sha256_rgba8': gles.image_hash(pixels_replace),
            'composite_replace_samples': rgba_probe(pixels_replace),
        })
    return {
        'version': 1,
        'provenance': 'Embedded GLSL and RT V1 quad data from Samsung Notes 4.4.45.37 hash-pinned libSPenInkPen.so; source triples from inkpen-v4.json native post-AddPoint vectors; shader execution on host surfaceless EGL/GLES.',
        'inkpen_sha256': INKPEN_SHA256,
        'shader_virtual_addresses': {k: [f'{a:#x}' for a in v] for k, v in SHADERS.items()},
        'shader_sha256': shader_hashes,
        'base_quad_virtual_address': f'{QUAD_VA:#x}',
        'base_quad_rect_points': [list(struct.unpack('<4f', read_va(INKPEN_BYTES, QUAD_VA + i * 16, 16))) for i in range(4)],
        'gles_host': gles.identity,
        'host_render_profile': {
            'viewport': [WIDTH, HEIGHT],
            'projection': 'x scale 2/width and translation -0.5; y scale 2/height and translation 0; pixel units; explicit synthetic canvas.',
            'coverage_stage': 'actual embedded InkPenStrokeAlphaShader; instanced exact RT V1 base quad and native post-AddPoint triples; GL_MAX, ONE/ONE, transparent RGBA8 target.',
            'composite_stage': 'actual embedded InkPenStrokeCompositeShader; host-supplied color and opaque background; compare RT V1 source-over descriptor (FUNC_ADD, ONE, ONE_MINUS_SRC_ALPHA) with replace descriptor (FUNC_ADD, ONE, ZERO).',
            'color_rgba': list(COLOR),
            'background_rgba': list(BACKGROUND),
            'limitations': ['does not execute native RTV1::Draw, RendererFactory, shader bindings, PenUtil projection, target lifecycle or Android GL context', 'pixel hashes are specific to the recorded host GLES renderer/version', 'no Samsung GPU/device pixel equivalence claim', 'no overlapping-stroke/page compositing or SVG/PDF parity claim'],
        },
        'cases': cases,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--library', type=Path, default=Path(INKPEN_PATH))
    parser.add_argument('--input-fixture', type=Path, default=Path(INPUT_FIXTURE))
    parser.add_argument('--fixture', type=Path, default=Path(FIXTURE))
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--write-fixture', action='store_true')
    mode.add_argument('--check-fixture', action='store_true')
    args = parser.parse_args()
    result = capture(args.library, args.input_fixture)
    if args.write_fixture:
        args.fixture.write_text(json.dumps(result, indent=2) + '\n')
    else:
        expected = json.loads(args.fixture.read_text())
        if result != expected:
            raise SystemExit('InkPen paint fixture mismatch; inspect host GLES identity and shader pixels before updating')
    print('GLES', result['gles_host'])
    for case in result['cases']:
        print(f"{case['input_case']}: {case['input_point_count']} stamps; coverage={case['coverage_sha256_rgba8']}; composite={case['composite_source_over_sha256_rgba8']}")


if __name__ == '__main__':
    main()

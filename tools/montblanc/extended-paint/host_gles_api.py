"""Typed GLES bindings and bounded RGBA8 target readback stages."""
import ctypes as C
import hashlib

REQUIRED_F = (
    'glBindFramebuffer', 'glViewport', 'glClearColor', 'glClear', 'glGetError',
    'glReadPixels', 'glCheckFramebufferStatus',
)


def _typed_symbol(library, name, restype, argtypes):
    raw = getattr(library, name, None)
    if raw is None:
        raise RuntimeError(f'missing GLES symbol {name}')
    try:
        address = C.cast(raw, C.c_void_p).value
    except (TypeError, ValueError) as exc:
        raise RuntimeError(f'GLES symbol {name} has no native function address') from exc
    if not address:
        raise RuntimeError(f'GLES symbol {name} has null function address')
    return C.CFUNCTYPE(restype, *argtypes)(address)


def bind_host_gles(gles):
    """Validate adapter-owned typed functions and bind the two missing APIs."""
    functions = getattr(gles, 'f', None)
    if not isinstance(functions, dict):
        raise RuntimeError('canonical GLES typed function table is missing')
    missing = [name for name in REQUIRED_F if not callable(functions.get(name))]
    if missing:
        raise RuntimeError(f'canonical GLES function table lacks {missing!r}')
    gl = dict((name, functions[name]) for name in REQUIRED_F)
    # GLES exposes these on the same pinned CDLL; neither is in inkpen_paint_native.GLES.f.
    gl['glFinish'] = _typed_symbol(gles.gl, 'glFinish', None, [])
    gl['glGetFramebufferAttachmentParameteriv'] = _typed_symbol(
        gles.gl, 'glGetFramebufferAttachmentParameteriv', None,
        [C.c_uint, C.c_uint, C.c_uint, C.POINTER(C.c_int)])
    return gl


def _read_bytes(buf):
    raw = getattr(buf, 'raw', None)
    if raw is None:
        raise RuntimeError('readback buffer does not expose bounded raw bytes')
    return bytes(raw)


def prepare_target(gl, width, height, framebuffer, make_buffer=C.create_string_buffer):
    """Clear the selected target, read RGBA8, and fail closed on GL errors."""
    gl['glBindFramebuffer'](0x8D40, framebuffer)
    gl['glViewport'](0, 0, width, height)
    gl['glClearColor'](0.0, 0.0, 0.0, 0.0)
    gl['glClear'](0x00004000)
    before = gl['glGetError']()
    if before:
        raise RuntimeError(f'preexisting GLES error before pre-draw readback: {before:#x}')
    buf = make_buffer(width * height * 4)
    gl['glReadPixels'](0, 0, width, height, 0x1908, 0x1401, buf)
    data = _read_bytes(buf)
    after = gl['glGetError']()
    if after:
        raise RuntimeError(f'host target initialization/readback raised GLES error: {after:#x}')
    if len(data) != width * height * 4:
        raise RuntimeError('pre-draw RGBA8 readback length mismatch')
    if any(data):
        raise RuntimeError('host-cleared pre-draw RGBA8 target is not transparent black')
    return {'bytes': data, 'sha256': hashlib.sha256(data).hexdigest(),
            'gl_error_before': before, 'gl_error_after': after}


def finish_target_readback(gl, width, height, framebuffer, texture,
                           make_buffer=C.create_string_buffer):
    """Finish draw, validate exact color attachment, read RGBA8 bytes."""
    gl['glFinish']()
    gl['glBindFramebuffer'](0x8D40, framebuffer)
    status = gl['glCheckFramebufferStatus'](0x8D40)
    if status != 0x8CD5:
        raise RuntimeError(f'native Draw target FBO incomplete: {status:#x}')
    before = gl['glGetError']()
    if before:
        raise RuntimeError(f'preexisting GLES error before attachment query/readback: {before:#x}')
    value = C.c_int()
    gl['glGetFramebufferAttachmentParameteriv'](0x8CA8, 0x8CE0, 0x8CD1, C.byref(value))
    if value.value != texture:
        raise RuntimeError('readback target color attachment differs from constructor-owned texture')
    buf = make_buffer(width * height * 4)
    gl['glReadPixels'](0, 0, width, height, 0x1908, 0x1401, buf)
    data = _read_bytes(buf)
    after = gl['glGetError']()
    if after:
        raise RuntimeError(f'host RGBA8 readback raised GLES error: {after:#x}')
    if len(data) != width * height * 4:
        raise RuntimeError('post-draw RGBA8 readback length mismatch')
    return {'bytes': data, 'sha256': hashlib.sha256(data).hexdigest(),
            'gl_error_before': before, 'gl_error_after': after,
            'framebuffer_status': status, 'attachment_texture': value.value}

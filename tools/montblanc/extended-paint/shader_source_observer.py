"""Read-only observer for the existing glShaderSource callback ABI."""
import struct

MAX_SHADER_COUNT=16
MAX_SHADER_BYTES=1_048_576
MAX_SINGLE_SHADER_BYTES=524_288


def _bounded_c_string(read_bytes, pointer, cap):
    if not pointer: raise ValueError('null shader source pointer')
    chunks=bytearray()
    for offset in range(cap):
        chunk=read_bytes(pointer+offset,1)
        if len(chunk)!=1: raise ValueError('short C-string memory read')
        if chunk==b'\0': return bytes(chunks)
        chunks.extend(chunk)
    raise ValueError('shader C string exceeds bound or lacks terminator')


def capture_gl_shader_source(read_register, read_memory, *, max_count=MAX_SHADER_COUNT,
                             max_total=MAX_SHADER_BYTES, max_one=MAX_SINGLE_SHADER_BYTES):
    """Return callback wrapper and telemetry sinks; always forwards once.

    GL ABI: X0 shader, X1 count, X2 pointer array, X3 optional length array.
    Telemetry read errors are recorded but never suppress/duplicate the captured
    native callback. The wrapper does not write guest registers or memory.
    """
    rows=[]; errors=[]
    def wrapped(prior):
        def callback():
            try:
                shader,count,array_ptr,lengths_ptr=(int(read_register(i)) for i in range(4))
                if not 0<count<=max_count: raise ValueError('shader string count outside bound')
                if not array_ptr: raise ValueError('null shader string pointer array')
                total=0; sources=[]
                for i in range(count):
                    ptr=struct.unpack('<Q',read_memory(array_ptr+8*i,8))[0]
                    if not ptr: raise ValueError(f'null shader source pointer at index {i}')
                    length=-1
                    if lengths_ptr:
                        length=struct.unpack('<i',read_memory(lengths_ptr+4*i,4))[0]
                    if length<0:
                        raw=_bounded_c_string(read_memory,ptr,max_one)
                    else:
                        if length>max_one: raise ValueError('shader source exceeds per-string bound')
                        raw=read_memory(ptr,length)
                        if len(raw)!=length: raise ValueError('short shader source read')
                    total+=len(raw)
                    if total>max_total: raise ValueError('shader source call exceeds total byte bound')
                    sources.append(raw.decode('utf-8'))
                rows.append({'shader':shader,'source':sources})
            except Exception as exc:
                errors.append(f'{type(exc).__name__}: {exc}')
            prior()
        return callback
    return wrapped,rows,errors

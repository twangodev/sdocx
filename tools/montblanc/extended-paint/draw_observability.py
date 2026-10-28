"""Read-only GLES draw state and compact shader/uniform/VBO diagnostics."""
import ctypes as C
import hashlib

MAX_ATTRS_CAP = 64
MAX_SHADER_BYTES = 1_048_576
MAX_SHADER_ROWS = 32


def _function(lib, name, restype, args):
    raw = getattr(lib, name, None)
    if raw is None:
        raise RuntimeError(f'missing GLES inspection symbol {name}')
    try:
        address = C.cast(raw, C.c_void_p).value
    except (TypeError, ValueError) as exc:
        raise RuntimeError(f'GLES inspection symbol {name} has no function address') from exc
    if not address:
        raise RuntimeError(f'GLES inspection symbol {name} has null address')
    return C.CFUNCTYPE(restype, *args)(address)


def bind_draw_query_api(gl_library):
    return {
        'get_integer': _function(gl_library, 'glGetIntegerv', None,
                                 [C.c_uint, C.POINTER(C.c_int)]),
        'get_attrib': _function(gl_library, 'glGetVertexAttribiv', None,
                                [C.c_uint, C.c_uint, C.POINTER(C.c_int)]),
        'get_pointer': _function(gl_library, 'glGetVertexAttribPointerv', None,
                                 [C.c_uint, C.c_uint, C.POINTER(C.c_void_p)]),
    }


def _integer(api, pname, index=None):
    out = C.c_int()
    if index is None:
        api['get_integer'](pname, C.byref(out))
    else:
        api['get_attrib'](index, pname, C.byref(out))
    return out.value


def snapshot_draw_attributes(api):
    """Read actual enabled vertex inputs and their ARRAY_BUFFER bindings."""
    max_count = _integer(api, 0x8869)  # GL_MAX_VERTEX_ATTRIBS
    if not 1 <= max_count <= MAX_ATTRS_CAP:
        raise RuntimeError(f'GL_MAX_VERTEX_ATTRIBS outside bounded diagnostic cap: {max_count}')
    rows = []
    for index in range(max_count):
        enabled = _integer(api, 0x8622, index)  # GL_VERTEX_ATTRIB_ARRAY_ENABLED
        if not enabled:
            continue
        pointer = C.c_void_p()
        api['get_pointer'](index, 0x8645, C.byref(pointer))  # ARRAY_POINTER
        row = {
            'index': index,
            'enabled': True,
            'size': _integer(api, 0x8623, index),
            'stride': _integer(api, 0x8624, index),
            'type': _integer(api, 0x8625, index),
            'normalized': bool(_integer(api, 0x886A, index)),
            'buffer': _integer(api, 0x889F, index),
            'divisor': _integer(api, 0x88FE, index),
            'pointer_offset': int(pointer.value or 0),
        }
        if row['buffer'] <= 0:
            raise RuntimeError(f'enabled vertex attribute {index} has no source buffer object')
        rows.append(row)
    if not rows:
        raise RuntimeError('Draw has no enabled vertex attributes')
    return {'max_vertex_attribs': max_count, 'enabled': rows}


def annotate_buffer_upload(gl_calls, before, bound_array_buffer):
    rows = [row for row in gl_calls[before:] if row.get('call') == 'glBufferData']
    if len(rows) != 1:
        raise RuntimeError(f'one glBufferData callback produced {len(rows)} ledger rows')
    row = rows[0]
    if not bound_array_buffer:
        raise RuntimeError('glBufferData has no observed bound ARRAY_BUFFER object')
    row['boundBuffer'] = int(bound_array_buffer)
    return row


def compact_shader_sources(shader_source_calls):
    if len(shader_source_calls) > MAX_SHADER_ROWS:
        raise RuntimeError('shader source observation exceeded bounded row cap')
    result=[]; total=0
    for row in shader_source_calls:
        sources=row.get('source')
        if not isinstance(sources,list) or not sources:
            raise RuntimeError('shader source call has no source strings')
        packed=[]
        for source in sources:
            if not isinstance(source,str): raise RuntimeError('shader source is not text')
            raw=source.encode('utf-8'); total+=len(raw)
            if total>MAX_SHADER_BYTES: raise RuntimeError('shader source text exceeded bounded byte cap')
            packed.append({'utf8_bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'text':source})
        result.append({'shader':int(row['shader']),'sources':packed})
    return result


def compact_uniform_trace(gl_calls, start=0):
    program=None; result=[]
    names={'glUniformMatrix4fv','glUniform4f','glUniform4fv','glUniform1i','glUniform1f'}
    for row in gl_calls[start:]:
        if row.get('call')=='glUseProgram':
            program=int(row['program'])
        elif row.get('call') in names:
            item={k:v for k,v in row.items() if k!='dataHex'}
            item['program_at_call']=program
            result.append(item)
    return result


def join_draw_provenance(draw_events, attribute_rows, buffer_upload_rows, native_vector_hashes):
    by_buffer={}
    for row in buffer_upload_rows:
        buf=int(row.get('boundBuffer',0))
        if buf>0:
            by_buffer.setdefault(buf,[]).append(row)
    result=[]
    for i,(event,attrs) in enumerate(zip(draw_events,attribute_rows)):
        state=event.get('state',{})
        actual_program=state.get('program')
        if not isinstance(actual_program,int) or actual_program<=0:
            raise RuntimeError('draw event lacks actual active program state')
        joined=[]
        for attr in attrs['enabled']:
            if int(attr.get('buffer',0))<=0:
                raise RuntimeError(f'enabled vertex attribute {attr.get("index")} has no source buffer object')
            uploads=by_buffer.get(attr['buffer'],[])
            latest=uploads[-1] if uploads else None
            joined.append({**attr,'latest_buffer_upload':None if latest is None else {
                'size':latest.get('size'),'usage':latest.get('usage'),
                'data_sha256':latest.get('dataSha256'),
                'matches_native_add_vector':latest.get('dataSha256') in native_vector_hashes,
            }})
        result.append({'draw_index':i,'mode_count':event.get('values'),
                       'actual_program':actual_program,'framebuffer':state.get('framebuffer'),
                       'vao':state.get('vao'),'attributes':joined})
    if len(draw_events)!=len(attribute_rows):
        raise RuntimeError('draw event and attribute snapshot counts differ')
    return result


def rgba8_statistics(data, width, height):
    expected=width*height*4
    if len(data)!=expected: raise ValueError(f'RGBA8 byte count {len(data)} differs from {expected}')
    pixels=[data[i:i+4] for i in range(0,len(data),4)]
    live=[px for px in pixels if px[3]!=0]
    if not live: raise RuntimeError('post-draw RGBA8 target contains no nontransparent pixels')
    return {
        'dimensions':[width,height], 'byte_count':len(data),
        'nontransparent_pixels':len(live),
        'channel_min':[min(px[i] for px in live) for i in range(4)],
        'channel_max':[max(px[i] for px in live) for i in range(4)],
        'unique_rgba_count':len(set(live)),
        'nontransparent_rgba_sha256':hashlib.sha256(b''.join(live)).hexdigest(),
    }


def current_array_buffer(api):
    return _integer(api, 0x8894)  # GL_ARRAY_BUFFER_BINDING


def require_native_vector_draw_joins(draw_rows, native_vector_hashes):
    matched={}
    for draw in draw_rows:
        for attr in draw['attributes']:
            upload=attr.get('latest_buffer_upload')
            if upload and upload.get('matches_native_add_vector'):
                matched[upload['data_sha256']]=True
    missing=set(native_vector_hashes)-set(matched)
    if missing:
        raise RuntimeError(f'native Add vector payloads are not joined to enabled Draw attributes: {sorted(missing)!r}')
    return sorted(matched)

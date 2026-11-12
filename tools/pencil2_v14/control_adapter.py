"""Checked compatibility hooks for the pinned V38 capture harness.

The historical harness exposes capture(), but its draw state is local. These
finite source hooks retain its original control blocks until that harness gains
an importable hook API. Historical scripts and receipts are never executed here.
"""
import gzip
import hashlib

SOURCE_RELATIVE = "tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/capture_candidate.py"
SOURCE_SHA256 = "34d5874cb86e27b619c01e9902b0aacf743619be560d8a4ca5ee65789ca8193d"

DRAW_ANCHOR = '    graphics=p.get(rt+200,"Q")[0]'

EDGE_DRAW = '''    for name, stream in EDGE_STREAMS.items():
        p.u.mem_write(vector_begin,stream)
        p.call(RT_UPDATE,[rt,0],instruction_limit=5_000_000)
        p.u.mem_write(vector_begin,actual_geometry)
        for dy in (0,-16,-75):
            transform=[2,0,0,0, 0,-2,0,0, 0,0,1,0, 0,192+dy,0,1]
            p.u.mem_write(matrix,struct.pack('<16f',*transform))
            p.call(ACTIVATE_SUBBITMAP_RT,[subbitmap,0],instruction_limit=5_000_000)
            f['glViewport'](0,0,width,height)
            f['glClearColor'](0.,0.,0.,0.); f['glClear'](0x00004000)
            f['glBindFramebuffer'](0x8d40,0)
            p.call(RT_DRAW,[rt,subbitmap,matrix],instruction_limit=8_000_000)
            f['glBindFramebuffer'](0x8d40,draw_target_fbo)
            f['glReadPixels'](0,0,width,height,0x1908,0x1401,pixels)
            if f['glGetError'](): raise RuntimeError('edge control GL error')
            frame=pixels.raw
            sparse=[[i//4%width,i//4//width,*frame[i:i+4]] for i in range(0,len(frame),4) if frame[i+3]]
            EDGE_RESULTS[name+':'+str(dy)]={'sha256':hashlib.sha256(frame).hexdigest(),'matrix':transform,'pixels':sparse}
            print(name,dy,len(sparse),flush=True)
    p.u.mem_write(vector_begin,actual_geometry)
    p.call(RT_UPDATE,[rt,0],instruction_limit=5_000_000)
'''

HORIZONTAL_DRAW = '''    for name, stream in EDGE_STREAMS.items():
        p.u.mem_write(vector_begin,stream)
        p.call(RT_UPDATE,[rt,0],instruction_limit=5_000_000)
        p.u.mem_write(vector_begin,actual_geometry)
        for dy in (0,1):
            transform=[2,0,0,0, 0,-2,0,0, 0,0,1,0, 0,192,0,1]
            p.u.mem_write(matrix,struct.pack('<16f',*transform))
            p.call(ACTIVATE_SUBBITMAP_RT,[subbitmap,0],instruction_limit=5_000_000)
            f['glViewport'](0,0,width*(1+dy),height)
            CLIP_MODE[0]=dy
            f['glClearColor'](0.,0.,0.,0.); f['glClear'](0x00004000)
            f['glBindFramebuffer'](0x8d40,0)
            event_start=len(p.gl_events)
            MATRIX_EVENTS.clear()
            p.call(RT_DRAW,[rt,subbitmap,matrix],instruction_limit=8_000_000)
            assert len(MATRIX_EVENTS) == 1, (name,dy,MATRIX_EVENTS)
            for event in MATRIX_EVENTS:
                expected = list(event['before'])
                if dy:
                    expected[0] *= 0.5; expected[12] = expected[12] * 0.5 - 0.5
                expected = list((C.c_float * len(expected))(*expected))
                assert event['uploaded'] == expected, (name,dy,event,expected)
            viewport=(C.c_int*4)()
            p._cfunc('glGetIntegerv',None,[C.c_uint,C.POINTER(C.c_int)])(0x0ba2,viewport)
            events=[e for e in p.gl_events[event_start:] if e['name']=='glViewport']
            assert list(viewport)==[0,0,width*(1+dy),height], (name,dy,list(viewport),events)
            f['glBindFramebuffer'](0x8d40,draw_target_fbo)
            f['glReadPixels'](0,0,width,height,0x1908,0x1401,pixels)
            if f['glGetError'](): raise RuntimeError('edge control GL error')
            frame=pixels.raw
            sparse=[[i//4%width,i//4//width,*frame[i:i+4]] for i in range(0,len(frame),4) if frame[i+3]]
            EDGE_RESULTS[name+':'+str(dy)]={'sha256':hashlib.sha256(frame).hexdigest(),'matrix':transform,'viewport_after_draw':list(viewport),'draw_viewport_events':events,'projection_uploads':list(MATRIX_EVENTS),'projection_upload_count':len(MATRIX_EVENTS),'pixels':sparse}
            print(name,dy,len(sparse),flush=True)
    CLIP_MODE[0]=0
    p.u.mem_write(vector_begin,actual_geometry)
    p.call(RT_UPDATE,[rt,0],instruction_limit=5_000_000)
'''

TRANSLATION_CASE_LOOP = '    for case in TRANSLATION_CASES:\n        name, stream = case.receipt_key, case.stream\n'
HISTORICAL_TRANSLATION_LOOP = '    for name, stream in EDGE_STREAMS.items():\n'
TRANSLATION_COORDINATES = '        dx, down = case.x_pixels, case.down_pixels\n'
HISTORICAL_TRANSLATION_COORDINATES = "        dx, down, _ = name.split('_', 2)\n"

TRANSLATION_DRAW = '''    for case in TRANSLATION_CASES:
        name, stream = case.receipt_key, case.stream
        p.u.mem_write(vector_begin,stream)
        p.call(RT_UPDATE,[rt,0],instruction_limit=5_000_000)
        p.u.mem_write(vector_begin,actual_geometry)
        dx, down = case.x_pixels, case.down_pixels
        for dy in (0,1):
            transform=[2,0,0,0, 0,-2,0,0, 0,0,1,0, int(dx),192-int(down),0,1]
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
            assert list(viewport)==[0,0,width,height*(1+dy)], (name,dy,list(viewport),events)
            f['glBindFramebuffer'](0x8d40,draw_target_fbo)
            f['glReadPixels'](0,0,width,height,0x1908,0x1401,pixels)
            if f['glGetError'](): raise RuntimeError('edge control GL error')
            frame=pixels.raw
            sparse=[[i//4%width,i//4//width,*frame[i:i+4]] for i in range(0,len(frame),4) if frame[i+3]]
            EDGE_RESULTS[name+':'+str(dy)]={'sha256':hashlib.sha256(frame).hexdigest(),'matrix':transform,'viewport_after_draw':list(viewport),'draw_viewport_events':events,'pixels':sparse}
            print(name,dy,len(sparse),flush=True)
    CLIP_MODE[0]=0
    p.u.mem_write(vector_begin,actual_geometry)
    p.call(RT_UPDATE,[rt,0],instruction_limit=5_000_000)
'''

MATRIX_ANCHOR = '            vals=self.get(ptr,"f"*(16*count)); arr=(C.c_float*len(vals))(*vals)'

HORIZONTAL_MATRIX = '''            vals=self.get(ptr,"f"*(16*count))
            if loc == 0 and count == 1:
                original=list(vals)
                if CLIP_MODE[0]:
                    vals=list(vals); vals[0]*=0.5; vals[12]=vals[12]*0.5-0.5
                vals=list((C.c_float*len(vals))(*vals))
                MATRIX_EVENTS.append({'before':original,'uploaded':vals})
            arr=(C.c_float*len(vals))(*vals)'''

PROJECTION_MATRIX = '''            vals=self.get(ptr,"f"*(16*count))
            if loc == 0 and count == 1:
                original=list(vals)
                if CLIP_MODE[0]:
                    vals=list(vals); vals[5]=struct.unpack('<f',struct.pack('<I',struct.unpack('<I',struct.pack('<f',vals[5]))[0]+1))[0]
                vals=list((C.c_float*len(vals))(*vals))
                MATRIX_EVENTS.append({'before':original,'uploaded':vals})
            arr=(C.c_float*len(vals))(*vals)'''

VIEWPORT_ANCHOR = '''            values = [self.x(i) for i in range(argc)]
            if argtypes is None:'''

HORIZONTAL_VIEWPORT = '''            values = [self.x(i) for i in range(argc)]
            if name == "glViewport" and CLIP_MODE[0]:
                assert values == [0,0,192,192], values
                values[2] *= 2
            if argtypes is None:'''


VERTICAL_MATRIX = '''            vals=self.get(ptr,"f"*(16*count))
            if CLIP_MODE[0] and loc == 0 and count == 1:
                vals=list(vals); vals[5]*=0.5; vals[13]=vals[13]*0.5-0.5
            arr=(C.c_float*len(vals))(*vals)'''

VERTICAL_VIEWPORT = '''            values = [self.x(i) for i in range(argc)]
            if name == "glViewport" and CLIP_MODE[0]:
                assert values == [0,0,192,192], values
                values[3] *= 2
            if argtypes is None:'''


def replace_once(source, anchor, replacement):
    if source.count(anchor) != 1:
        raise ValueError(f"Expected one historical source anchor: {anchor}")
    return source.replace(anchor, replacement, 1)


def projection_draw():
    draw = HORIZONTAL_DRAW
    draw = replace_once(draw, "            f['glViewport'](0,0,width*(1+dy),height)\n", "            f['glViewport'](0,0,width,height)\n")
    draw = replace_once(draw, '                    expected[0] *= 0.5; expected[12] = expected[12] * 0.5 - 0.5\n', "                    expected[5] = struct.unpack('<f', struct.pack('<I', struct.unpack('<I', struct.pack('<f', expected[5]))[0] + 1))[0]\n")
    draw = replace_once(draw, '            assert list(viewport)==[0,0,width*(1+dy),height], (name,dy,list(viewport),events)\n', '            assert list(viewport)==[0,0,width,height], (name,dy,list(viewport),events)\n')
    return draw


def prepare_program(root, variant):
    if variant not in ("edge", "horizontal", "projection", "translation"):
        raise ValueError(f"Unknown Pencil2 control: {variant}")
    path = root / SOURCE_RELATIVE
    raw = gzip.decompress((root / "tools/pencil2_v14/context-base-capture.py.gz").read_bytes())
    if hashlib.sha256(raw).hexdigest() != SOURCE_SHA256:
        raise ValueError(f"Historical capture source hash mismatch: {path}")
    source = raw.decode()
    draw = EDGE_DRAW
    if variant == "translation":
        source = replace_once(source, MATRIX_ANCHOR, VERTICAL_MATRIX)
        source = replace_once(source, VIEWPORT_ANCHOR, VERTICAL_VIEWPORT)
        draw = TRANSLATION_DRAW
    elif variant != "edge":
        matrix = HORIZONTAL_MATRIX if variant == "horizontal" else PROJECTION_MATRIX
        viewport = HORIZONTAL_VIEWPORT
        if variant == "projection":
            viewport = replace_once(viewport, "values[2] *= 2", "values[2] = values[2]")
        source = replace_once(source, MATRIX_ANCHOR, matrix)
        source = replace_once(source, VIEWPORT_ANCHOR, viewport)
        draw = HORIZONTAL_DRAW if variant == "horizontal" else projection_draw()
    return replace_once(source, DRAW_ANCHOR, draw + DRAW_ANCHOR)


def historical_translation_program(program):
    """Recover the pinned program by undoing only named-case access changes."""
    program = replace_once(program, TRANSLATION_CASE_LOOP, HISTORICAL_TRANSLATION_LOOP)
    return replace_once(program, TRANSLATION_COORDINATES, HISTORICAL_TRANSLATION_COORDINATES)

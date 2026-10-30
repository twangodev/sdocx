import hashlib, os, pathlib, tempfile
root=pathlib.Path(__file__).resolve().parents[2]
runner = root / 'tmp/conformance-cleanup/native-capture-a241793/conformance/inkpen_joined_paint_native.py'
caller = runner.with_name('inkpen_saved_caller_native.py')
assert hashlib.sha256(runner.read_bytes()).hexdigest() == '7a05db658343956af992823ee5a17622fa94a08c88152be9f39cee950eca4a79'
assert hashlib.sha256(caller.read_bytes()).hexdigest() == 'c0b1657cdcf94bcf2c173ce68ed7817e55a4de246883b47265bb7b4d3f50dc04'
source=(root/'tools/inkpen-capture/inkpen_v1_scale_capture.py').read_text()
anchor='exec(compile(source, str(runner)'
color=int(os.environ.get('INKPEN_COLOR','0xff336699'), 0)
width=float(os.environ.get('FIXED_WIDTH','6'))
tool=int(os.environ.get('TOOL','1'))
curve=os.environ.get('CURVE','1') == '1'
inject='''anchor = "caller_source='queue_task_events=[]\\\\nqueue_task_active=[]\\\\n'+caller_source"
fixed_anchor = "p.f(0,input_case['size']); p.call('SPen::ObjectStroke::SetPenSize(float)',[obj]);"
fixed_replace = "p.call('SPen::ObjectStroke::SetColor(unsigned int)',[obj,%%d]); " %% color + "p.call('SPen::ObjectStroke::SetFixedWidthEnabled(bool)',[obj,1]); p.f(0,%r); p.call('SPen::ObjectStroke::SetFixedWidth(float)',[obj]); " + fixed_anchor
edit = "assert caller_source.count(%%r)==1; caller_source=caller_source.replace(%%r,%%r); caller_source=caller_source.replace(\\\"'tool_type':1\\\",\\\"'tool_type':%d\\\").replace(\\\"'curve_enabled':True\\\",\\\"'curve_enabled':%s\\\")\\n" %% (fixed_anchor,fixed_anchor,fixed_replace)
assert source.count(anchor)==1
source=source.replace(anchor,edit+anchor)
''' % (width,tool,curve)
assert source.count(anchor)==1
source=source.replace(anchor,inject+anchor,1)
with tempfile.TemporaryDirectory(prefix='sdocx-v1-fixed-capture-') as temporary:
    old = "exec(compile(source, str(runner)"
    edit = "capture_anchor = \"capture_dir=ROOT/'tmp/vector-parity-plan/inkpen-drawing-paint'\"\nassert source.count(capture_anchor)==1\nsource=source.replace(capture_anchor, \"capture_dir=Path(\" + repr(temporary) + \")\")\n"
    assert source.count(old) == 1
    source = source.replace(old, edit + old)
    exec(compile(source,str(root/'tools/inkpen-capture/inkpen_v1_scale_capture.py'),'exec'),{'__name__':'__main__','__file__':str(root/'tools/inkpen-capture/inkpen_v1_scale_capture.py'),'temporary':temporary,'color':color})

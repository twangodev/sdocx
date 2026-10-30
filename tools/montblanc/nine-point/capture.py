import hashlib, json, os, sys, tempfile
from pathlib import Path

root = Path(os.environ['SDOCX_CAPTURE_ROOT'])
runner = root / 'tools/montblanc/extended-paint/native_drawing_capture.py'
host = root / 'tools/montblanc/extended-paint/capture_candidate.py'
assert hashlib.sha256(runner.read_bytes()).hexdigest() == 'a8647b96fa001dbe387de8c231ed4895d2f7b5ef619240835c17867366f8c0c8'
assert hashlib.sha256(host.read_bytes()).hexdigest() == '7378931c784f1be00527da5dd8d39dd48597b95a8c5bcc2c636fdf69d82a2fae'
case = json.loads(os.environ['MB_CASE'])
s = runner.read_text().replace('CASE_INPUT=CASE_INPUTS[CASE]', 'CASE_INPUT='+repr(case))
runtime_dir = tempfile.TemporaryDirectory(prefix='montblanc-runtime-')
runtime_path = Path(runtime_dir.name)
libraries = root / 'scratch/apk-analysis-native/arm64-v8a'
for name in ('libSPenBase.so','libSPenPenCommon.so','libSPenFountainPen.so','libSPenAirBrushPen.so','libSPenGraphics.so','libSPenRenderer.so','libSPenMarker3.so','libc++_shared.so','libSPenCxx.so'):
    (runtime_path / name).symlink_to(libraries / ('libc++_shared.so' if name == 'libSPenCxx.so' else name))
s = s.replace("RUNTIME_INPUTS=Path(__file__).resolve().parent/'runtime-inputs'", 'RUNTIME_INPUTS=Path('+repr(str(runtime_path))+')')
a = s.index('# Exactly three sample positions'); b = s.index('model_arrays={}', a)
s = s[:a] + '''n=len(CASE_INPUT['points'])
pts=p.alloc(n*8);prs=p.alloc(n*4);ts=p.alloc(n*4);tilt=p.alloc(n*4);ori=p.alloc(n*4)
p.put(pts,str(n*2)+'f',*sum(CASE_INPUT['points'],[]));p.put(prs,str(n)+'f',*CASE_INPUT['pressure']);p.put(ts,str(n)+'i',*[100+10*i for i in range(n)]);p.put(tilt,str(n)+'f',*[0]*n);p.put(ori,str(n)+'f',*[0]*n)
call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pts,prs,ts,tilt,ori,n])
''' + s[b:]
s = s.replace("'timestamps':[100,110,120],'tilt':[0,0,0],'orientation':[0,0,0]", "'timestamps':[100+10*i for i in range(n)],'tilt':[0]*n,'orientation':[0]*n")
s = s.replace("if event['method'].endswith('AddEndPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)') and rtv1_mesh_snapshots['add_vectors'] is None:", "if '::Add' in event['method']:")
s = s.replace("'captured_after':'final AddEndPoint body return'", "'captured_after':'latest Add body return'")
s = s[:s.index('if MODE_ARGS.write_fixture:')]
host_source = host.read_text().replace("try: ns=runpy.run_path(str(V2),run_name='__main__')", "try:\n ns={'__file__':str(V2),'__name__':'__main__'}; exec(compile(adapted_source,str(V2),'exec'),ns)")
host_source = host_source.replace("V2=ROOT/'tmp/vector-parity-plan/montblanc-rtv1-mesh-capture-root-v2/montblanc_state_controls.py'", "V2=ROOT/'tools/montblanc/extended-paint/native_drawing_capture.py'")
host_source = host_source.replace('V2 real Drawing/ObjectStroke/RTV1 Add* source producer is replayed unchanged.', 'Adapted variable-point synthetic Drawing/ObjectStroke/RTV1 Add* source producer executes.')
host_source = host_source.replace("p=ns['p']; runtime=ns['rt'];", "actual = [[row['entry'], row['float_words_s0_s5']] for row in ns['records'][0]['actual_montblanc_rtv1_addpoint_records']]\nassert actual == expected_stream, 'native geometry stream differs from retained control'\np=ns['p']; runtime=ns['rt'];")
fixture = json.loads((root / 'conformance/montblanc-nine-point-native.json').read_text())
assert fixture['input'] == case
expected_stream = fixture['result']['stream']
sys.path.insert(0,str(host.parent))
ns = {'__file__':str(host),'__name__':'__main__','adapted_source':s,'expected_stream':expected_stream}
exec(compile(host_source,str(host),'exec'),ns)

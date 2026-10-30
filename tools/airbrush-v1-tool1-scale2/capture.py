import os,sys,subprocess,tempfile,pathlib,json,hashlib
root=pathlib.Path(__file__).resolve().parents[2]; runner=root/'tmp/vector-parity-plan/pen-family-audit/airbrush-v1-short-native/frame-capture.py'; source=runner.read_text(); assert hashlib.sha256(source.encode()).hexdigest()=='d38a83f3cf71b228f58654c08a135d3f91d3bda0349df1d6014926752dc3b73e'
for path,expected in json.loads((root/'tools/pen-capture/airbrush_v1_curved_prerequisites.json').read_text()).items(): assert hashlib.sha256((root/path).read_bytes()).hexdigest()==expected
env=os.environ.copy();env['AB_N']='5';env['PYTHONPATH']=':'.join(str(root/p) for p in ('scratch/apk-analysis-runtime/python','scratch/apk-analysis-runtime/binutils-aarch64/usr/bin','tmp/conformance-cleanup/native-capture-a241793/conformance'));env['LD_LIBRARY_PATH']=str(root/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')
s=source.replace("'points':[_probe_start]*n", "'points':[[2*(_probe_start[0]-i*2.5),2*(_probe_start[1]-i*1.25)] for i in range(n)]").replace("'tool_type':2,'stroke_type':0", "'tool_type':1,'stroke_type':0")
with tempfile.NamedTemporaryFile(mode='w',suffix='.py',dir=runner.parent) as f:
 f.write(s);f.flush()
 with open(sys.argv[1],'x') as o: subprocess.run([sys.executable,f.name,'--single-case','--print-capture','--advanced-mode','unset','--width','512','--height','512','--source-size','4.5','--color-argb','0xffdf2255'],cwd=root,env=env,stdout=o,check=True)
lines=pathlib.Path(sys.argv[1]).read_text().splitlines()
frame=json.loads(lines[0][len('FRAME_STATE '):])['post_frame']
replay=json.loads(lines[1]); receipt=json.loads((pathlib.Path(__file__).parent/'receipt.json').read_text())
assert frame['sha256']==receipt['frame']['sha256']
assert replay['source_arrays']==receipt['source_arrays']
assert [event['delta_bytes'] for event in replay['producer_events']]==[event['delta_bytes'] for event in receipt['producer_events']]
print('Native frame, input arrays and stamp stream match the retained receipt.')

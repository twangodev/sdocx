from pathlib import Path
import tempfile, subprocess, json, os
root=Path(__file__).resolve().parents[2]; here=root/'tools/pencil2-serialized-bridge'; scratch=Path(tempfile.mkdtemp(prefix='sdocx-pencil2-tolerance-'))
code=(here/'capture.py').read_text(); code=code.replace("root = Path(__file__).resolve().parents[2]",f"root = Path({str(root)!r})"); code=code.replace('str(Path(__file__).resolve().parent)',repr(str(scratch))); code=code.replace("Path(__file__).with_name('receipt.json')",f"Path({str(scratch/'receipt.json')!r})"); code=code.replace("Path(__file__).resolve().parent / 'first.zlib'",f"Path({str(scratch/'first.zlib')!r})"); code=code.replace("Path(__file__).resolve().parent / 'second.zlib'",f"Path({str(scratch/'second.zlib')!r})")
hook=''' for producer_name, producer_symbol in (('drawLine', 'SPen::Pencil2StrokeDrawableGLV14::drawLine(float, float, float, float, float, SPen::RectF*, bool)'),):
  if a == p.syms[producer_symbol] and phase == 'second_draw_object_stroke':
   obj=u.reg_read(R.UC_ARM64_REG_X0)
   event_order.append({'event':'RTV4::producer_'+producer_name,'object_bytes':bytes(u.mem_read(obj,272)).hex(),'s_bits':[u.reg_read(getattr(R,'UC_ARM64_REG_S'+str(i))) for i in range(5)]})
'''
inject="    code = code.replace('spec.loader.exec_module(m)', 'spec.loader.exec_module(m)\\n' + "+repr('m.P2_BUFFER_HOOK = '+repr(hook)+' + m.P2_BUFFER_HOOK')+")\n"
inject+="    code = code.replace(\" result['native_target_probe']=target\", \" result['native_target_probe']=target\\n result['producer_events']=[row for row in event_order if row['event'].startswith('RTV4::producer_')]\")\n"
marker="    for name in ('first', 'second'):\n"
anchor='original = source.read_text()'
code=code.replace(anchor,anchor+'\noriginal = original.replace('+repr(marker)+', '+repr(inject+marker)+')\noriginal = original.replace('+repr("'outer_outcome':target.get('outcome')")+', '+repr("'events':capture.get('producer_events'), 'outer_outcome':target.get('outcome')")+')')
value=float(os.environ.get('TOL', '0.47816234827041626'))
needle="payload=bytes.fromhex(source['payload_hex']); data=p.alloc(len(payload))"
code=code.replace(needle, "payload=bytes.fromhex(source['payload_hex']); payload=payload[:-4]+struct.pack('<f', "+repr(value)+"); data=p.alloc(len(payload))")
# no further compact processing
code=code[:code.index("receipt_path = ")]
with tempfile.NamedTemporaryFile(mode='w', prefix='.orig-trace-', suffix='.py', dir=here) as script:
 script.write(code); script.flush(); subprocess.run(['python3',script.name],cwd=root,check=True)
d=json.loads((scratch/'receipt.json').read_text()); print('tolerance', value, 'geometry', d['second']['geometry_bytes']//56, d['second']['geometry_sha256'], 'frame', d['second_sha256'], 'events',len(d['events'])); import struct
for i in (0,1,2,12,14,35):
 b=bytes.fromhex(d['events'][i]['object_bytes']);print(i+1,struct.unpack_from('<f',b,44)[0],b[48],b[257],struct.unpack_from('<f',b,264)[0])


import shutil
out=Path(os.environ['P2_DIR']); shutil.rmtree(out,ignore_errors=True); shutil.copytree(scratch,out); shutil.rmtree(scratch)

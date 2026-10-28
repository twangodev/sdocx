import sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'tmp/conformance-cleanup/native-capture-a241793/conformance'))
import brushpen_queue_native as q
p=q.HandoffProbe()
p.synthetic_bitmap_matrix=[float(x) for x in __import__('os').environ.get('BRUSHPEN_BITMAP_MATRIX','1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1').split(',')]
for n in ['SPen::ObjectDrawing::ObjectDrawing()','SPen::ObjectDrawing::Construct(SPen::IContext const*)','SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)','SPen::SPGraphicsFactory::CreatePenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*, int)','createPenInst']:
 print(n,hex(p.syms.get(n,0)))
print('callbacks', [n for n in p.callbacks if 'CreatePenCanvas' in n or 'createPenInst' in n or 'GetDrawable' in n])
print('bases maybe',[(n,hex(a)) for n,a in p.syms.items() if 'ObjectDrawing::drawObjectStroke' in n][:4])
print('regions',[(hex(a),hex(b)) for a,b,_ in p.u.mem_regions() if 0x6000000 <= a <= 0x8000000])
print('symbols6',[(n,hex(a)) for n,a in p.syms.items() if 0x6000000<=a<0x7000000][:10])
print('symbols7',[(n,hex(a)) for n,a in p.syms.items() if 0x7000000<=a<0x8000000][:5])
print('drawingish',[n for n,a in p.syms.items() if 'ObjectDrawing' in n][:20])
print('libs addresses',[(n,hex(a)) for n,a in p.syms.items() if n=='JNI_OnLoad'])

import os,sys,json,hashlib
from pathlib import Path
root=Path(__file__).resolve().parents[3]
source=root/'tmp/vector-parity-plan/montblanc-rtv1-mesh-capture-root-v2/montblanc_state_controls.py'
raw=source.read_bytes()
if hashlib.sha256(raw).hexdigest() != 'a8647b96fa001dbe387de8c231ed4895d2f7b5ef619240835c17867366f8c0c8':
    raise SystemExit('native harness hash mismatch')
s=raw.decode(); case=json.loads(os.environ['MB_CASE'])
s=s.replace('CASE_INPUT=CASE_INPUTS[CASE]', 'CASE_INPUT='+repr(case))
a=s.index('# Exactly three sample positions'); b=s.index('model_arrays={}',a)
s=s[:a]+'''n=len(CASE_INPUT['points'])
pts=p.alloc(n*8);prs=p.alloc(n*4);ts=p.alloc(n*4);tilt=p.alloc(n*4);ori=p.alloc(n*4)
p.put(pts,str(n*2)+'f',*sum(CASE_INPUT['points'],[]));p.put(prs,str(n)+'f',*CASE_INPUT['pressure']);p.put(ts,str(n)+'i',*[100+10*i for i in range(n)]);p.put(tilt,str(n)+'f',*[0]*n);p.put(ori,str(n)+'f',*[0]*n)
call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pts,prs,ts,tilt,ori,n])
'''+s[b:]
marker="p.u.hook_del(alpha_source_hook)\nmanager=p.get(drawing+16,'Q')[0]"
s=s[:s.index(marker)]+'''\nstream=[[row['entry'],row['float_words_s0_s5']] for row in add_calls]

selected=('RedrawPen(', 'redrawLine(', 'drawLine(', 'endPen(', 'SmPath::moveTo(', 'SmPath::quadTo(', 'SmPath::cubicTo(', 'SmPath::resetPath(', 'SmPath::getLength(', 'getRepeatEff(', 'AddStartPoint(', 'AddEndPoint(')
events=[]
for row in state_trace:
    if any(name in row['symbol'] for name in selected):
        event={'id':row['id'],'symbol':row['symbol'],'caller_rva':hex(int(row['lr'],16)-mbbase),'s0_s3_f32_bits':row['s0_s5_f32_bits'][:4]}
        if 'getRepeatEff(' in row['symbol']:
            event['result_x0']=int(row['return']['x0_x3'][0],16)
        events.append(event)
result={'return':rc,'model_count':model_count,'size':model_size_bits,'add_count':len(stream),'events':events}

'''
sys.argv=[str(source),'--case','baseline']; ns={'__file__':str(source),'__name__':'__main__'};exec(compile(s,str(source),'exec'),ns);print(json.dumps(ns['result']))

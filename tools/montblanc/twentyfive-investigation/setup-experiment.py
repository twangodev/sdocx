import json,pathlib
root=pathlib.Path('/home/jding/RustroverProjects/sdocx'); dest=pathlib.Path('/tmp/sdocx-mb25-run'); ink=dest/'crates/sdocx/src/ink'
points=[[x, y] for x,y in zip(range(0,100,4),[0,2,4,6,8,6,4,2,0,-2,-4,-6,-8,-6,-4,-2,0,2,4,6,8,6,4,2,0])]; pressure=[0.5]*25
stream=json.loads(next(x.partition('=')[2] for x in pathlib.Path('/tmp/sdocx-montblanc-25.stderr').read_text().splitlines() if x.startswith('CAPTURE_STREAM=')))
(ink/'montblanc_25_stream.json').write_text(json.dumps({'case':{'input':{'points':points,'pressure':pressure,'pen_size':1},'result':{'stream':stream}}}))
p=ink/'montblanc_many.rs'; s=p.read_text().replace('(6..=24)', '(6..=25)'); start=s.index('fn nine_point_investigative_native_stream()'); end=s.index('\n}\n',start)+2; test=s[start:end].replace('nine_point_investigative_native_stream','twentyfive_native_stream').replace('montblanc_nine_streams.json','montblanc_25_stream.json'); p.write_text(s+'\n#[test]\n'+test+'\n')
p=ink/'montblanc.rs'; s=p.read_text().replace('(3..=24)', '(3..=25)').replace('21..=24 if size', '21..=25 if size'); p.write_text(s)
p=root/'tools/montblanc/many-point-public/src/main.rs'; s=p.read_text(); a=s.index('    for (name, coords, pressure, size) in ['); b=s.index('    ] {',a)+len('    ] {'); fmt=lambda v:f'{float(v):.8f}'; row='("twentyfive-1.0",vec!['+','.join('['+','.join(map(fmt,x))+']' for x in points)+'],vec!['+','.join(map(fmt,pressure))+'],1.0),'; s=s[:a]+'    for (name, coords, pressure, size) in [\n'+row+'\n    ] {'+s[b:]; (dest/'tools/montblanc/many-point-public/src/main.rs').write_text(s)

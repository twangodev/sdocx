import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

root = Path(sys.argv[1]).resolve()
lib = Path(sys.argv[2]).resolve()
unicorn = Path(sys.argv[3]).resolve()
package = root / 'tools/named_arc_rotation'
assert hashlib.sha256((unicorn / 'libunicorn.so.2').read_bytes()).hexdigest() == 'ddb196ec82b52e502c18e4a34478bf7b9f61c83c2ebaa95c74d8ded45a95da9c'
expected = json.loads((package / 'sources.json').read_text())
receipts = json.loads((package / 'receipts.json').read_text())
with tempfile.TemporaryDirectory(prefix='sdocx-arc-rotation-') as directory:
    tmp = Path(directory)
    (tmp / 'conformance').mkdir()
    shutil.copy(root / 'conformance/native_table.rs', tmp / 'conformance/native_table.rs')
    shutil.copytree(root / 'conformance/native_table', tmp / 'conformance/native_table', symlinks=True)
    shutil.copytree(root / 'tools', tmp / 'tools', symlinks=True, copy_function=os.symlink)
    (tmp / 'tools/named_arc_drawing/capture.rs').unlink()
    shutil.copy(root / 'tools/named_arc_cap_join/capture.rs', tmp / 'tools/named_arc_drawing/capture.rs')
    subprocess.run(['patch', '-s', str(tmp / 'tools/named_arc_drawing/capture.rs'), str(package / 'capture.patch')], check=True)
    cap=tmp / 'tools/named_arc_drawing/capture.rs'
    assert hashlib.sha256(cap.read_bytes()).hexdigest() == expected['tools/named_arc_drawing/capture.rs']
    code=cap.read_text().replace('pub(crate) fn capture_named(', 'static mut ROT_INPUT: [u64;6]=[0;6];\nunsafe extern "C" fn trace_steps(e: Engine, a:u64, _:u32, _: *mut c_void) { if [0x2246cc,0x2246dc,0x224700,0x224718,0x2247a0].contains(&a) { eprintln!("STEP {a:x} {:?}", (0..16).map(|i| f32::from_bits(read_register(e,136+i) as u32)).collect::<Vec<_>>()); } if a==0x40b0f10 { let p=read_register(e,REGISTER_X0); unsafe {ROT_INPUT=[read_u32(e,p) as u64,read_u32(e,p+4) as u64,read_register(e,136),read_register(e,137),read_register(e,138),read_register(e,REGISTER_X30)];} } if a==0x40b0f9c { let x=read_register(e,136) as u32; let y=read_register(e,137) as u32; if [1103247762,1108411596,1104466756,1127546406].contains(&y) || [1127546406].contains(&x) {eprintln!("RETURN {x} {y} input {:?}",unsafe {ROT_INPUT});} } if a==0x2246dc {let q=read_u64(e,read_register(e,REGISTER_X0+19)+16); if q!=0 {eprintln!("OLDANGLES {} {}",f32::from_bits(read_u32(e,q)),f32::from_bits(read_u32(e,q+4)));}} if a==0x211670 { eprintln!("ANGLECALL lr {:x} {:?}",read_register(e,REGISTER_X30),(0..4).map(|i|f32::from_bits(read_register(e,136+i) as u32)).collect::<Vec<_>>());} if a==0x20c6f0 {eprintln!("ISSAME {}",read_register(e,REGISTER_X0));} if a==0x20ca88 { let p=read_register(e,REGISTER_X0+20); let x=read_u32(e,p); let y=read_u32(e,p+4); if x==1120409959 || x==1108150607 {eprintln!("ROT in {x} {y} out {} {} angle {}",read_register(e,136),read_register(e,137),f32::from_bits(read_register(e,144) as u32));} } if a==0x2244e0 { let sp=read_register(e,REGISTER_SP); eprintln!("RAW {:?}",(0..26).map(|i| read_u32(e,sp+4*i)).collect::<Vec<_>>()); } }\npub(crate) fn capture_named(')
    code=code.replace('    let target = machine.call(0x36d6cc', '    let mut step_hook=0; check(unsafe {uc_hook_add(machine.engine,&mut step_hook,4,trace_steps as *mut c_void,ptr::null_mut::<c_void>(),0x2244e0,0x2247a0)});\n    let mut rot_hook=0; check(unsafe {uc_hook_add(machine.engine,&mut rot_hook,4,trace_steps as *mut c_void,ptr::null_mut::<c_void>(),0x20ca88,0x20ca88)});\n    let mut ret_hook=0; check(unsafe {uc_hook_add(machine.engine,&mut ret_hook,4,trace_steps as *mut c_void,ptr::null_mut::<c_void>(),0x40b0f10,0x40b0f9c)});\n    let mut same_hook=0; check(unsafe {uc_hook_add(machine.engine,&mut same_hook,4,trace_steps as *mut c_void,ptr::null_mut::<c_void>(),0x20c6f0,0x20c6f0)});\n    let mut angle_hook=0; check(unsafe {uc_hook_add(machine.engine,&mut angle_hook,4,trace_steps as *mut c_void,ptr::null_mut::<c_void>(),0x211670,0x211670)});\n    let target = machine.call(0x36d6cc')
    code=code.replace('([10.0f32,20.0,210.0,120.0],0xffff0000)', '([10.0f32,20.0,210.0,120.0].map(|x| x * std::env::var("SCALE").unwrap().parse::<f32>().unwrap()),0xffff0000)')
    code=code.replace('unwrap_or_else(|_| "100".into()).parse().unwrap()', 'unwrap_or_else(|_| "100".into()).parse::<f32>().unwrap() * std::env::var("SCALE").unwrap().parse::<f32>().unwrap()')
    code=code.replace('unwrap_or_else(|_| "20".into()).parse().unwrap()', 'unwrap_or_else(|_| "20".into()).parse::<f32>().unwrap() * std::env::var("SCALE").unwrap().parse::<f32>().unwrap()')
    code=code.replace('unwrap_or_else(|_| "40".into()).parse().unwrap()', 'unwrap_or_else(|_| "40".into()).parse::<f32>().unwrap() * std::env::var("SCALE").unwrap().parse::<f32>().unwrap()')
    cap.write_text(code)
    (tmp / 'libunicorn.so').symlink_to(unicorn / 'libunicorn.so.2')
    subprocess.run(['rustc', '--edition=2024', '--emit=dep-info', 'conformance/native_table.rs', '-o', 'sources.d'], cwd=tmp, check=True)
    names = (tmp / 'sources.d').read_text().splitlines()[0].split(': ', 1)[1].split()
    assert {os.path.normpath(n) for n in names} == set(expected)
    for name, digest in expected.items():
        if name != 'tools/named_arc_drawing/capture.rs':
            assert hashlib.sha256((tmp / name).read_bytes()).hexdigest() == digest, name
    subprocess.run(['rustc', '--edition=2024', 'conformance/native_table.rs', '-L', str(tmp), '-o', str(tmp / 'probe')], cwd=tmp, check=True)
    for case in ['37']:
        for fill in [0,85,165,255,0]:
            env = os.environ | {'LD_LIBRARY_PATH': str(unicorn), 'ARC_OUTPUT': str(tmp / 'frame'), 'ARC_SECOND': '1', 'ARC_FILL': str(fill), 'ANGLE': '37' if case == 'alt' else case, 'SCALE': os.environ['SCALE']}
            if case == 'alt':
                env |= {'ARC_MOVED': '20', 'ARC_MOVED_Y': '30', 'ARC_END_X': '40', 'ARC_END_Y': '100'}
            run = subprocess.run([str(tmp / 'probe'), str(lib / 'libSPenModel.so'), '--named-arc-drawing', str(lib / 'libSPenBase.so'), str(lib / 'libSPenSkia.so')], env=env, capture_output=True, check=True)
            prefixes = ('FRAMED_SIZE', 'CONTROL ', 'STEP ', 'RAW ', 'ROT ', 'RETURN ', 'ISSAME ', 'OLDANGLES ', 'ANGLECALL ', 'SOURCE ', 'ACTIVE ', 'canvas 0 points', 'canvas 1 points', 'LEDGER ', 'HOST_CALLS ', 'reached plt ')
            print('CASE',case)
            receipt = '\n'.join(line for line in run.stderr.decode().splitlines() if line.startswith(prefixes)) + '\n'
            selected=('CONTROL ', 'ISSAME ', 'OLDANGLES ', 'ANGLECALL ', 'SOURCE ', 'ACTIVE ', 'canvas 0 points', 'canvas 1 points')
            compact=''.join(line+'\n' for line in run.stderr.decode().splitlines() if line.startswith(selected))
            receipt_digest={'0.5':'68df5a2c4dcee8df76d171fce0afc734899faf3d25de458ac9998f4c27cdbdf7','2':'fd6334aec0b9c80d43f4d8be09d1afc939e9ad54f6d3d305b7ecc47b357d64eb'}[os.environ['SCALE']]
            frame_digest={'0.5':'eca93f55caef815d34873c13d6be58baf56e09dd6dce68ac4bffb0ece3af92bb','2':'9268308a73173b6c378d1e2531139e8d81fc449a696c84919291d08cc7348817'}[os.environ['SCALE']]
            assert hashlib.sha256(compact.encode()).hexdigest()==receipt_digest, fill
            assert hashlib.sha256((tmp / 'frame').read_bytes()).hexdigest()==frame_digest, fill
            b=(tmp / 'frame').read_bytes(); n=0
            for _ in range(3): n+=int.from_bytes(b[n:n+4],'little')
            assert b[:n]==Path(os.environ['SCALE_FIXTURE']).read_bytes()
            pass
            if case == '37':
                pass
print('Rotated Arc native roundtrip and Drawing receipts match across allocation fills.')

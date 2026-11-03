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
    code=code.replace('    let target = machine.call(0x36d6cc', '    let offset=std::env::var("ALTER_OFFSET").unwrap().parse::<u64>().unwrap(); let x=f64::from_bits(read_u64(machine.engine,input+offset)); write(machine.engine,input+offset,&(x+std::env::var("DELTA").unwrap().parse::<f64>().unwrap()).to_bits().to_le_bytes()); std::fs::write(std::env::var("ALTER_FRAME").unwrap(), bytes(machine.engine,input,604)).unwrap();\n    let target = machine.call(0x36d6cc')
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
        for fill in [0, 85, 165, 255, 0]:
            env = os.environ | {'LD_LIBRARY_PATH': str(unicorn), 'ARC_OUTPUT': str(tmp / 'frame'), 'ARC_SECOND': '1', 'ARC_FILL': str(fill), 'ANGLE': '37' if case == 'alt' else case, 'ALTER_OFFSET': os.environ['ALTER_OFFSET'], 'ALTER_FRAME': os.environ['ALTER_FRAME'], 'DELTA': os.environ.get('DELTA', '10')}
            if case == 'alt':
                env |= {'ARC_MOVED': '20', 'ARC_MOVED_Y': '30', 'ARC_END_X': '40', 'ARC_END_Y': '100'}
            run = subprocess.run([str(tmp / 'probe'), str(lib / 'libSPenModel.so'), '--named-arc-drawing', str(lib / 'libSPenBase.so'), str(lib / 'libSPenSkia.so')], env=env, capture_output=True, check=True)
            prefixes = ('FRAMED_SIZE', 'CONTROL ', 'STEP ', 'RAW ', 'ROT ', 'RETURN ', 'ISSAME ', 'OLDANGLES ', 'ANGLECALL ', 'SOURCE ', 'ACTIVE ', 'canvas 0 points', 'canvas 1 points', 'LEDGER ', 'HOST_CALLS ', 'reached plt ')
            print('CASE',case)
            receipt = '\n'.join(line for line in run.stderr.decode().splitlines() if line.startswith(prefixes)) + '\n'
            selected = ('CONTROL ', 'ISSAME ', 'OLDANGLES ', 'ANGLECALL ', 'SOURCE ', 'ACTIVE ', 'canvas 0 points', 'canvas 1 points')
            compact = ''.join(line + '\n' for line in run.stderr.decode().splitlines() if line.startswith(selected))
            digest = hashlib.sha256(compact.encode()).hexdigest()
            expected_receipt = {('501','10'): '981a976319e1d08a37c09ed430348a6cf0167e1639330a45f87e2fa0d58b4441', ('517','10'): '483d17a20db7240290a44c857be8e2f075a4ac749e8928e9ce4286d0f0cbffd1', ('501','0.00006'): '16637426743b0d875099289dffa9bd29fffca5b83877da468e5861210af0aa59', ('501','0.0001'): 'e87a6b318287226ebb93cac557ebcc104b2a10ce75e65715c886732f9d0ef2fa', ('517','0.0001'): '4d7e7fe2152a4008fdb6cf19527e7c0e289c43aef8ba6b096d130d934eacdf0f', ('517','0.00015'): '4055df0c9aa570a8c2e6e0ac1b15eb88ecc9b0198f4e8561e33c3ecf1745dcb3'}[(os.environ['ALTER_OFFSET'], os.environ.get('DELTA', '10'))]
            assert digest == expected_receipt, (fill, digest)
            expected_frame = {('501','10'): '7cd201aefd1b0d0db8b0915f39bd06a439b6d328926c74b2a0979c6cbcc27bed', ('517','10'): '9e250f4da5b6b36b64d0dc2d9696c2a287c8ab5ef2badafd7432b4fcf23e09e5', ('501','0.00006'): '761f8e51da274d8f3f33ff6e7b14a5381bf1eaf3569dbff7dad29bcce4f0ce05', ('501','0.0001'): 'a0c941845b5a53e2888627d7ba9ec7c7631e0dc5650d323cd4d24c64b4d26f66', ('517','0.0001'): 'a959c0bb8c77c198ad556c8b6e8ac259ceea2ff8d06a3b1178d583b3196af514', ('517','0.00015'): 'ad29f2e5fbdd3ef8214364c7a66fef5e328fb14c90dc1b4f21644216df5c79af'}[(os.environ['ALTER_OFFSET'], os.environ.get('DELTA', '10'))]
            assert hashlib.sha256(Path(os.environ['ALTER_FRAME']).read_bytes()).hexdigest() == expected_frame
            assert hashlib.sha256((tmp / 'frame').read_bytes()).hexdigest() == receipts[case]['frame'], (case, fill)
            if case == '37':
                pass
print('Rotated Arc native roundtrip and Drawing receipts match across allocation fills.')

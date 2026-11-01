"""Hash ordered Rust-parsed handwritten stroke UUIDs without retaining identifiers."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
ROOT=Path(__file__).resolve().parents[2]
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
    archive=ROOT/'tmp/stroke-conformance/handwritten.sdocx'
    source=ROOT/'crates/sdocx/examples/saved_ink_inputs.rs'
    if sha(archive)!='77e3997a066afa0333d0f5020bb428efffeb783c741bc956ae596292e2d5cda3' or sha(source)!='e4494fc13e18c131e6eaec55c04108fe7fa1d0ed1b8a8ceefb4edbc20b02525c':raise RuntimeError('input mismatch')
    env=dict(os.environ,CARGO_TARGET_DIR='/tmp/sdocx-agent-target',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_BUILD_JOBS='2')
    result=subprocess.run(['cargo','run','--offline','--quiet','-p','sdocx','--features','serde','--example','saved_ink_inputs','--',str(archive)],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=False)
    if result.returncode:raise RuntimeError('parser failed')
    data=json.loads(result.stdout);items=[hashlib.sha256(row['rendering']['metadata']['uuid'].encode('ascii')).hexdigest() for row in data['strokes']]
    value=hashlib.sha256(''.join(items).encode('ascii')).hexdigest()
    if data['document_sha256']!='77e3997a066afa0333d0f5020bb428efffeb783c741bc956ae596292e2d5cda3' or len(items)!=2769 or len(set(items))!=2769 or value!='963f09365784936859772c332222fd69d25cd71011f2c950349f57c25cc4d567' or sha(archive)!=data['document_sha256']:raise RuntimeError('identity mismatch')
    print(json.dumps({'archive_sha256':data['document_sha256'],'count':len(items),'distinct':len(set(items)),'ordered_uuid_hashes_sha256':value}))
try:main()
except Exception:
    sys.stderr.write('Rust identity probe failed\n')
    raise SystemExit(1) from None

"""Reproduce one saved-object native/vector comparison in a private temporary directory."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

if not __debug__:
    raise RuntimeError('optimized Python is unsupported')
ROOT=Path(__file__).resolve().parents[2]
HERE=Path(__file__).resolve().parent
INPUT=ROOT/'hf/04-marker4-highlighter.sdocx'
ARCHIVE='8299abbf763e8e3e5af746815d13dfdf45443da27b0eb79192c328571905cdd2'
UUID='3d1819b137be66c5d3955b112bceb22a8f46c48d7d3960e43118c15b478c40c5'
PINS={'wnote_small_vector_capture.py':'279138f18346f1cb51ac52bc570e6b8de1e55900598250d690e9a28c4f63fc90','wnote_small_vector_export.rs':'994b8793b4379a04dcbec58e844e765c6ff42372c2cd34bc101a2ed6776035d5','wnote_small_vector_compare.py':'fbba21ac9692113a0d292351390cdba62e6bd3da71e35d8803de545c87c71ddd'}
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def run(args,env):
    r=subprocess.run(args,cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=False)
    if r.returncode:raise RuntimeError('child process failed')
    return r.stdout

def main():
    for name,expected in PINS.items():
        if digest(HERE/name)!=expected:raise RuntimeError('adapter pin mismatch')
    if digest(INPUT)!=ARCHIVE:raise RuntimeError('archive pin mismatch')
    rust_source=ROOT/'crates/sdocx/examples/saved_ink_inputs.rs'
    if digest(rust_source)!='e4494fc13e18c131e6eaec55c04108fe7fa1d0ed1b8a8ceefb4edbc20b02525c':raise RuntimeError('Rust source pin mismatch')
    resvg=Path(shutil.which('resvg') or '')
    if not resvg.is_file() or digest(resvg)!='3e9eaf857346d730761a7c58412c923859525d5f498f9badc1851abe0f540611':raise RuntimeError('resvg pin mismatch')
    env=dict(os.environ,CARGO_TARGET_DIR='/tmp/sdocx-agent-target',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_BUILD_JOBS='2')
    version=run([str(resvg),'--version'],env).decode().strip()
    if version!='0.45.1':raise RuntimeError('resvg version mismatch')
    rust=json.loads(run(['cargo','run','--offline','--quiet','-p','sdocx','--features','serde','--example','saved_ink_inputs','--',str(INPUT),'0'],env))
    row=rust['strokes'][0];style=row['rendering']['style'];metadata=row['rendering']['metadata']
    if rust['document_sha256']!=ARCHIVE or len(rust['strokes'])!=1 or row['index']!=0 or row['page_index']!=0 or row['point_count']!=121 or hashlib.sha256(metadata['uuid'].encode('ascii')).hexdigest()!=UUID or style['color_argb']!=4278387459 or row['pen_width']!=9.116800308227539 or row['rendering']['pen_name']!='com.samsung.android.sdk.pen.pen.preload.FountainPen' or style['advanced_pen_setting_id']!=1:raise RuntimeError('Rust object mismatch')
    with tempfile.TemporaryDirectory(prefix='sdocx-vector-',dir='/tmp') as tmp:
        d=Path(tmp);env['SDOCX_VECTOR_TMP']=tmp
        native_output=run([sys.executable,str(HERE/'wnote_small_vector_capture.py')],env).decode()
        native=json.loads(native_output[native_output.index('{'):]);draw=native['drawing']
        if not any(row.get('step')=='ObjectBase::GetType' and row.get('result')=='0x1' for row in native['steps']):raise RuntimeError('native object mismatch')
        if native['inputs']['archive_sha256']!=ARCHIVE or draw['uuid_sha256']!=UUID or draw['color_argb']!=4278387459 or draw['pen_size_bits']!=1091690090 or draw['matrix']!=[1,0,0,0,-1,0,-80,401,1] or draw['rtv5_addpoint_entries']!=59 or draw['gl_error']!=0 or draw['draw']!={'mode':5,'first':0,'count':4,'instances':59,'framebuffer_binding':draw['target_framebuffer']}:raise RuntimeError('native object mismatch')
        crate=d/'crate';(crate/'src').mkdir(parents=True)
        (crate/'Cargo.toml').write_text('[package]\nname="wnote-vector-probe"\nversion="0.1.0"\nedition="2024"\n[dependencies]\nsdocx={path="'+str(ROOT/'crates/sdocx')+'",features=["pdf"]}\nsha2="0.10.9"\n')
        (crate/'src/main.rs').write_bytes((HERE/'wnote_small_vector_export.rs').read_bytes())
        run(['cargo','run','--offline','--quiet','--manifest-path',str(crate/'Cargo.toml'),'--',str(INPUT),tmp],env)
        run([str(resvg),str(d/'selected.svg'),str(d/'selected.png')],env)
        metrics=json.loads(run([sys.executable,str(HERE/'wnote_small_vector_compare.py')],env))
        if metrics['native_topdown_sha256']!='ae15b708ab2af0903185ccf3c1f11734ff4bf8becd5a5004ae5b95039076f26a' or metrics['baseline_topdown_sha256']!='cac3c81f59e3c2e65f7e22096f743f66d65fdd5d36e5b91443c50f84794979dd':raise RuntimeError('native frame mismatch')
        if metrics['readers']!={'pillow':'12.1.1','mupdf':'1.27.2.3','pdfium':'150.0.7869.0'}:raise RuntimeError('reader version mismatch')
        expected={'svg_resvg':'8c544ab7f8c6720ca03a1a325ccd950a290ae04c74dcdebb38dd96910492dfb6','svg_pdf_mupdf':'c256a9f0765f6b4aa73ffd07ef0874833b75a3619fe93170704be07f149af5c0','svg_pdf_pdfium':'157544bd90d38c73201ec881a5bd47f3c26c8bff68ebf05405acd134c516cdab','direct_pdf_mupdf':'6e2b0d83999385253cd4ad92a5238225cd1a1da196b995080e2a42b5b6928352','direct_pdf_pdfium':'18c090cfb0bedd8913066fc3b709808c95bc4907cbe3931ed515a7c379c3c24f'}
        if {k:v['sha256'] for k,v in metrics['results'].items()}!=expected:raise RuntimeError('public output mismatch')
        if digest(INPUT)!=ARCHIVE:raise RuntimeError('archive changed')
        print(json.dumps(metrics,indent=2))

try:main()
except Exception as exc:
    known={'adapter pin mismatch','archive pin mismatch','Rust source pin mismatch','resvg pin mismatch','resvg version mismatch','child process failed','Rust object mismatch','native object mismatch','native frame mismatch','reader version mismatch','public output mismatch','archive changed'}
    raise RuntimeError(str(exc) if type(exc) is RuntimeError and str(exc) in known else 'comparison failed: '+type(exc).__name__) from None

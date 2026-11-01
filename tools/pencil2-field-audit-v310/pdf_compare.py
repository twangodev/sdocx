"""Replay v308 full PDFs with a single coalescing switch; no output artifacts."""
from fixtures import verify
verify()
import contextlib, hashlib, io, os, re, sys
from pathlib import Path
root=Path(__file__).resolve().parents[2]
path=root/'tools/pencil2-mesh-v308/scene.py'
source=path.read_text()
assert hashlib.sha256(source.encode()).hexdigest()=='3fb9251787c81dbba0d1ce8b2728f0f0f4c2d6af38a6db607f1501c8c4333447'
needle='if runs and runs[-1][2]==grain:'
assert source.count(needle)==1
unmerged=os.environ.get('PENCIL_UNMERGED')=='1'
if unmerged: source=source.replace(needle,'if False:')
# v308's bound deliberately excludes this coalescing-disabled experiment.
assert source.count('8*1024*1024 and triangles<1_500_000')==1
source=source.replace('8*1024*1024 and triangles<1_500_000','16*1024*1024 and triangles<3_000_000')
sys.argv=[str(path),'--full']
buf=io.StringIO()
with contextlib.redirect_stdout(buf):exec(compile(source,str(path),'exec'),{'__name__':'__main__','__file__':str(path)})
out=buf.getvalue();print(out,end='')
triples=[tuple(map(int,m)) for m in re.findall(r'(?:MuPDF|PDFium) sum (\d+) max (\d+) changed (\d+)',out)]
expected=([(13299,77,1196),(14868,78,1234),(44931,94,4236),(47652,95,4352),(43963,66,4351),(46448,66,4409)] if unmerged else [(12947,72,1182),(14264,71,1239),(43676,92,4230),(45620,93,4361),(43579,66,4341),(45357,66,4399)])
assert triples==expected,(triples,expected)
counts=[int(x) for x in re.findall(r'triangles (\d+) bytes',out)]
assert counts==([2596148,2596136,2596136] if unmerged else [360670,360672,360672]),counts

"""Check replay fixtures, with opt-in workspace and local recapture checks."""
import argparse,hashlib,json,sys
from pathlib import Path
parser=argparse.ArgumentParser()
parser.add_argument('--workspace',action='store_true',help='check pinned archived crate sources as well')
parser.add_argument('--native-local',action='store_true',help='check optional local native recapture adapters')
args=parser.parse_args()
root=Path(__file__).resolve().parents[2]
manifest=json.loads((root/'tools/pencil2-mesh-v309/input-manifest.json').read_text())
expected=manifest['sha256'];native=set(manifest['native_local_paths'])
selected=set(expected)-native
if args.native_local:selected|=native
errors=[]
if args.workspace:
 actual={str(p.relative_to(root)) for p in (root/'crates/sdocx').rglob('*') if p.is_file() and ('src' in p.relative_to(root/'crates/sdocx').parts or p.name in ('Cargo.toml','build.rs'))}
 digest=hashlib.sha256(''.join(f'{path}\0{hashlib.sha256((root/path).read_bytes()).hexdigest()}\n' for path in sorted(actual)).encode()).hexdigest()
 snapshot=manifest['crate_snapshot']
 if len(actual)!=snapshot['file_count'] or digest!=snapshot['sha256']:errors.append('changed crate snapshot: file count or aggregate SHA-256 differs')
for path in sorted(selected):
 p=root/path
 if not p.is_file():errors.append(f'missing: {path}');continue
 if hashlib.sha256(p.read_bytes()).hexdigest()!=expected[path]:errors.append(f'changed: {path}')
if errors:
 print('\n'.join(errors),file=sys.stderr);sys.exit(1)
print(f'Verified {len(selected)} workspace inputs; crate snapshot={args.workspace}, local recapture={args.native_local}')

"""Compare all clean replay error triples with the compact receipt."""
import hashlib,json,re,sys
from pathlib import Path
root=Path(__file__).resolve().parents[2]
receipt=json.loads((root/'tools/pencil2-mesh-v309/receipt.json').read_text())
work=Path(sys.argv[1])
for name,expected in receipt['rust_descriptor_sha256'].items():
 actual=hashlib.sha256((work/'geometry'/f'{name}.bin').read_bytes()).hexdigest()
 if actual!=expected:raise SystemExit(f'{name} descriptor hash differs: {actual}')
 print(f'{name} descriptor hash matches')
def found(pattern,text,label):
 rows=re.findall(pattern,text,flags=re.MULTILINE)
 if len(rows)!=1:raise SystemExit(f'{label}: expected one result, found {len(rows)}')
 return [int(x) for x in rows[0]]
def check(label,actual,expected):
 if actual!=expected:raise SystemExit(f'{label}: expected {expected}, got {actual}')
 print(f'{label}: {actual} matches')
text=(work/'synthetic.log').read_text()
for case,expected in receipt['synthetic_full_scene'].items():
 backdrop,scale=case.split('-')
 pattern=rf'^{backdrop} {scale} triangles \d+ bytes \d+\nMuPDF (\d+) (\d+) (\d+)\nPDFium (\d+) (\d+) (\d+)$'
 values=found(pattern,text,case)
 check(case+' MuPDF',values[:3],expected['mesh_pdf_mupdf'])
 check(case+' PDFium',values[3:],expected['mesh_pdf_pdfium'])
for case,expected in receipt['note_full_scene'].items():
 text=(work/f'{case}-mesh.log').read_text()
 for reader,key in [('MuPDF','mesh_pdf_mupdf'),('PDFium','mesh_pdf_pdfium'),('SVG resvg','mesh_svg_resvg')]:
  pattern=rf'^{re.escape(reader)} sum (\d+) max (\d+) changed (\d+)(?: bytes \d+)?$'
  check(case+' '+key,found(pattern,text,key),expected[key])
 text=(work/f'{case}-generic.log').read_text()
 for reader,key in [('SVG resvg','generic_svg_resvg'),('PDF MuPDF','generic_pdf_mupdf'),('PDF PDFium','generic_pdf_pdfium')]:
  pattern=rf'^{re.escape(reader)} (\d+) (\d+) (\d+)$'
  check(case+' '+key,found(pattern,text,key),expected[key])

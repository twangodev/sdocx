"""Pin retained compressed native frames and their capture receipts."""
import hashlib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
HASHES={
 'white-1-first.zlib':'21efba6f7275f8b08d621c877bea2f87532aeb6d680b7f12a19dcfcc0f0ab03c',
 'white-1-second.zlib':'38b2c1b537fcd786929c15450707dadd26198b1510071ecfa14aadaabcc32fa0',
 'white-1-receipt.json':'22a7d7db5481e6d52b299cc2cd4f237b6e8d0db1dfc9a0a43238376ef4f7bb99',
 'white-2-first.zlib':'54ae207f9c76c3129005f3a9f1896654577eb178f1afb27ec16896ddfccf505c',
 'white-2-second.zlib':'68dbb04d0c3284987fec93fb3580c70df57a64e009250a12c36974057b5b4ceb',
 'white-2-receipt.json':'9c25ee538af27d8f06359227badb75bfd6ff220afb79dc846e03b76bcf0b8218',
 'blue-2-first.zlib':'c39291f64a4b7600b25179791af0745888269d24590361b72d34ff86a8abdc8a',
 'blue-2-second.zlib':'742515f1c457bd60a6113cac8935381041945752d28b8ca824c55a07cba61c47',
 'blue-2-receipt.json':'be2718978c4eca6f9fc97df5e7d8565550e9c291a6d8d255b25f32dc99a5edad',
 'white-1-second.vbo':'ab8c231108800d1a9437783f27d12c57e59c4d81a00538268343f662a1c64389',
}
def verify():
 for name,expected in HASHES.items():
  actual=hashlib.sha256((ROOT/'tools/pencil2-mixed-scene-controls'/name).read_bytes()).hexdigest()
  assert actual==expected,(name,actual,expected)

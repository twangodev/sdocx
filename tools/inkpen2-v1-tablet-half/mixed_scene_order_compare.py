"""Verify retained native mixed-scene order frame hashes without public exports."""
import hashlib
import pathlib
import zlib

p = pathlib.Path(__file__).resolve().parent
a=zlib.decompress((p/'mixed-scene-order-final.rgba.zlib').read_bytes())
b=zlib.decompress((p/'mixed-scene-order-before.rgba.zlib').read_bytes())
mid=zlib.decompress((p/'mixed-scene-order-line.rgba.zlib').read_bytes())
assert hashlib.sha256(mid).hexdigest()=='5dcb9735e677eafac26f0c4a03c1fab2e2d0f29b602ea98fe2f2da7ccac5bf8a'
assert len(a)==len(b)==len(mid)==64*48*4
assert hashlib.sha256(a).hexdigest()=='77fb3c431261e44d9608c744fe2a09caf0267bb80dda26bd40725d721b7d2234'
assert hashlib.sha256(b).hexdigest()=='ef2d94b172caedab5d4047969a2e74fc71c95a8351e718c1c62cf6d22020576a'
print('Verified native before, line, and final frame hashes')

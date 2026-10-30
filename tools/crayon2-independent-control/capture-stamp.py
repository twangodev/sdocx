#!/usr/bin/env python3
"""Capture independent native control with one selected native vector record."""
import argparse
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('index', type=int, choices=range(479))
index = parser.parse_args().index
wrapper = Path(__file__).with_name('capture.py').resolve()
source = wrapper.read_text()
anchor = '# Keep the native texture-scale uniform alongside the paint calls.'
assert source.count(anchor) == 1
patch = '''anchor = "assert len(vector_data)%24==0 and len(vector_data)>0"
assert source.count(anchor) == 1
source = source.replace(anchor, anchor + {injection!r})
'''.format(injection=f'\nvector_data=vector_data[{index * 24}:{(index + 1) * 24}]\np.u.mem_write(vecbegin,vector_data)\np.put(vecobj+8,"Q",vecbegin+24)')
source = source.replace(anchor, patch + anchor)
exec(compile(source, str(wrapper), 'exec'), {'__name__': '__main__', '__file__': str(wrapper)})

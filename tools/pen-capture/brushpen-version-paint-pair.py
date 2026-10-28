"""Run the retained synthetic Drawing-to-Mesa probe with setting 10; or 11;.

Requires the project-local ignored probe, native libraries and Unicorn runtime.
Use the environment in brushpen-version-paint-pair.json; stdout is the full trace.
"""
import argparse
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('version', choices=('10', '11'))
args = parser.parse_args()
probe = Path('tmp/vector-parity-plan/brushpen-same-drawing-queue/second_mapdraw.py')
source = probe.read_text()
needle = "self.u.mem_write(c,b'\\0');self.call('SPen::String::Set(char const*)',[adv,c])"
assert source.count(needle) == 1
replacement = f"self.u.mem_write(c,b'{args.version};\\0');self.call('SPen::String::Set(char const*)',[adv,c])"
exec(compile(source.replace(needle, replacement), str(probe), 'exec'), {'__file__': str(probe), '__name__': '__main__'})

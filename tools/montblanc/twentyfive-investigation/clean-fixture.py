#!/usr/bin/env python3
import gzip
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])
evidence = Path(__file__).resolve().parent
stderr = gzip.decompress((evidence / 'native-stream.stderr.gz').read_bytes()).decode()
stream = json.loads(next(line.partition('=')[2] for line in stderr.splitlines() if line.startswith('CAPTURE_STREAM=')))
points = [[x, y] for x, y in zip(range(0, 100, 4), [0, 2, 4, 6, 8, 6, 4, 2, 0, -2, -4, -6, -8, -6, -4, -2, 0, 2, 4, 6, 8, 6, 4, 2, 0])]
case = {'case': {'input': {'points': points, 'pressure': [0.5] * 25, 'pen_size': 1}, 'result': {'stream': stream}}}
(root / 'crates/sdocx/src/ink/montblanc_25_stream.json').write_text(json.dumps(case))

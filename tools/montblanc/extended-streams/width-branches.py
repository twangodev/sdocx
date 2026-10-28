#!/usr/bin/env python3
"""Capture native width-branch streams and check the disassembled repeat model."""
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys

root = Path(__file__).resolve().parents[3]
capture = root / 'tools/montblanc/extended-streams/capture.py'
trace = root / 'tools/montblanc/extended-streams/trace.py'
widths = [2, 2.01, 3, 5, 10, 15, 20, 30, 40, 55, 80, 81]

def f32(x):
    return struct.unpack('<f', struct.pack('<f', x))[0]

def model(length, width):
    if width <= 2: scaled = length
    elif width <= 3: scaled = f32(length / 1.5)
    elif width <= 5: scaled = f32(length * 0.5)
    elif width <= 10: scaled = f32(length / 3)
    elif width <= 15: scaled = f32(length * 0.25)
    elif width <= 20: scaled = f32(length / 5)
    elif width <= 30: scaled = f32(length / 6)
    elif width <= 40: scaled = f32(length / 7)
    elif width <= 55: scaled = f32(length * 0.125)
    elif width <= 80: scaled = f32(length / 9)
    else: scaled = f32(length / 10)
    return int(f32(f32(max(int(scaled), 2) + 1) * f32(1.2)))

def run(width):
    case = {'points': [[0, 0], [10, 0], [20, 2]], 'pressure': [0.3, 0.6, 1], 'pen_size': width}
    env = os.environ.copy()
    env['PYTHONPATH'] = str(root / 'scratch/apk-analysis-runtime/python')
    env['MB_CASE'] = json.dumps(case)
    def invoke(script):
        p = subprocess.run([sys.executable, str(script)], cwd=root, env=env, text=True, capture_output=True, check=True)
        return json.loads(p.stdout.splitlines()[-1])
    t = invoke(trace)
    repeat = next(e for e in t['events'] if 'getRepeatEff(' in e['symbol'])
    length = struct.unpack('<f', bytes.fromhex(repeat['s0_s3_f32_bits'][0]))[0]
    count = repeat['result_x0']
    result = invoke(capture)
    assert count == model(length, f32(width)) == result['count'], (width, count, length)
    return str(width), {'input': case, 'length': length, 'repeat': count, 'stream': result['stream']}

with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
    cases = dict(pool.map(run, widths))
result = {'dso_sha256': hashlib.sha256((root / 'scratch/apk-analysis-native/arm64-v8a/libSPenMontblancFountainPen.so').read_bytes()).hexdigest(), 'capture_sha256': hashlib.sha256(capture.read_bytes()).hexdigest(), 'trace_sha256': hashlib.sha256(trace.read_bytes()).hexdigest(), 'cases': cases}
print(json.dumps(result, indent=2))

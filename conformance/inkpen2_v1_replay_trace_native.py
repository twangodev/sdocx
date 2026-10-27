"""Run the pinned saved-caller harness with an explicit synthetic JSON input."""
from pathlib import Path
import hashlib
import json

source = Path(__file__).resolve().parents[1] / 'scratch/straightinkpen2-alias/inkpen2_saved_caller_native.py'
controls = Path(__file__).resolve().parent / 'inkpen2-null-v1-controls-native.json'
assert hashlib.sha256(source.read_bytes()).hexdigest() == json.loads(controls.read_text())['source_harness_sha256']
code = source.read_text()
code = code.replace("parser.add_argument('--pen-name', choices=", "parser.add_argument('--input-json', required=True)\nparser.add_argument('--pen-name', choices=")
code = code.replace('input_case=INPUT_CASES[args.case]', "input_case=json.loads(pathlib.Path(args.input_json).read_text())")
code = code.replace("'stroke_type':args.stroke_type", "'stroke_type':input_case['stroke_type']")
code = code.replace("record={'signatures':method_trace_names_by_pc[a],'phase':phase,", "record={'callback_count': len(inkpen2_buffer_events), 'signatures':method_trace_names_by_pc[a],'phase':phase,")
code = code.replace("('::RedrawPen(','::endPen(','::drawPoint(')", "('::RedrawPen(','::startPen(','::drawLine(','::endPen(','::drawPoint(')")
code = code.replace("('signatures','phase','pc','regs','motion_event_entry_bytes','pointer_heads')", "('signatures','phase','pc','regs','motion_event_entry_bytes','pointer_heads','callback_count')")
exec(compile(code, str(source), 'exec'), {'__name__': '__main__', '__file__': str(source)})

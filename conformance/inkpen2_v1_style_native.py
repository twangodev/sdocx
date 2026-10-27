"""Run the pinned Drawing caller with supplied synthetic V1 style fields."""
from pathlib import Path
source = Path(__file__).resolve().parents[1] / 'scratch/straightinkpen2-alias/inkpen2_saved_caller_native.py'
code = source.read_text()
code = code.replace("parser.add_argument('--pen-name', choices=", "parser.add_argument('--input-json', required=True)\nparser.add_argument('--pen-name', choices=")
code = code.replace('input_case=INPUT_CASES[args.case]', "input_case=json.loads(pathlib.Path(args.input_json).read_text())")
code = code.replace("'stroke_type':args.stroke_type", "'stroke_type':input_case['stroke_type']")
code = code.replace("if args.case.endswith('_color'):", "if 'color_argb' in input_case:", 1)
code = code.replace('source_color_argb=0xff336699;', "source_color_argb=input_case['color_argb'];", 1)
# The original color assertion expects V9's nine-float records; V1 has three.
code = code.replace(" if source_color_argb is not None:\n  rgb=tuple", " if source_color_argb is not None and selected_drawable_version == '9':\n  rgb=tuple", 1)
exec(compile(code, str(source), 'exec'), {'__name__': '__main__', '__file__': str(source)})

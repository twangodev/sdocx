"""Run the pinned saved-caller harness with an explicit synthetic JSON input."""
from pathlib import Path

source = Path(__file__).resolve().parents[1] / 'scratch/straightinkpen2-alias/inkpen2_saved_caller_native.py'
code = source.read_text()
code = code.replace("parser.add_argument('--pen-name', choices=", "parser.add_argument('--input-json', required=True)\nparser.add_argument('--pen-name', choices=")
code = code.replace('input_case=INPUT_CASES[args.case]', "input_case=json.loads(pathlib.Path(args.input_json).read_text())")
exec(compile(code, str(source), 'exec'), {'__name__': '__main__', '__file__': str(source)})

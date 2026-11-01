from pathlib import Path
source=Path(__file__).resolve().parents[2] / 'scratch/straightinkpen2-alias/inkpen2_saved_caller_native.py'
code=source.read_text().replace("parser.add_argument('--pen-name', choices=", "parser.add_argument('--input-json', required=True)\nparser.add_argument('--pen-name', choices=")
code=code.replace('input_case=INPUT_CASES[args.case]', "input_case=json.loads(pathlib.Path(args.input_json).read_text())")
code=code.replace("p.f(0,input_case['size']); p.call('SPen::ObjectStroke::SetPenSize(float)'", "\n if input_case.get('tolerance') is not None:\n  p.f(0,input_case['tolerance']); p.call('SPen::ObjectStroke::SetInitialToloerance(float)',[obj]); p.call('SPen::ObjectStroke::GetInitialToloerance() const',[obj]); assert p.u.reg_read(R.UC_ARM64_REG_S0)==struct.unpack('<I',struct.pack('<f',input_case['tolerance']))[0]\n p.f(0,input_case['size']); p.call('SPen::ObjectStroke::SetPenSize(float)'")
exec(compile(code,str(source),'exec'),{'__name__':'__main__','__file__':str(source)})

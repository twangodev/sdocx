"""Replay InkPen V4 source strokes through the pinned saved Drawing caller.

The runner applies exact in-memory edits to the pinned caller source and runs
each capture in a fresh Python process. It does not generate temporary files.
"""
import argparse
import hashlib
import json
import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
CALLER = ROOT / "conformance/inkpen_saved_caller_native.py"
FIXTURE = ROOT / "conformance/inkpen-reuse-native.json"
CALLER_SHA256 = "c0b1657cdcf94bcf2c173ce68ed7817e55a4de246883b47265bb7b4d3f50dc04"
INKPEN_DSO = ROOT / "scratch/apk-analysis-native/arm64-v8a/libSPenInkPen.so"
INKPEN_DSO_SHA256 = "939a7c719838306d693f96701994486b03f5c7eea35f16f25c6a89d84daba401"
REUSE_BRANCH = {"rva": "0x30eec", "bytes_le": "60250036", "mnemonic": "tbz w0, #0, 0x31398"}


def replace_once(source, old, new, label):
    count = source.count(old)
    if count != 1:
        raise RuntimeError(f"caller patch {label!r} expected once, found {count}")
    return source.replace(old, new, 1)


def patched_caller_source():
    raw = CALLER.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    if digest != CALLER_SHA256:
        raise RuntimeError(f"pinned caller hash changed: {digest}")
    source = raw.decode("utf-8")
    source = replace_once(
        source,
        "parser.add_argument('--advanced-setting',choices=('null','empty'),default='null')",
        "parser.add_argument('--advanced-setting',choices=('null','empty'),default='null')\n"
        "parser.add_argument('--reuse-first',choices=('fresh','short','long'),default='fresh')\n"
        "parser.add_argument('--distinct-object',action='store_true')",
        "scenario arguments",
    )
    source = replace_once(
        source,
        "input_case={'points':[[0.,0.],[10.,0.],[20.,0.],[30.,10.]],'size':10.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[.25,.5,.75,1.0],'timestamps':[10,20,30,40],'tilt':[0.]*4,'orientation':[0.]*4}",
        "input_case={'points':[[0.,0.],[10.,0.],[20.,0.],[30.,10.]],'size':10.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[.25,.5,.75,1.0],'timestamps':[10,20,30,40],'tilt':[0.]*4,'orientation':[0.]*4}\n"
        "target_case=input_case\n"
        "if args.reuse_first=='short':\n"
        " input_case={'points':[[0.,0.],[1.,0.],[2.,0.]],'size':10.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[.25,.5,.75],'timestamps':[10,20,30],'tilt':[0.]*3,'orientation':[0.]*3}\n"
        "elif args.reuse_first=='long':\n"
        " input_case={'points':[[0.,0.],[20.,10.],[40.,20.],[60.,30.],[80.,40.],[100.,50.]],'size':10.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[.2,.35,.5,.65,.8,1.0],'timestamps':[10,20,30,40,50,60],'tilt':[0.]*6,'orientation':[0.]*6}\n"
        "first_input_case=input_case",
        "preconditioning source cases",
    )
    source = replace_once(source, "drawable_entry_snapshots=[]", "drawable_entry_snapshots=[]\nobject_cache_state=[]", "cache snapshots")
    source = replace_once(
        source,
        "def snapshot_source_arrays(obj):",
        "def snapshot_base_cache(label,obj):\n"
        " p.call('SPen::ObjectBase::GetUserId() const',[obj]); uid=p.x(0)\n"
        " p.call('SPen::ObjectBase::IsChanged() const',[obj]); changed=p.x(0)\n"
        " p.call('SPen::ObjectBase::IsRedrawingCacheDirty() const',[obj]); dirty=p.x(0)\n"
        " row={'label':label,'object':hex(obj),'user_id':uid,'is_changed':changed,'is_redrawing_cache_dirty':dirty}\n"
        " object_cache_state.append(row); return row\n\n"
        "def snapshot_source_arrays(obj):",
        "native dirty-state reader",
    )
    source = replace_once(
        source,
        "snapshot_object_stroke('before_drawObjectStroke',obj); source_object_arrays=snapshot_source_arrays(obj); phase='draw_object_stroke';",
        "snapshot_object_stroke('before_drawObjectStroke',obj); source_object_arrays=snapshot_source_arrays(obj);\n"
        " if args.distinct_object:\n"
        "  first_base_impl=p.alloc(0x200); p.call('SPen::ObjectBaseImpl::ObjectBaseImpl(SPen::ObjectBase*)',[first_base_impl,obj]); p.put(obj+16,'Q',first_base_impl); p.call('SPen::ObjectBase::SetUserId(int)',[obj,101]); snapshot_base_cache('first_object_before_draw',obj)\n"
        " phase='draw_object_stroke';",
        "first object setup",
    )
    source = replace_once(
        source,
        "if draw_result != 1 or not factory_created_pens:\n  raise RuntimeError(f'Drawing failed to select InkPen: result={draw_result}, loader={loader_calls}')",
        "if draw_result != 1 or not factory_created_pens:\n  raise RuntimeError(f'Drawing failed to select InkPen: result={draw_result}, loader={loader_calls}')\n"
        " first_stream_row=next(row for row in reversed(inkpen_buffer_events) if row['phase']=='draw_object_stroke')\n"
        " if args.distinct_object: snapshot_base_cache('first_object_after_draw',obj)\n"
        " first_pen_ptr=factory_created_pens[-1]; p.call('SPen::InkPen::GetStrokeDrawableGL()',[first_pen_ptr]); first_drawable_ptr=p.x(0); first_rt_ptr=struct.unpack('<Q',p.u.mem_read(first_drawable_ptr+16,8))[0] if first_drawable_ptr else 0\n"
        " first_native_state={'pen':hex(first_pen_ptr),'drawable':hex(first_drawable_ptr),'rt':hex(first_rt_ptr),'drawable_bytes':bytes(p.u.mem_read(first_drawable_ptr,0x200)).hex() if first_drawable_ptr else None,'rt_bytes':bytes(p.u.mem_read(first_rt_ptr,0x120)).hex() if first_rt_ptr else None}\n"
        " if args.reuse_first!='fresh' or args.distinct_object:\n"
        "  target_obj=obj\n"
        "  if args.distinct_object:\n"
        "   target_obj=p.alloc(0x100); target_impl=p.alloc(0x400); p.call('SPen::ObjectStroke::ObjectStroke()',[target_obj]); p.call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[target_impl,target_obj]); p.put(target_obj+32,'Q',target_impl); p.f(0,target_case['size']); p.call('SPen::ObjectStroke::SetPenSize(float)',[target_obj]); p.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[target_obj,target_case['tool_type']]); p.call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[target_obj,target_case['stroke_type']]); p.call('SPen::ObjectStroke::SetCurveEnabled(bool)',[target_obj,int(target_case['curve_enabled'])]); p.call('SPen::ObjectBase::SetVisibility(bool)',[target_obj,1]); p.call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[target_obj,pname])\n"
        "   if args.advanced_setting=='empty': p.call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[target_obj,advanced])\n"
        "  target_points=target_case['points']; target_count=len(target_points); target_pts=p.alloc(target_count*8); target_prs=p.alloc(target_count*4); target_ts=p.alloc(target_count*4); target_tilt=p.alloc(target_count*4); target_orient=p.alloc(target_count*4)\n"
        "  p.u.mem_write(target_pts,struct.pack('<'+'f'*(target_count*2),*(v for xy in target_points for v in xy))); p.u.mem_write(target_prs,struct.pack('<'+'f'*target_count,*target_case['pressure'])); p.u.mem_write(target_ts,struct.pack('<'+'i'*target_count,*target_case['timestamps'])); p.u.mem_write(target_tilt,struct.pack('<'+'f'*target_count,*target_case['tilt'])); p.u.mem_write(target_orient,struct.pack('<'+'f'*target_count,*target_case['orientation']))\n"
        "  p.call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[target_obj,target_pts,target_prs,target_ts,target_tilt,target_orient,target_count]); target_source_arrays=snapshot_source_arrays(target_obj)\n"
        "  if args.distinct_object:\n"
        "   target_base_impl=p.alloc(0x200); p.call('SPen::ObjectBaseImpl::ObjectBaseImpl(SPen::ObjectBase*)',[target_base_impl,target_obj]); p.put(target_obj+16,'Q',target_base_impl); p.call('SPen::ObjectBase::SetUserId(int)',[target_obj,202]); snapshot_base_cache('target_object_before_draw',target_obj)\n"
        "  snapshot_object_stroke('before_reuse_target_draw',target_obj); phase='reuse_second_draw'; reuse_draw_result=p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[p.drawing,p.draw_canvas,p.pen_canvas,target_obj,1],limit=5000000); reuse_draw_result=p.x(0)\n"
        "  if reuse_draw_result!=1: raise RuntimeError(f'reuse target draw failed: {reuse_draw_result}')\n"
        "  if args.distinct_object: snapshot_base_cache('target_object_after_draw',target_obj)\n"
        "  reuse_stream_row=next(row for row in reversed(inkpen_buffer_events) if row['phase']=='reuse_second_draw')\n"
        "  target_pen_ptr=factory_created_pens[-1]; p.call('SPen::InkPen::GetStrokeDrawableGL()',[target_pen_ptr]); target_drawable_ptr=p.x(0); target_rt_ptr=struct.unpack('<Q',p.u.mem_read(target_drawable_ptr+16,8))[0] if target_drawable_ptr else 0\n"
        "  target_native_state={'pen':hex(target_pen_ptr),'drawable':hex(target_drawable_ptr),'rt':hex(target_rt_ptr),'drawable_bytes':bytes(p.u.mem_read(target_drawable_ptr,0x200)).hex() if target_drawable_ptr else None,'rt_bytes':bytes(p.u.mem_read(target_rt_ptr,0x120)).hex() if target_rt_ptr else None}\n"
        "  input_case=target_case\n"
        " else:\n"
        "  target_obj=obj; target_native_state=first_native_state; target_source_arrays=source_object_arrays",
        "reuse sequence",
    )
    source = replace_once(
        source,
        "call_address(selected_object_redraw_target,[actual_drawable+8,obj,rect,rect]); saved_redraw_result=p.x(0); saved_redraw_rect=bytes(p.u.mem_read(rect,16)).hex()",
        "call_address(selected_object_redraw_target,[actual_drawable+8,target_obj if args.reuse_first!='fresh' or args.distinct_object else obj,rect,rect]); saved_redraw_result=p.x(0); saved_redraw_rect=bytes(p.u.mem_read(rect,16)).hex()",
        "dirty-gated distinct replay",
    )
    source = replace_once(
        source,
        "if screen_resolution_calls != [{'width':args.width,'height':args.height}]: raise RuntimeError(f'unexpected native screen profile calls: {screen_resolution_calls}')",
        "expected_screen_call={'width':args.width,'height':args.height}\n if not screen_resolution_calls or any(row!=expected_screen_call for row in screen_resolution_calls): raise RuntimeError(f'unexpected native screen profile calls: {screen_resolution_calls}')",
        "repeated explicit profile calls",
    )
    source = replace_once(source, "draw_stream=bytes.fromhex(next(row for row in reversed(inkpen_buffer_events) if row['phase']=='draw_object_stroke')['after_bytes'])", "draw_phase='reuse_second_draw' if args.reuse_first!='fresh' else 'draw_object_stroke'\n draw_stream=bytes.fromhex(next(row for row in reversed(inkpen_buffer_events) if row['phase']==draw_phase)['after_bytes'])", "target stream select")
    source = replace_once(source, "saved_stream=bytes.fromhex(next(row for row in reversed(inkpen_buffer_events) if row['phase']=='saved_redraw')['after_bytes'])", "saved_stream=(bytes.fromhex(next(row for row in reversed(inkpen_buffer_events) if row['phase']=='saved_redraw')['after_bytes']) if any(row['phase']=='saved_redraw' for row in inkpen_buffer_events) else b'')", "distinct stream")
    source = replace_once(source, "if not draw_stream or len(draw_stream)%12 or draw_stream!=saved_stream:", "if not draw_stream or len(draw_stream)%12 or (not args.distinct_object and draw_stream!=saved_stream):", "target/saved equality")
    phase_loop = "for phase_name in ('draw_object_stroke','saved_redraw'):"
    if source.count(phase_loop) != 2:
        raise RuntimeError("caller patch 'phase loop' expected twice")
    source = source.replace(phase_loop, "for phase_name in ((draw_phase,) if args.distinct_object else (draw_phase,'saved_redraw')):")
    source = replace_once(source, "'schema':'inkpen-saved-caller-native-v1'", "'schema':'inkpen-v4-reuse-capture-v1'", "capture schema")
    source = replace_once(source, "'callback_count':phase_counts['draw_object_stroke']", "'callback_count':phase_counts[draw_phase]", "target count")
    source = replace_once(source, "'callback_count':phase_counts['saved_redraw']", "'callback_count':phase_counts.get('saved_redraw',0)", "saved count")
    source = replace_once(
        source,
        "'points_f32_bits':[[struct.pack('<f',v).hex() for v in xy] for xy in points]",
        "'points_f32_bits':[[struct.pack('<f',v).hex() for v in xy] for xy in (target_case['points'] if args.reuse_first!='fresh' or args.distinct_object else points)]",
        "target input points",
    )
    source = replace_once(
        source,
        "'streams':{'draw_object_stroke':",
        "'reuse':{'mode':args.reuse_first,'distinct_object':args.distinct_object,'first_input':{'points_f32_bits':[[struct.pack('<f',v).hex() for v in xy] for xy in first_input_case['points']],'pressure_f32_bits':[struct.pack('<f',v).hex() for v in first_input_case['pressure']],'timestamps_i32':first_input_case['timestamps'],'tilt_f32_bits':[struct.pack('<f',v).hex() for v in first_input_case['tilt']],'orientation_f32_bits':[struct.pack('<f',v).hex() for v in first_input_case['orientation']]},'first_source_ObjectStroke_arrays':source_object_arrays,'target_source_ObjectStroke_arrays':target_source_arrays,'first_object_address':hex(obj),'target_object_address':hex(target_obj) if args.reuse_first!='fresh' or args.distinct_object else hex(obj),'first_user_id':101 if args.distinct_object else None,'target_user_id':202 if args.distinct_object else (101 if args.reuse_first!='fresh' else None),'first_native_state':first_native_state,'target_native_state':target_native_state,'ObjectBase_cache_dirty_transitions':object_cache_state,'same_pen_object':first_native_state['pen']==target_native_state['pen'],'same_drawable_object':first_native_state['drawable']==target_native_state['drawable'],'same_rt_object':first_native_state['rt']==target_native_state['rt'],'first_stream_sha256':hashlib.sha256(bytes.fromhex(first_stream_row['after_bytes'])).hexdigest(),'first_stream_bytes':first_stream_row['after_bytes'],'target_draw_phase':draw_phase,'target_drawing_return':reuse_draw_result if args.reuse_first!='fresh' else None,'target_stream_bytes':draw_stream.hex(),'target_stream_sha256':hashlib.sha256(draw_stream).hexdigest(),'drawable_entry_snapshots':drawable_entry_snapshots,'saved_redraw_native_entries':[row for row in native_draw_entries if row['phase']=='saved_redraw' and row['pc']==hex(selected_object_redraw_target)]},'streams':{'draw_object_stroke':",
        "reuse record",
    )
    source = replace_once(
        source,
        "getter_phases!={'draw_object_stroke','saved_redraw'}",
        "not {'draw_object_stroke'}<=getter_phases or (not args.distinct_object and 'saved_redraw' not in getter_phases)",
        "getter phases",
    )
    source = replace_once(source, "if draw_result != 1 or saved_redraw_result != 1:", "if draw_result != 1 or saved_redraw_result != 1:", "return validation")
    source = replace_once(source, "if not any(row['phase']=='saved_redraw' and row['pc']==hex(selected_object_redraw_target) for row in native_draw_entries):", "if not any(row['phase']=='saved_redraw' and row['pc']==hex(selected_object_redraw_target) for row in native_draw_entries):", "direct replay validation")
    source = replace_once(source, "draw_terminal=next(row for row in reversed(inkpen_buffer_events) if row['phase']=='draw_object_stroke')['after_bytes']", "draw_terminal=next(row for row in reversed(inkpen_buffer_events) if row['phase']==draw_phase)['after_bytes']", "target terminal")
    source = replace_once(source, "saved_terminal=next(row for row in reversed(inkpen_buffer_events) if row['phase']=='saved_redraw')['after_bytes']", "saved_terminal=(next(row for row in reversed(inkpen_buffer_events) if row['phase']=='saved_redraw')['after_bytes'] if any(row['phase']=='saved_redraw' for row in inkpen_buffer_events) else '')", "saved terminal")
    source = replace_once(source, "if draw_terminal != saved_terminal:", "if args.distinct_object and not saved_terminal:\n  pass\n elif draw_terminal != saved_terminal:", "byte equality validation")
    return source


def capture(case_args):
    source = patched_caller_source()
    launcher = (
        "import pathlib,sys\n"
        "filename=sys.argv[1]; argv=sys.argv[2:]; sys.argv=[filename,*argv]\n"
        "namespace={'__file__':filename,'__name__':'__main__'}\n"
        "exec(compile(sys.stdin.read(),filename,'exec'),namespace)\n"
    )
    command = [
        sys.executable,
        "-c",
        launcher,
        str(CALLER),
        "--print-capture",
        "--advanced-setting",
        "empty",
        "--width",
        "1920",
        "--height",
        "1080",
        *case_args,
    ]
    completed = subprocess.run(command, input=source, cwd=ROOT, env=os.environ.copy(), text=True, capture_output=True)
    if completed.returncode:
        raise RuntimeError(f"native child failed ({completed.returncode}):\n{completed.stderr}\n{completed.stdout[-3000:]}")
    return json.loads(completed.stdout)


def cases_now():
    return {
        "fresh": capture([]),
        "short_first_same_object": capture(["--reuse-first", "short"]),
        "long_first_same_object": capture(["--reuse-first", "long"]),
        "short_first_distinct_object": capture(["--reuse-first", "short", "--distinct-object"]),
    }


def validate(cases):
    fresh = cases["fresh"]
    expected = fresh["streams"]["draw_object_stroke"]["sha256"]
    if expected != "7b08daf7b07bd579c26d04009dfa4f13858858fc35508c24c66fb21ce4ed84e6":
        raise RuntimeError(f"fresh stream changed: {expected}")
    for name in ("short_first_same_object", "long_first_same_object", "short_first_distinct_object"):
        case = cases[name]
        reuse = case["reuse"]
        def packed(values):
            return "".join(values)

        def require_arrays(label, source, values):
            expected_arrays = {
                "points": packed(bit for point in values["points_f32_bits"] for bit in point),
                "pressures": packed(values["pressure_f32_bits"]),
                "timestamps": packed(int(value).to_bytes(4, "little", signed=True).hex() for value in values["timestamps_i32"]),
                "tilts": packed(values["tilt_f32_bits"]),
                "orientations": packed(values["orientation_f32_bits"]),
            }
            for channel, expected_hex in expected_arrays.items():
                if source[channel]["data_hex"] != expected_hex:
                    raise RuntimeError(f"{label} native Model {channel} bytes disagree with the declared input")

        require_arrays(f"{name} first", reuse["first_source_ObjectStroke_arrays"], reuse["first_input"])
        target_input = case["input"]
        target_channels = {key: target_input[key] for key in ("points_f32_bits", "pressure_f32_bits", "timestamps_i32", "tilt_f32_bits", "orientation_f32_bits")}
        require_arrays(f"{name} target", reuse["target_source_ObjectStroke_arrays"], target_channels)
        if case["route"]["selected_drawable_vtable_id"] != "v4":
            raise RuntimeError(f"{name} did not select V4")
        first_state = reuse["first_native_state"]
        target_state = reuse["target_native_state"]
        for identity in ("pen", "drawable", "rt"):
            if first_state[identity] in (None, "0x0") or target_state[identity] in (None, "0x0"):
                raise RuntimeError(f"{name} has null native {identity} identity")
            if first_state[identity] != target_state[identity]:
                raise RuntimeError(f"{name} changed native {identity} identity")
        if not (reuse["same_pen_object"] and reuse["same_drawable_object"] and reuse["same_rt_object"]):
            raise RuntimeError(f"{name} identity summary disagrees with captured addresses")
        if not first_state["drawable_bytes"] or not first_state["rt_bytes"] or not target_state["drawable_bytes"] or not target_state["rt_bytes"]:
            raise RuntimeError(f"{name} lacks full native drawable/RT snapshots")
        if reuse["target_stream_sha256"] != expected:
            raise RuntimeError(f"{name} target stream differs from fresh")
        fresh_bits = fresh["streams"]["draw_object_stroke"]["values_f32_bits"]
        target_bits = case["streams"]["draw_object_stroke"]["values_f32_bits"]
        if target_bits != fresh_bits:
            raise RuntimeError(f"{name} target float bits differ from fresh")
        target_bytes = b"".join(bytes.fromhex(value) for value in target_bits)
        if target_bytes.hex() != reuse["target_stream_bytes"] or hashlib.sha256(target_bytes).hexdigest() != reuse["target_stream_sha256"]:
            raise RuntimeError(f"{name} target bytes/hash disagree with captured float words")
        first_bytes = bytes.fromhex(reuse["first_stream_bytes"])
        if hashlib.sha256(first_bytes).hexdigest() != reuse["first_stream_sha256"]:
            raise RuntimeError(f"{name} first stream bytes/hash disagree")
        if name == "short_first_distinct_object":
            if not reuse["saved_redraw_native_entries"] or case["route"]["saved_redraw_return"] != 1:
                raise RuntimeError("distinct direct RedrawPen was not executed to native return")
            dirty = reuse["ObjectBase_cache_dirty_transitions"]
            if [(row["user_id"], row["is_redrawing_cache_dirty"]) for row in dirty] != [(101, 1), (101, 0), (202, 1), (202, 0)]:
                raise RuntimeError(f"distinct ObjectBase dirty lifecycle changed: {dirty}")
            if case["streams"]["saved_redraw"]["callback_count"] != 0:
                raise RuntimeError("distinct clean ObjectStroke unexpectedly generated direct saved-redraw points")
        elif case["streams"]["saved_redraw"]["sha256"] != reuse["target_stream_sha256"]:
            raise RuntimeError(f"{name} direct saved RedrawPen differs from Drawing target")
    if REUSE_BRANCH["bytes_le"] != INKPEN_DSO.read_bytes()[int(REUSE_BRANCH["rva"], 16):int(REUSE_BRANCH["rva"], 16) + 4].hex():
        raise RuntimeError("pinned V4 dirty-gate instruction bytes changed")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--write-fixture", action="store_true")
    modes.add_argument("--check-fixture", action="store_true")
    modes.add_argument("--print-capture", action="store_true")
    args = parser.parse_args()
    if not (args.write_fixture or args.check_fixture or args.print_capture):
        args.check_fixture = True
    if hashlib.sha256(INKPEN_DSO.read_bytes()).hexdigest() != INKPEN_DSO_SHA256:
        raise RuntimeError("pinned InkPen DSO hash changed")
    captures = cases_now()
    validate(captures)
    envelope = {
        "schema": "sdocx-inkpen-v4-reuse-native-v1",
        "source_caller": {"path": "conformance/inkpen_saved_caller_native.py", "sha256": CALLER_SHA256},
        "patched_source_sha256": hashlib.sha256(patched_caller_source().encode("utf-8")).hexdigest(),
        "runner_sha256": hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),
        "native_library": {"path": "scratch/apk-analysis-native/arm64-v8a/libSPenInkPen.so", "sha256": INKPEN_DSO_SHA256},
        "reuse_branch": REUSE_BRANCH,
        "cases": captures,
        "scope": "Native Model ObjectStroke setters through Drawing-selected InkPen V4 and RTV1 callback vectors under an explicit 1920x1080 context. Distinct-object direct RedrawPen is dirty-gated after Drawing consumes the cache-dirty state. No Android device, serialized SDK parser, renderer pixels, or export parity claim.",
    }
    if args.write_fixture:
        FIXTURE.write_text(json.dumps(envelope, indent=2, sort_keys=True) + "\n")
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    elif args.check_fixture:
        if not FIXTURE.exists() or json.loads(FIXTURE.read_text()) != envelope:
            raise SystemExit("InkPen V4 reuse fixture mismatch")
        print(f"verified {FIXTURE.relative_to(ROOT)} (fresh, short-first, long-first, distinct-object)")
    else:
        print(json.dumps(envelope, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Native Pencil3 V3 display-resolution to RTV1 background-scale witness.

Default mode replays four fresh saved-caller cases and checks the pinned JSON.
Use --write-fixture only after reviewing a deliberate native/toolchain update.
"""
from pathlib import Path
import hashlib, json, os, struct, subprocess, sys

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "conformance/pencil3-paper-scale-native.json"
CALLER = ROOT / "conformance/pencil3_saved_caller_native.py"
LIBDIR = ROOT / "scratch/apk-analysis-native/arm64-v8a"
PINS = {
    "caller_script_sha256": "129001ce587c104e22e1675e759df9a04d942e884a93fb747b195d001b6a776e",
    "helper_sources": {
        "oblique_canvas_native.py": "1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2",
        "pencil_native.py": "b4d88fb86da22c732e63466e6f1fc7c829aca9d0b2623abbcb3d0fc6f981795c",
        "fountain_native.py": "40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30",
        "elf_plt.py": "816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b",
    },
    "native_libraries": {
        "Base": "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
        "Composer": "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f",
        "Drawing": "788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd",
        "FountainPen": "1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec",
        "Graphics": "aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621",
        "Model": "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a",
        "PenCommon": "afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d",
        "Pencil3": "41a995cac7a597af53359401df26187d4299221f3460c91b56540596fc73d359",
        "libSPenComposer.so": "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f",
        "libc++_shared.so": "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4",
    },
}
PROFILES = (
    ("landscape_1920x1080", 1920, 1080),
    ("landscape_1280x720", 1280, 720),
    ("landscape_2560x1440", 2560, 1440),
    ("portrait_1080x1920", 1080, 1920),
)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def preflight():
    if sha(CALLER) != PINS["caller_script_sha256"]:
        raise SystemExit("pinned pencil3_saved_caller_native.py changed; review and update pins explicitly")
    for name, expected in PINS["helper_sources"].items():
        path = ROOT / "conformance" / name
        if sha(path) != expected:
            raise SystemExit(f"pinned helper source changed: {path.relative_to(ROOT)}")
    for name, expected in PINS["native_libraries"].items():
        filename = name if name.endswith(".so") else f"libSPen{name}.so"
        path = LIBDIR / filename
        if sha(path) != expected:
            raise SystemExit(f"pinned native library changed: {path.relative_to(ROOT)}")


def f32hex(value):
    return struct.pack("<f", value).hex()


def run_case(label, width, height):
    env = os.environ.copy()
    env["PYTHONPATH"] = os.pathsep.join(filter(None, [
        str(ROOT / "scratch/apk-analysis-runtime/python"),
        str(ROOT / "conformance"), env.get("PYTHONPATH", "")]))
    ld = str(ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu")
    env["LD_LIBRARY_PATH"] = os.pathsep.join(filter(None, [ld, env.get("LD_LIBRARY_PATH", "")]))
    env["PENCIL_CONTEXT_WIDTH"] = str(width)
    env["PENCIL_CONTEXT_HEIGHT"] = str(height)
    env["PENCIL_PAPER_SCALE_NATIVE_CHILD"] = "1"
    proc = subprocess.run([sys.executable, str(Path(__file__).resolve())], cwd=ROOT,
                          env=env, text=True, capture_output=True, check=True)
    raw = json.loads(proc.stdout)
    if raw["harness_sha256"] != PINS["caller_script_sha256"]:
        raise SystemExit("executed caller source hash differs from preflight pin")
    if raw["libraries"] != PINS["native_libraries"]:
        raise SystemExit(f"runtime library identity differs for {label}: {raw['libraries']}")
    if raw["screen_resolution_calls"] != [{"width": width, "height": height}]:
        raise SystemExit(f"Pen::SetScreenResolution arguments differ for {label}")
    resolution = raw["context"]["native_pen_setting_resolution"]
    if resolution != [width, height]:
        raise SystemExit(f"native Pen::getSettingData resolution differs for {label}")
    if raw["route"].get("selected_drawable_class") != "SPen::Pencil3StrokeDrawableGLV3" or raw["route"].get("selected_drawable_vtable_id") != "v3":
        raise SystemExit(f"expected selected GLV3 path for {label}: {raw['route']}")
    if "Pencil3StrokeDrawableGLV3::RedrawPen" not in raw["route"].get("redraw_entry", ""):
        raise SystemExit(f"unexpected redraw entry for {label}: {raw['route']}")

    tasks = [row for row in raw["queued_native_tasks"]
             if row["phase"] == "saved_redraw" and row["raw_qwords_0_64"][3] == "0x510d48c"]
    if len(tasks) != 1:
        raise SystemExit(f"expected one saved-redraw SetBackgroundScale task for {label}, got {len(tasks)}")
    task = tasks[0]
    worker_runs = [row for row in raw["task_run_seen"]
                   if row["phase"] == "saved_redraw" and row["pc"] == "0x5108a54"
                   and row["x0"] == task["task"]]
    if len(worker_runs) != 1:
        raise SystemExit(f"native WorkerTask::Run receiver did not match queued task for {label}")
    setter = [row for row in raw["native_depthmap_setter_trace"]
              if row["phase"] == "saved_redraw" and row["pc"] == "0x510d48c"]
    if len(setter) != 1:
        raise SystemExit(f"native RTV1::SetBackgroundScale call missing/duplicated for {label}")
    stored = [row for row in raw["scale_after"] if row["phase"] == "saved_redraw"]
    if len(stored) != 1:
        raise SystemExit(f"native RTV1+264 store missing/duplicated for {label}")
    selected_rt = raw["route"]["selected_rt_address"]
    task_target = task["raw_qwords_0_64"][2]
    if task_target != selected_rt or setter[0]["x0"] != task_target or stored[0]["x0"] != task_target:
        raise SystemExit(f"scale task target / RTV1 field receiver mismatch for {label}")

    expected_bits = f32hex(min(width, height) / 1440.0)
    task_bits = task["raw_qwords_0_64"][5][-8:]
    # qword text is numeric hex, with the float in its low 32 bits.
    task_bits_le = int(task_bits, 16).to_bytes(4, "little").hex()
    if task_bits_le != expected_bits:
        raise SystemExit(f"native WorkerTask argument does not equal min(resolution)/1440 for {label}: {task_bits_le} != {expected_bits}")
    if stored[0]["scale_bits"] != expected_bits:
        raise SystemExit(f"RTV1+264 differs from task argument for {label}")
    if setter[0]["s0_f32"] != struct.unpack("<f", bytes.fromhex(expected_bits))[0]:
        raise SystemExit(f"native setter S0 differs from task argument for {label}")

    return {
        "profile": [width, height],
        "native_Pen_SetScreenResolution_args": [width, height],
        "native_getSettingData_resolution_offsets_32_36": resolution,
        "min_dimension": min(resolution),
        "RedrawPen_getSettingData_scale_1440_over_min_f32_bits": raw["context"]["native_derived_min_dimension_scale_f32_bits"],
        "RedrawPen_output_min_over_1440_expected_f32_bits": expected_bits,
        "native_WorkerTask": {
            "vtable": task["vtable"],
            "method_pointer": task["raw_qwords_0_64"][3],
            "float_argument_low32_bits": task_bits_le,
            "Run_entry_pc": worker_runs[0]["pc"],
            "Run_entry_this": worker_runs[0]["x0"],
            "queued_task_object": task["task"],
            "task_run_receiver_matches_queued_object": worker_runs[0]["x0"] == task["task"],
            "target_receiver_from_task_qword2": task_target,
            "target_matches_selected_RTV1": task_target == selected_rt,
        },
        "native_RTV1_SetBackgroundScale": {
            "pc": setter[0]["pc"],
            "this": setter[0]["x0"],
            "matches_selected_RTV1": setter[0]["x0"] == selected_rt,
            "field_write_this": stored[0]["x0"],
            "field_write_receiver_matches_setter": stored[0]["x0"] == setter[0]["x0"],
            "S0": setter[0]["s0_f32"],
            "stored_RTV1_plus264_f32_bits": stored[0]["scale_bits"],
        },
        "host_background_bitmap_dimensions": raw["native_observations"]["background_dimensions"],
        "RTV1_depthmap_pointer_after_saved_redraw": raw["rtv1_after_native_saved_redraw"]["depthmap_ptr"],
        "native_GL_or_RTV1_Draw_entries": raw["paint_entries"],
    }


def collect():
    preflight()
    cases = [run_case(label, width, height) for label, width, height in PROFILES]
    return {
        "schema": "sdocx-pencil3-paper-scale-native-v1",
        "runner_sha256": sha(Path(__file__).resolve()),
        "pinned_inputs": PINS,
        "in_memory_source_patch": {
            "base_source_path": "conformance/pencil3_saved_caller_native.py",
            "base_source_sha256": PINS["caller_script_sha256"],
            "insertion_anchor": "phase='draw_object_stroke';",
            "insertion_anchor_occurrences_required": 1,
            "result_assembly_anchor": "'scope':'Native Drawing caller and direct replay of its selected versioned Pencil3 RedrawPen entry under an explicit synthetic context. Does not claim Samsung device defaults, SDK deserialization, renderer pixels, or export parity.'}",
            "result_assembly_anchor_occurrences_required": 1,
            "instrumented_observations": ["queued native task object and qwords", "WorkerTask Run receiver", "SetBackgroundScale receiver and S0", "RTV1+264 post-store bytes"],
        },
        "native_static_contract": {
            "GLV3_RedrawPen_resolution_and_formula": "Pencil3StrokeDrawableGLV3::RedrawPen ELF VA 0x10b1e4 reads native Pen::getSettingData resolution ints at +32/+36, selects min with CMP/CSEL, converts to float, and divides by literal 1440.0 (0x44b40000) at ELF VA 0x10b1f0..0x10b1fc.",
            "helper_and_task_argument": "RedrawPen passes the formula local address as X4 to inherited helper ELF VA 0x10826c; helper reads S0 from [X4] at 0x1082a8 and writes the float to WorkerTask+40 at 0x1082bc. The task method pointer is RTV1::SetBackgroundScale(float), rebased PC 0x510d48c.",
            "native_task_and_field_store": "The existing host IGLMsgQueue shim synchronously invokes the native task Run body (saved-redraw run entry PC 0x5108a54); native RTV1::SetBackgroundScale at rebased PC 0x510d48c stores S0 directly to this+264.",
        },
        "execution_boundary": [
            "Four fresh Unicorn AArch64 runs use the pinned Model/Drawing/Graphics/Pencil3 DSOs and the existing saved ObjectStroke caller. Each synthetic IContext profile flows through native Pen::SetScreenResolution and native Pen::getSettingData.",
            "RedrawPen formula, inherited task construction, WorkerTask::Run body, and RTV1 scale setter execute native code. The IGLMsgQueue is a host shim that synchronously dispatches queued native tasks.",
            "ObjectDrawing background/depthmap bitmap wrappers, texture getters, rendering context/canvas, and native resource lifecycle seams are host supplied; the background bitmap remains 640x360 for all four profiles.",
            "This witness ends after the RTV1+264 scale field store. It does not execute GLV3::Draw or RTV1::Draw, and makes no bitmap producer, application scheduler, GLES, Android device, or final paint claim.",
        ],
        "cases": cases,
    }


if os.environ.get("PENCIL_PAPER_SCALE_NATIVE_CHILD") != "1":
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--check-fixture", action="store_true")
    group.add_argument("--write-fixture", action="store_true")
    args = parser.parse_args()
    data = collect()
    encoded = json.dumps(data, indent=2, sort_keys=True) + "\n"
    if args.write_fixture:
        OUT.write_text(encoded)
        print(f"wrote {OUT.relative_to(ROOT)}")
    else:
        if not OUT.exists():
            raise SystemExit(f"missing {OUT.relative_to(ROOT)}; use --write-fixture after review")
        if OUT.read_text() != encoded:
            raise SystemExit("native Pencil3 paper scale fixture mismatch")
        print("verified four native Pencil3 resolution-to-RTV1 scale profiles")
    raise SystemExit(0)

"""Bounded saved Drawing caller with one synthetic depth-map ISPBitmap.

This reuses the pinned Pencil3 native caller harness without modifying it. The
bitmap object is an explicit host adapter; Drawing, DrawingUtil, Pencil3 V3 and
RTV1 setters execute as native AArch64 code in Unicorn.
"""
from pathlib import Path
import contextlib, hashlib, io, json, os, struct, sys

ROOT = Path(__file__).resolve().parents[1]
os.chdir(ROOT)
sys.path.insert(0, str(ROOT / "conformance"))
sys.path.insert(0, str(ROOT / "scratch/apk-analysis-runtime/python"))

source_path = ROOT / "conformance/pencil3_saved_caller_native.py"
source = source_path.read_text()
needle = "phase='draw_object_stroke';"
inject = r'''
# Explicit synthetic ISPBitmap adapter: its texture getter returns a non-null
# host texture pointer and its integer dimension getters return 320x180.
paper_bitmap=p.alloc(64); paper_vtable=p.alloc(192); paper_texture=p.alloc(32)
p.put(paper_bitmap,'Q',paper_vtable)
def literal_return(address, value):
 p.u.mem_write(address, struct.pack('<IIQ', 0x58000040, 0xd65f03c0, value))
literal_return(0x200f2100,paper_texture)
literal_return(0x200f2110,0)
literal_return(0x200f2120,320)
literal_return(0x200f2130,180)
p.put(paper_vtable+144,'Q',0x200f2100) # GetTextureBitmap
p.put(paper_vtable+128,'Q',0x200f2110) # native RT setter's other getter
p.put(paper_vtable+72,'Q',0x200f2120)  # GetWidth
p.put(paper_vtable+80,'Q',0x200f2130)  # GetHeight
p.call('SPen::ObjectDrawing::SetDepthmap(SPen::ISPBitmap*)',[p.drawing,paper_bitmap])
background_bitmap=p.alloc(64); background_vtable=p.alloc(192); background_texture=p.alloc(32)
p.put(background_bitmap,'Q',background_vtable)
literal_return(0x200f2140,background_texture)
literal_return(0x200f2150,0)
literal_return(0x200f2160,640)
literal_return(0x200f2170,360)
p.put(background_vtable+144,'Q',0x200f2140)
p.put(background_vtable+128,'Q',0x200f2150)
p.put(background_vtable+72,'Q',0x200f2160)
p.put(background_vtable+80,'Q',0x200f2170)
p.call('SPen::ObjectDrawing::SetBackground(SPen::ISPBitmap*)',[p.drawing,background_bitmap])
paper_depthmap_injected={'drawing_depthmap_field':hex(struct.unpack('<Q',p.u.mem_read(p.drawing+48,8))[0]),'depthmap_bitmap':hex(paper_bitmap),'depthmap_texture':hex(paper_texture),'depthmap_dimensions':[320,180],'drawing_background_field':hex(struct.unpack('<Q',p.u.mem_read(p.drawing+40,8))[0]),'background_bitmap':hex(background_bitmap),'background_texture':hex(background_texture),'background_dimensions':[640,360]}
from unicorn import UC_HOOK_MEM_WRITE
penreturn_symbols={n:a for n,a in p.syms.items() if 'SPen::PenReturnCallback::' in n and '@plt' not in n}
penreturn_ctor_pcs={a for n,a in penreturn_symbols.items() if 'PenReturnCallback(SPen::PenDrawableRT*, SPen::ISPPenCanvas*)' in n}
penreturn_dtor_pcs={a for n,a in penreturn_symbols.items() if '~PenReturnCallback()' in n and '@plt' not in n}
penreturn_callback_objects=[]
penreturn_callback_events=[]
def trace_penreturn_callbacks(_uc,address,_size,_data):
 for obj in penreturn_callback_objects:
  if obj.get('active') and obj.get('return_pc')==address:
   obj['active']=False
   penreturn_callback_events.append({'event':'destructor_return','phase':phase,'pc':hex(address),'this':hex(obj['this'])})
 if address in penreturn_ctor_pcs:
  this=p.x(0); penreturn_callback_objects.append({'this':this,'phase':phase,'active':True,'return_pc':None}); penreturn_callback_events.append({'event':'constructor','phase':phase,'pc':hex(address),'this':hex(this)})
 elif address in penreturn_dtor_pcs:
  this=p.x(0)
  matches=[row for row in penreturn_callback_objects if row['this']==this and row.get('active')]
  if matches:
   retpc=p.u.reg_read(R.UC_ARM64_REG_LR)
   matches[-1]['return_pc']=retpc
   penreturn_callback_events.append({'event':'destructor_entry','phase':phase,'pc':hex(address),'return_pc':hex(retpc),'this':hex(this),'bytes_0_80':p.u.mem_read(this,80).hex()})
def trace_callback_writes(_uc,_access,address,size,value,_user_data):
 for obj in penreturn_callback_objects:
  if obj.get('active') and obj['this'] <= address < obj['this']+80:
   penreturn_callback_events.append({'event':'field_write','phase':phase,'pc':hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'this':hex(obj['this']),'offset':address-obj['this'],'size':size,'value_hex':(int(value) & ((1 << (8*size))-1)).to_bytes(size,'little').hex()})
p.u.hook_add(UC_HOOK_CODE,trace_penreturn_callbacks)
p.u.hook_add(UC_HOOK_MEM_WRITE,trace_callback_writes)
paper_setter_trace=[]
paper_setter_pcs={p.syms['SPen::Pencil3StrokeDrawableGLV3::SetDepthMapBitmap(SPen::ISPBitmap*)'],p.syms['SPen::Pencil3StrokeDrawableRTV1::SetDepthMapBitmap(SPen::ISPBitmap*)'],0x510d48c}
def trace_paper_setters(_uc,address,_size,_data):
 if address in paper_setter_pcs:
  paper_setter_trace.append({'phase':phase,'pc':hex(address),'x0':hex(p.x(0)),'x1':hex(p.x(1)),'s0_f32':struct.unpack('<f',p.u.reg_read(R.UC_ARM64_REG_Q0).to_bytes(16,'little')[:4])[0]})
p.u.hook_add(UC_HOOK_CODE,trace_paper_setters)
scale_after=[]
def trace_scale_return(_uc,address,_size,_data):
 if address==0x510d490: scale_after.append({'phase':phase,'x0':hex(p.x(0)),'scale_bits':p.u.mem_read(p.x(0)+264,4).hex()})
p.u.hook_add(UC_HOOK_CODE,trace_scale_return)
paint_entries=[]
queued_tasks=[]
executed_task_runs=[]
task_run_seen=[]
def trace_queue_enqueue(_uc,address,_size,_data):
 if address in (0x510a694,0x510cce0): paint_entries.append({'phase':phase,'pc':hex(address),'x0':hex(p.x(0)),'x1':hex(p.x(1)),'x2':hex(p.x(2))})
 if address in (0x51089d4,0x5108a00,0x5108a28,0x5108a54,0x510d48c,0x510c7e4): task_run_seen.append({'phase':phase,'pc':hex(address),'x0':hex(p.x(0)),'x1':hex(p.x(1)),'s0_f32':struct.unpack('<f',p.u.reg_read(R.UC_ARM64_REG_Q0).to_bytes(16,'little')[:4])[0]})
 elif address==0x20001000:
  task=p.x(1); vt=struct.unpack('<Q',p.u.mem_read(task,8))[0]
  slots=struct.unpack('<6Q',p.u.mem_read(vt,48))
  queued_tasks.append({'phase':phase,'task':hex(task),'vtable':hex(vt),'vtable_slots':[hex(v) for v in slots],'raw_qwords_0_64':[hex(v) for v in struct.unpack('<8Q',p.u.mem_read(task,64))]})
p.u.hook_add(UC_HOOK_CODE,trace_queue_enqueue)
'''
inject = "\n".join(" " + line if line else line for line in inject.splitlines())
if source.count(needle) != 1:
    raise RuntimeError("native caller injection point changed")
source = source.replace(needle, needle + inject)
old = "'scope':'Native Drawing caller and direct replay of its selected versioned Pencil3 RedrawPen entry under an explicit synthetic context. Does not claim Samsung device defaults, SDK deserialization, renderer pixels, or export parity.'}"
new = "'scope':'Native Drawing caller and direct replay of its selected versioned Pencil3 RedrawPen entry under an explicit synthetic context. Does not claim Samsung device defaults, SDK deserialization, renderer pixels, or export parity.','paper_depthmap_injected':paper_depthmap_injected,'paper_setter_trace':paper_setter_trace,'penreturn_callback_events':penreturn_callback_events,'penreturn_symbols':{n:hex(a) for n,a in penreturn_symbols.items()},'queued_tasks':queued_tasks,'executed_task_runs':executed_task_runs,'task_run_seen':task_run_seen,'scale_after':scale_after,'screen_resolution_calls':screen_resolution_calls,'paint_entries':paint_entries,'glv3_depthmap_ptr_after_saved_redraw':hex(struct.unpack('<Q',p.u.mem_read(actual_drawable+488,8))[0]),'rtv1_paper_state_after_saved_redraw':{'depthmap_ptr':hex(struct.unpack('<Q',p.u.mem_read(rt+248,8))[0]),'depthmap_dimensions':list(struct.unpack('<II',p.u.mem_read(rt+256,8))),'background_scale_f32_bits':p.u.mem_read(rt+264,4).hex()}}"
if source.count(old) != 1:
    raise RuntimeError("native caller result assembly changed")
source = source.replace(old,new)
context_width=int(os.environ.get('PENCIL_CONTEXT_WIDTH','1920')); context_height=int(os.environ.get('PENCIL_CONTEXT_HEIGHT','1080'))
sys.argv = [str(source_path), "--print-capture", "--advanced-setting", "empty", "--width", str(context_width), "--height", str(context_height)]
output = io.StringIO()
with contextlib.redirect_stdout(output):
    state = {"__name__": "__main__", "__file__": str(source_path)}
    exec(compile(source, str(source_path), "exec"), state)
capture = json.loads(output.getvalue())
symbols_by_address = {hex(address): name for name, address in state['p'].syms.items()}
for task in capture['queued_tasks']:
    task['vtable_symbols'] = [symbols_by_address.get(address) for address in task['vtable_slots']]
result = {
    "schema": "pencil3-paper-depthmap-route-probe-v1",
    "harness_sha256": hashlib.sha256(source_path.read_bytes()).hexdigest(),
    "libraries": capture["libraries"],
    "context": {"requested_native_profile": [context_width, context_height], "native_pen_setting_resolution": capture["pen_setting_data"]["screen_resolution_i32_offsets_32_36"], "native_derived_min_dimension_scale_f32_bits": capture["pen_setting_data"]["derived_movement_resolution_scale_f32_bits"]},
    "route": capture["route"],
    "input_boundary": "synthetic Drawing background bitmap 640x360 and depthmap bitmap 320x180, each with host-supplied nonnull GetTextureBitmap; original pinned saved-caller ObjectStroke input is otherwise unchanged",
    "native_observations": capture["paper_depthmap_injected"],
    "native_depthmap_setter_trace": capture["paper_setter_trace"],
    "penreturn_callback_events": capture["penreturn_callback_events"],
    "penreturn_symbols": capture["penreturn_symbols"],
    "queued_native_tasks": capture["queued_tasks"],
    "task_run_seen": capture["task_run_seen"],
    "scale_after": capture["scale_after"],
    "screen_resolution_calls": capture["screen_resolution_calls"],
    "paint_entries": capture["paint_entries"],
    "fake_queue_setup": {"queue": hex(state["queue"]), "vtable": hex(struct.unpack('<Q',state['p'].u.mem_read(state['queue'],8))[0]), "slot16": hex(struct.unpack('<Q',state['p'].u.mem_read(struct.unpack('<Q',state['p'].u.mem_read(state['queue'],8))[0]+16,8))[0])},
    "glv3_depthmap_pointer_after_saved_redraw": capture["glv3_depthmap_ptr_after_saved_redraw"],
    "rtv1_after_native_saved_redraw": capture["rtv1_paper_state_after_saved_redraw"],
    "scope": "Actual ObjectDrawing::SetBackground/SetDepthmap and drawObjectStroke plus native DrawingUtil dispatch and selected Pencil3 GLV3 SetDepthMapBitmap execute in Unicorn. The IGLMsgQueue is host supplied and synchronously executes the queued native WorkerTask; queued task qword[2], native SetBackgroundScale receiver, selected RTV1, and +264 field-write receiver are joined by pointer, and the native Run receiver is joined to the queued task object. The field value is profile-derived and recorded per case. No RT-level SetDepthMapBitmap call occurs on the executed saved-wrapper route; RTV1+248 remains null. Context and bitmap/texture objects/getters are host supplied. This does not establish application bitmap producers, production queue scheduling, the GL paint/Draw call, or device rendering.",
}
print(json.dumps(result, indent=2, sort_keys=True))

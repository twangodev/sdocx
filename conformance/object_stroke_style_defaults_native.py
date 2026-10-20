#!/usr/bin/env python3
"""Native Model omitted stroke/color type defaults on an original corpus frame."""
import argparse, hashlib, importlib.util, json, os, re, shutil, subprocess, sys
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
RUNNER=Path(__file__).resolve()
FIXTURE=ROOT/"conformance/object-stroke-style-defaults-native.json"
SOURCE=ROOT/"conformance/fountain-saved-color-native.json"
SOURCE_SHA="de23cf21f239fc9e12794bc55777fa18406d1129162f8198b1544b8f009f932d"
CHANNEL=ROOT/"conformance/object_stroke_channels_native.py"
CHANNEL_SHA="78216df84c591d5c04e1fbda4c4a93d01921513ca7b619a5a0a79f1d97f25c2a"
DSOS={"Model":"4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a","Base":"e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb"}
HELPERS={"conformance/calligraphy_saved_native.py":"663f15668825ce6ed219830f3e671c408b85ad103477b444201f68f0b12c6517","conformance/calligraphy_saved_wrapper_native.py":"5952473d15716fa686113f35887f550224462f2ee4fbd500c589cb4e52b05cff","conformance/fountain_native.py":"40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30"}
OBJDUMP=ROOT/"scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump"
NM=Path(shutil.which("nm"))
p=argparse.ArgumentParser(description=__doc__)
g=p.add_mutually_exclusive_group()
g.add_argument("--write-fixture",action="store_true")
g.add_argument("--check-fixture",action="store_true")
args=p.parse_args()

def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()

def source_frame():
    if sha(SOURCE)!=SOURCE_SHA: raise SystemExit("pinned source fixture changed")
    row=next(x for x in json.loads(SOURCE.read_text())["real_serialized_frames"] if x["id"]=="real_v14_handwritten_flat1445")
    frame=bytes.fromhex(row["frame_hex"])
    if hashlib.sha256(frame).hexdigest()!=row["frame_sha256"]: raise SystemExit("source frame hash mismatch")
    at=10; pc=frame[at]; at+=1+pc; fc=frame[at]; at+=1
    mask=frame[at:at+fc]; bits=int.from_bytes(mask,"little")
    if bits&(1<<16) or bits&(1<<25): raise SystemExit("selected frame has an explicit type field")
    return row,frame,mask

def native_capture(frame):
    if sha(CHANNEL)!=CHANNEL_SHA: raise SystemExit("pinned channel helper changed")
    src=CHANNEL.read_text(); patches=[]
    def sub(old,new,label):
        nonlocal src
        if src.count(old)!=1: raise SystemExit("patch anchor mismatch: "+label)
        src=src.replace(old,new,1)
        patches.append({"anchor":label,"old_sha256":hashlib.sha256(old.encode()).hexdigest(),"new_sha256":hashlib.sha256(new.encode()).hexdigest()})
    sub('"SPen::ObjectStroke::GetStrokeType() const",\n        "SPen::ObjectStroke::GetColor() const",',
        '"SPen::ObjectStroke::GetStrokeType() const",\n        "SPen::ObjectStroke::GetColorType() const",\n        "SPen::ObjectStroke::GetColor() const",',"color-type-symbol")
    sub('    source = n.alloc(len(frame))\n    n.u.mem_write(source, frame)\n',
'''    type_setters={"stroke_type":"SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)","color_type":"SPen::ObjectStroke::SetColorType(SPen::PenColorType)"}
    type_getters={"stroke_type":"SPen::ObjectStroke::GetStrokeType() const","color_type":"SPen::ObjectStroke::GetColorType() const"}
    seed_values={"stroke_type":2,"color_type":1}; seed={}
    for key,value in seed_values.items():
        n.call(type_setters[key],[obj,value]); seed[key]=call_native(type_getters[key],(obj,))["return_x0"]
    source=n.alloc(len(frame)); n.u.mem_write(source,frame)
''',"seed-before-apply")
    sub('    consumed = n.x(0) & 0xffffffff\n',
'''    consumed=n.x(0)&0xffffffff
    post_apply={key:call_native(name,(obj,))["return_x0"] for key,name in type_getters.items()}
''',"getter-after-apply")
    sub('        "stroke_type": gi("SPen::ObjectStroke::GetStrokeType() const"),\n        "color_argb":',
        '        "stroke_type": gi("SPen::ObjectStroke::GetStrokeType() const"),\n        "color_type": gi("SPen::ObjectStroke::GetColorType() const"),\n        "color_argb":',"final-state-color-type")
    sub('    support_dir = ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin"\n',
'''    control={}
    for key,value in (("stroke_type",1),("color_type",2)):
        n.call(type_setters[key],[obj,value]); control[key]=call_native(type_getters[key],(obj,))["return_x0"]
    for key in type_setters: n.call(type_setters[key],[obj,0])
    support_dir = ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin"
''',"setter-getter-controls")
    sub('    return {"consumed_bytes": consumed, "native_state": state, "arrays": arrays,\n',
        '    return {"consumed_bytes": consumed, "native_state": state, "arrays": arrays,\n            "type_controls":{"pre_apply_seed_getters":seed,"post_apply_getters":post_apply,"explicit_nonzero_setter_getters":control},\n',"return-controls")
    libdir=ROOT/"scratch/apk-analysis-native/arm64-v8a"
    for name,digest in DSOS.items():
        if sha(libdir/("libSPen"+name+".so"))!=digest: raise SystemExit("native DSO preflight failed: "+name)
    for path,digest in HELPERS.items():
        if sha(ROOT/path)!=digest: raise SystemExit("native helper preflight failed: "+path)
    tool_pins={str(NM):"ddb55c0685721bf494aa2d090056b8b32a40614ad4464b305896f527d882c3c1",str(OBJDUMP):"b9ae08c9c02c91ced02b80a140c937ba119fe073872fff701f67477bf597a5eb"}
    for path,digest in tool_pins.items():
        if sha(Path(path))!=digest: raise SystemExit("native tool preflight failed: "+path)
    sys.path[:0]=[str(ROOT/"conformance"),str(ROOT/"scratch/apk-analysis-runtime/python")]
    spec=importlib.util.spec_from_file_location("style_default_native_probe",CHANNEL)
    module=importlib.util.module_from_spec(spec)
    exec(compile(src,str(CHANNEL)+"[instrumented]","exec"),module.__dict__)
    return module.native_capture(frame),patches,hashlib.sha256(src.encode()).hexdigest()

def instruction_evidence():
    dso=ROOT/"scratch/apk-analysis-native/arm64-v8a/libSPenModel.so"
    env=dict(os.environ); env["LD_LIBRARY_PATH"]=str((ROOT/"scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu").resolve())
    ranges={"apply_stroke_type":[0x2edbec,0x2edc18],"apply_color_type":[0x2edd54,0x2edde8],"write_stroke_type":[0x2eca60,0x2eca98],"write_color_type":[0x2ecd28,0x2ecd60]}
    out={}
    for name,(start,stop) in ranges.items():
        raw=subprocess.check_output([str(OBJDUMP),"-d","-C",f"--start-address={start:#x}",f"--stop-address={stop:#x}",str(dso)],env=env,text=True)
        rows=[]
        for line in raw.splitlines():
            m=re.match(r"\s*([0-9a-f]+):\s+([0-9a-f]{8})\s+(.+)$",line)
            if m: rows.append({"rva":"0x"+m.group(1),"bytes_le_hex":bytes.fromhex(m.group(2)).hex(),"instruction":m.group(3).strip()})
        if not rows: raise SystemExit("empty disassembly evidence: "+name)
        out[name]=rows
    return out

def build():
    row,frame,mask=source_frame()
    capture,patches,adapted_sha=native_capture(frame)
    expected={"pre_apply_seed_getters":{"stroke_type":"0x2","color_type":"0x1"},
              "post_apply_getters":{"stroke_type":"0x0","color_type":"0x0"},
              "explicit_nonzero_setter_getters":{"stroke_type":"0x1","color_type":"0x2"}}
    if capture["type_controls"]!=expected: raise SystemExit("native type controls differ: "+repr(capture["type_controls"]))
    if capture["native_state"]["stroke_type"]!=0 or capture["native_state"]["color_type"]!=0: raise SystemExit("omitted type not native zero")
    if capture["consumed_bytes"]!=len(frame): raise SystemExit("NewApplyBinary did not consume full frame")
    apply_symbol="SPen::ObjectStrokeBinaryHandler::NewApplyBinary(unsigned char*, int, unsigned int, float)"
    apply_call=next((c for c in capture["native_calls"] if c["symbol"]==apply_symbol),None)
    if apply_call is None: raise SystemExit("NewApplyBinary call missing")
    dso_root=ROOT/"scratch/apk-analysis-native/arm64-v8a"
    if {n:sha(dso_root/("libSPen"+n+".so")) for n in DSOS}!=DSOS: raise SystemExit("Model/Base DSO hash mismatch")
    selected=("SPen::ObjectStrokeBinaryHandler::NewApplyBinary(unsigned char*, int, unsigned int, float)","SPen::ObjectStroke::GetStrokeType() const","SPen::ObjectStroke::GetColorType() const")
    entries=capture["native_entries"]
    return {
      "schema":"sdocx-object-stroke-style-defaults-native-v1",
      "source":{"fixture":"conformance/fountain-saved-color-native.json","fixture_sha256":SOURCE_SHA,"frame_id":row["id"],
        "source_document":"tmp/stroke-conformance/handwritten.sdocx","source_document_sha256":row["source_sha256"],"source_page_sha256":row["page_sha256"],
        "stroke_index":row["stroke_index"],"frame_sha256":hashlib.sha256(frame).hexdigest(),"frame_length":len(frame),
        "field_mask_hex":mask.hex(),"omitted_bits":[16,25],"model_sha256":DSOS["Model"],"base_sha256":DSOS["Base"],
        "channel_helper":"conformance/object_stroke_channels_native.py","channel_helper_sha256":CHANNEL_SHA,
        "instrumented_helper_sha256":adapted_sha,"runtime_helper_sha256":capture["helper_sha256"],
        "native_entries":{name:entries[name] for name in selected},"instruction_evidence":instruction_evidence(),"tool_sha256":{str(NM):sha(NM),str(OBJDUMP):sha(OBJDUMP)},"helper_pins":HELPERS,"runner_sha256":sha(RUNNER),"in_memory_patches":patches},
      "result":{"new_apply_return_bytes":capture["consumed_bytes"],"controls":capture["type_controls"],
        "native_state_after_apply":{"stroke_type":capture["native_state"]["stroke_type"],"color_type":capture["native_state"]["color_type"]},
        "native_apply_call":apply_call,
        "type_getter_calls":[c for c in capture["native_calls"] if c["symbol"] in selected[1:]],
        "mapped_dsos":capture["mapped_dsos"],
        "scope":"Native ObjectStrokeBinaryHandler::NewApplyBinary consumes an original corpus ObjectStroke frame after nonzero stroke/color type setters; getters observe omitted fields as zero. Separate setter/getter controls establish nonzero values. No Drawing, whole-note SDK loader, MagicPen-specific source, or rendering claim."},
      "parser_representation":{"stroke_type_raw":None,"color_type_raw":None,"meaning":"Both fields are absent in the source; parser None preserves omission. This demonstrates pinned Model defaults without rewriting parser metadata."}
    }

def main():
    data=json.dumps(build(),indent=2,sort_keys=True)+"\n"
    if args.write_fixture:
        FIXTURE.write_text(data); print("wrote "+str(FIXTURE.relative_to(ROOT)))
    elif not FIXTURE.exists() or FIXTURE.read_text()!=data:
        raise SystemExit("native style defaults fixture mismatch; use --write-fixture explicitly")
    else: print("verified original-frame omitted type defaults and setter controls")
if __name__=="__main__": main()

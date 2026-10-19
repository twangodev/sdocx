"""Bounded conformance check for Samsung's pinned PDFWriter path sink."""
import argparse
import hashlib
import json
import os
import shutil
import struct
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APK_DIR = ROOT / "scratch/apk-analysis-native/arm64-v8a"
FIXTURE = ROOT / "conformance/pdf-writer-path-native.json"
UNICORN_LIB = ROOT / "scratch/apk-analysis-runtime/python/unicorn/lib"
MODEL_SHA256 = "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a"
PDFIUM_SHA256 = "4bd55ef116541205cb8aaf04812a317fe11911c0ff5a5d44526d98d6b0e48854"
LIBCPP_SHA256 = "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4"
SPENPDF_SHA256 = "cdc62f9e02a3ef60e0dc504dbb13c4352accb811fb1ec629a7c8648954dd8f04"
SPENBASE_SHA256 = "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb"
LIBM_PATH = Path("/usr/lib/x86_64-linux-gnu/libm.so.6")
LIBM_SHA256 = "6d567d53e895273ca14a1f9dc164fc6c8d39aed2f60aa46a733c2784228915f3"
LIBZ_PATH = Path("/usr/lib/x86_64-linux-gnu/libz.so.1")
LIBZ_SHA256 = "85590dd58edf5445e18bc7193e5ebc01ac5841f1ae187e97705a662e90c6421e"

# This is inserted into a temporary copy of the existing runner after its
# pinned constructors/init arrays run and before any PDF document is opened.
DIAGNOSTIC = r'''        if std::env::var_os("SDOCX_PDF_SINK_CONTRACT").is_some() {
            const CONTRACT: u64 = 0x0902_0000;
            check(unsafe { uc_mem_map(machine.engine, CONTRACT, 0x1_0000, 7) });
            let engine = machine.call(SPENPDF_BASE + 0x6be50, &[]);
            assert_ne!(engine, 0, "PDFEnginePdfium::GetInstance returned null");
            let engine_vtable = read_u64(machine.engine, engine);
            let create_path_target = read_u64(machine.engine, engine_vtable + 664);
            let circle_target = read_u64(machine.engine, engine_vtable + 704);
            let draw_path_target = read_u64(machine.engine, engine_vtable + 1032);
            let writer = CONTRACT;
            write(machine.engine, writer + 8, &engine.to_le_bytes());
            let writer_created_path = machine.call(SPENPDF_BASE + 0x65d74, &[writer]);
            let path = CONTRACT + 0x100;
            machine.call(SPENPDF_BASE + 0x66450, &[path]);
            let path_constructed = machine.call(SPENPDF_BASE + 0x65de8, &[path, engine]);
            register(machine.engine, 136, f32::to_bits(23.25) as u64);
            register(machine.engine, 137, f32::to_bits(41.5) as u64);
            register(machine.engine, 138, f32::to_bits(2.75) as u64);
            machine.call(SPENPDF_BASE + 0x66500, &[path, 0]);
            let path_handle_after_circle = read_u64(machine.engine, path + 16);
            let paint = CONTRACT + 0x200;
            machine.call(SPENPDF_BASE + 0x6652c, &[paint]);
            let paint_constructed = machine.call(SPENPDF_BASE + 0x65d1c, &[paint, engine]);
            machine.call(SPENPDF_BASE + 0x665b4, &[paint, 0xff11_5cc9]);
            machine.call(SPENPDF_BASE + 0x665cc, &[paint, 0]);
            machine.call(SPENPDF_BASE + 0x65e40, &[writer, path, paint]);
            println!("{{\"native_engine\":\"PDFEnginePdfium::GetInstance\",\"engine_vtable\":\"0x{engine_vtable:x}\",\"engine_vtable_targets\":{{\"CreatePDFPath+664\":\"0x{create_path_target:x}\",\"Circle+704\":\"0x{circle_target:x}\",\"DrawPath+1032\":\"0x{draw_path_target:x}\"}},\"writer_create_path_return\":\"0x{writer_created_path:x}\",\"path_construct_result\":{path_constructed},\"path_handle_after_native_circle\":\"0x{path_handle_after_circle:x}\",\"paint_construct_result\":{paint_constructed},\"requested_calls\":[\"PDFWriter::CreatePath\",\"PDFWriter::PDFPath::Construct\",\"PDFWriter::PDFPath::Circle(23.25,41.5,2.75,false)\",\"PDFWriter::PDFPaint::Construct\",\"PDFWriter::PDFPaint::SetColor\",\"PDFWriter::PDFPaint::SetStyle\",\"PDFWriter::DrawPath\"],\"call_completion\":\"Machine::call asserts return to sentinel PC for each invocation\",\"document_opened\":false,\"serialized_output\":false}}\n");
            return;
        }
'''


def sha_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha(path: Path) -> str:
    return sha_bytes(path.read_bytes())


def elf_load_file_offset(image: bytes, virtual_address: int, length: int) -> int:
    if image[:4] != b"\x7fELF" or image[4:6] != b"\x02\x01":
        raise SystemExit("expected a little-endian ELF64 shared library")
    if struct.unpack_from("<H", image, 18)[0] != 183:
        raise SystemExit("expected an AArch64 ELF image")
    phoff = struct.unpack_from("<Q", image, 32)[0]
    phentsize, phnum = struct.unpack_from("<HH", image, 54)
    if phentsize < 56:
        raise SystemExit("unexpected ELF64 program-header size")
    matches = []
    for index in range(phnum):
        offset = phoff + index * phentsize
        p_type = struct.unpack_from("<I", image, offset)[0]
        if p_type != 1:  # PT_LOAD
            continue
        p_offset, p_vaddr, _, p_filesz = struct.unpack_from("<QQQQ", image, offset + 8)
        if p_vaddr <= virtual_address and virtual_address + length <= p_vaddr + p_filesz:
            matches.append(p_offset + virtual_address - p_vaddr)
    if len(matches) != 1:
        raise SystemExit(f"RVA {virtual_address:#x} does not map uniquely to a file-backed PT_LOAD segment")
    return matches[0]


def check_pins() -> dict:
    pins = {
        "model_library_sha256": (APK_DIR / "libSPenModel.so", MODEL_SHA256),
        "pdfium_library_sha256": (APK_DIR / "libSPenPdfiumB.so", PDFIUM_SHA256),
        "libcxx_library_sha256": (APK_DIR / "libc++_shared.so", LIBCPP_SHA256),
        "samsung_pdf_library_sha256": (APK_DIR / "libSPenPdf.so", SPENPDF_SHA256),
        "samsung_base_library_sha256": (APK_DIR / "libSPenBase.so", SPENBASE_SHA256),
        "host_libm_sha256": (LIBM_PATH, LIBM_SHA256),
        "host_zlib_sha256": (LIBZ_PATH, LIBZ_SHA256),
    }
    result = {}
    for name, (path, expected) in pins.items():
        actual = sha(path)
        if actual != expected:
            raise SystemExit(f"{name} changed: {actual}")
        result[name] = actual
    return result


def source_hashes() -> dict:
    source_path = ROOT / "conformance/native_table.rs"
    helper_paths = sorted((ROOT / "conformance/native_table").glob("*.rs"))
    return {
        "native_table.rs": sha(source_path),
        **{f"native_table/{path.name}": sha(path) for path in helper_paths},
    }


def patched_driver(temp: Path, expected_sources: dict | None) -> tuple[Path, dict]:
    source_path = ROOT / "conformance/native_table.rs"
    module_path = ROOT / "conformance/native_table/pdfium_create_page_save.rs"
    source = source_path.read_text()
    module = module_path.read_text()
    original_source_hashes = source_hashes()
    if expected_sources is not None and original_source_hashes != expected_sources:
        raise SystemExit("runner/helper source hashes differ from the fixture; use --write-fixture only after reviewing source changes")
    anchor = "        machine.call(PDFIUM_BASE + 0x5a5504, &[]);\n        const IMPL_OBJECT: u64 = 0x5000_0000;"
    if module.count(anchor) != 1:
        raise SystemExit("native runner insertion anchor is absent or ambiguous")
    module = module.replace(anchor, "        machine.call(PDFIUM_BASE + 0x5a5504, &[]);\n" + DIAGNOSTIC + "        const IMPL_OBJECT: u64 = 0x5000_0000;", 1)
    work = temp / "conformance"
    work.mkdir()
    shutil.copy2(source_path, work / "native_table.rs")
    shutil.copytree(ROOT / "conformance/native_table", work / "native_table")
    (work / "native_table/pdfium_create_page_save.rs").write_text(module)
    driver = temp / "pdf-writer-path-native-driver"
    subprocess.run([
        "rustc", "--edition", "2024", "-D", "warnings", "-C", "panic=abort",
        str(work / "native_table.rs"), "-L", f"native={UNICORN_LIB}",
        "-C", f"link-arg=-Wl,-rpath,{UNICORN_LIB}", "-o", str(driver),
    ], cwd=ROOT, check=True)
    provenance = {
        "unmodified_source_hashes": original_source_hashes,
        "runner_sha256": original_source_hashes["native_table.rs"],
        "temporary_patch_anchor": "after pinned SPenBase/SPenPdf init arrays and PDFium init call, before document implementation/open",
        "temporary_patch_sha256": sha_bytes(module.encode()),
        "patch_count": 1,
        "temporary_copy_module_count": len(list((work / "native_table").glob("*.rs"))),
        "original_module_site_count": source.count('#[path = "native_table/'),
    }
    return driver, provenance


def capture(expected_sources: dict | None) -> dict:
    pins = check_pins()
    with tempfile.TemporaryDirectory(prefix="sdocx-pdf-writer-path-") as directory:
        temp = Path(directory)
        driver, provenance = patched_driver(temp, expected_sources)
        env = os.environ.copy()
        env["SDOCX_PDF_SINK_CONTRACT"] = "1"
        run = subprocess.run([
            str(driver), str(APK_DIR / "libSPenModel.so"), "--pdfium-standard-export",
            str(APK_DIR / "libSPenPdfiumB.so"), "/dev/null", str(temp / "unused.pdf"),
        ], cwd=ROOT, env=env, text=True, capture_output=True, check=True)
        if run.stderr.strip():
            raise SystemExit(f"native diagnostic wrote unexpected stderr: {run.stderr}")
        observations = json.loads(run.stdout)

    library = APK_DIR / "libSPenPdf.so"
    image = library.read_bytes()
    code_rvas = {
        "PDFEnginePdfium::CreatePDFPath": 0x73270,
        "PDFEnginePdfium::Circle": 0x73288,
        "PDFEnginePdfium::DrawPath": 0x73478,
    }
    code_bytes = {}
    for name, rva in code_rvas.items():
        file_offset = elf_load_file_offset(image, rva, 16)
        code_bytes[name] = {"rva": f"0x{rva:x}", "elf_file_offset": f"0x{file_offset:x}", "bytes16": image[file_offset:file_offset + 16].hex()}
    expected_instruction_prefixes = {
        "PDFEnginePdfium::CreatePDFPath": "e0031faac0035fd6",
        "PDFEnginePdfium::Circle": "c0035fd6",
        "PDFEnginePdfium::DrawPath": "c0035fd6",
    }
    for method, prefix in expected_instruction_prefixes.items():
        if not code_bytes[method]["bytes16"].startswith(prefix):
            raise SystemExit(f"{method} instruction bytes no longer match the observed null/return stub")
    expected = {
        "CreatePDFPath+664": "0x3073270",
        "Circle+704": "0x3073288",
        "DrawPath+1032": "0x3073478",
    }
    if observations.get("engine_vtable_targets") != expected:
        raise SystemExit("native PDFEnginePdfium vtable targets differ from the pinned sink contract")
    if observations.get("writer_create_path_return") != "0x0":
        raise SystemExit("native PDFWriter::CreatePath no longer returns null")
    if observations.get("path_construct_result") != 0 or observations.get("path_handle_after_native_circle") != "0x0":
        raise SystemExit("native PDFPath construction/Circle behavior changed")
    if observations.get("paint_construct_result") != 1:
        raise SystemExit("native PDFPaint construction did not succeed")
    if observations.get("document_opened") is not False or observations.get("serialized_output") is not False:
        raise SystemExit("sink-only diagnostic crossed its document/serialization boundary")
    return {
        "schema": "sdocx.pdf-writer-path-native.v1",
        "wrapper_source_sha256": sha(Path(__file__)),
        "pinned_libraries": pins,
        "runner_source_sha256": provenance["runner_sha256"],
        "runner_provenance": provenance,
        "runtime": observations,
        "instruction_bytes_at_rva": code_bytes,
        "native_contract": {
            "engine_vtable_offsets_bytes": {"CreatePDFPath": 664, "Circle": 704, "DrawPath": 1032},
            "engine_vtable_targets": expected,
            "method_rvas": {"CreatePDFPath": "0x73270", "Circle": "0x73288", "DrawPath": "0x73478"},
            "actual_effect": {
                "CreatePDFPath": "returns null",
                "PDFWriter::PDFPath::Construct": "returns false",
                "Circle": "executes with x=23.25, y=41.5, radius=2.75, closed=false; path handle remains null",
                "PDFPaint": "constructs successfully and accepts color/style setters",
                "DrawPath": "actual writer method dispatch occurs; engine target returns immediately",
            },
        },
        "evidence_boundary": (
            "Executes the actual pinned PDFEnginePdfium::GetInstance and SPenPdf PDFWriter/PDFPath/PDFPaint methods in a bounded Unicorn run. "
            "The temporary diagnostic is inserted into a source-pinned copy of the existing Rust runner after library initialization. "
            "No PDF document is opened and no output is serialized. The stateful PDFium implementation context is not opened; this does not prove general PDFium inability, a Samsung Notes UI route, or arbitrary PDF export behavior."
        ),
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write-fixture", action="store_true")
    parser.add_argument("--check-fixture", action="store_true", help="explicitly run the default fixture check")
    args = parser.parse_args()
    if args.write_fixture and args.check_fixture:
        parser.error("--write-fixture and --check-fixture are mutually exclusive")
    expected_sources = None
    if not args.write_fixture:
        try:
            existing = json.loads(FIXTURE.read_text())
            if existing.get("wrapper_source_sha256") != sha(Path(__file__)):
                raise SystemExit("Python wrapper source hash differs from the pinned fixture")
            expected_sources = existing["runner_provenance"]["unmodified_source_hashes"]
        except (OSError, json.JSONDecodeError, KeyError, TypeError) as error:
            raise SystemExit(f"cannot preflight fixture source hashes: {error}") from error
    encoded = json.dumps(capture(expected_sources), indent=2) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    elif FIXTURE.read_text() != encoded:
        raise SystemExit("native PDFWriter path sink differs from conformance/pdf-writer-path-native.json")
    else:
        print(f"verified {FIXTURE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()

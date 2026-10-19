"""Run the pinned Samsung StandardPDF writer on the retained quiz PDF.

Supply the source PDF extracted from quiz.sdocx; the source bytes are not
stored in this repository. The output PDF must be a new path under /tmp.
"""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APK_DIR = ROOT / "scratch/apk-analysis-native/arm64-v8a"
UNICORN_LIB = ROOT / "scratch/apk-analysis-runtime/python/unicorn/lib"
DRIVER = Path("/tmp/sdocx-pdfium-quiz-source-native")
SOURCE_SHA256 = "f01243e2d712d47ad08735ec4e47da2a94974818f2cd7c28da2db3ee326c4279"
OUTPUT_SHA256 = "2c97c49a5d2de382ef8ecdfa65e4ebd536b7970d20c18bae3e4ff347107ad5b4"
MODEL_SHA256 = "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a"
PDFIUM_SHA256 = "4bd55ef116541205cb8aaf04812a317fe11911c0ff5a5d44526d98d6b0e48854"
LIBCPP_SHA256 = "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4"
LIBZ_SHA256 = "85590dd58edf5445e18bc7193e5ebc01ac5841f1ae187e97705a662e90c6421e"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-pdf", required=True, type=Path)
    parser.add_argument("--output-pdf", required=True, type=Path)
    args = parser.parse_args()

    source = args.source_pdf.resolve(strict=True)
    output = args.output_pdf.resolve()
    if not output.is_relative_to(Path("/tmp")):
        raise SystemExit("output PDF path must be under /tmp")
    if output.exists():
        raise SystemExit(f"output PDF already exists: {output}")
    source_hash = sha256(source)
    if source_hash != SOURCE_SHA256:
        raise SystemExit(f"quiz source SHA-256 changed: {source_hash}")

    subprocess.run([
        "rustc", "--edition", "2024", "-D", "warnings", "-C", "panic=abort",
        "conformance/native_table.rs", "-L", f"native={UNICORN_LIB}",
        "-C", f"link-arg=-Wl,-rpath,{UNICORN_LIB}",
        "-C", "link-arg=-l:libunicorn.so.2", "-o", str(DRIVER),
    ], cwd=ROOT, check=True)
    run = subprocess.run([
        str(DRIVER), str(APK_DIR / "libSPenModel.so"),
        "--pdfium-standard-export-source", str(APK_DIR / "libSPenPdfiumB.so"),
        str(source), SOURCE_SHA256, str(output),
    ], cwd=ROOT, text=True, capture_output=True, check=True)
    native = json.loads(run.stdout)
    output_hash = sha256(output)
    if output_hash != OUTPUT_SHA256:
        raise SystemExit(f"native quiz output SHA-256 changed: {output_hash}")
    if native.get("export_executed") is not True or native.get("export_result") != 0:
        raise SystemExit("Samsung PdfiumExportHandler::Export did not report execution success")
    if native.get("source_file_sha256") != SOURCE_SHA256:
        raise SystemExit("native capture source hash did not match the pinned quiz PDF")
    if native.get("output_file_sha256") != output_hash:
        raise SystemExit("native-reported output hash does not match emitted file bytes")
    if native.get("output_bytes") != output.stat().st_size or output.stat().st_size == 0:
        raise SystemExit("native-reported output length does not match emitted file bytes")
    if native.get("host_zlib_sha256") != LIBZ_SHA256:
        raise SystemExit("native capture did not use the pinned host zlib")
    if not native.get("inflate_streams") or any(item.get("calls", 0) <= 0 for item in native["inflate_streams"]):
        raise SystemExit("source import did not complete a tracked inflate stream")

    result = {
        "source_path": str(source),
        "source_sha256": SOURCE_SHA256,
        "output_path": str(output),
        "output_sha256": output_hash,
        "source_apk_version": "4.4.45.37",
        "model_library_sha256": MODEL_SHA256,
        "pdfium_library_sha256": PDFIUM_SHA256,
        "libcxx_library_sha256": LIBCPP_SHA256,
        "host_zlib_sha256": LIBZ_SHA256,
        "native": native,
        "evidence_boundary": (
            "Executes pinned Samsung PdfiumImpl constructor/Open/LoadPage, PDFium source parsing, "
            "native StandardPDF OpenWriter/DrawPath/CloseWriter/Export for inclusive page 0..0, "
            "and PDFFileWriter output. The note-space triangle and independently captured Composer "
            "ratio are explicit host inputs; BodyTextPDFWriter coordinate conversion is not run. "
            "Host zlib uses a bounded x86_64 stream adapter with the verified pinned library; the "
            "observed guest allocator callbacks are replaced by tracked host zeroed allocation/free "
            "callbacks and are never invoked as host function pointers. Host file/locale/time and "
            "single-thread platform callbacks remain explicit. This does not prove full app/JNI "
            "behavior, arbitrary PDF preservation, or Samsung pixel parity."
        ),
    }
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()

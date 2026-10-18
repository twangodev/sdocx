"""Capture the native Model -> Composer -> Drawing CodeBlock measurement path.

Run from the repository root:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/code_parent_layout_native.py --write-fixture
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/code_parent_layout_native.py --check-fixture
"""
import argparse
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APK_DIR = ROOT / "scratch/apk-analysis-native/arm64-v8a"
FIXTURE = ROOT / "conformance/code-parent-layout.json"
DRIVER = Path("/tmp/sdocx-native-code-parent-layout")


def capture(args):
    subprocess.run(
        [
            "rustc",
            "--edition",
            "2024",
            "-D",
            "warnings",
            "-C",
            "panic=abort",
            "conformance/native_table.rs",
            "-L",
            "native=scratch/apk-analysis-runtime/python/unicorn/lib",
            "-C",
            "link-arg=-Wl,-rpath," + str(ROOT / "scratch/apk-analysis-runtime/python/unicorn/lib"),
            "-o",
            str(DRIVER),
        ],
        cwd=ROOT,
        check=True,
    )
    paths = [
        args.base or APK_DIR / "libSPenBase.so",
        args.drawing or APK_DIR / "libSPenDrawing.so",
        args.bodytext or APK_DIR / "libSPenBodytext.so",
        args.cpp or APK_DIR / "libc++_shared.so",
        args.text or APK_DIR / "libSPenText.so",
        args.skia or APK_DIR / "libSPenSkia.so",
        args.font or ROOT / "crates/sdocx/assets/fonts/Roboto-Regular.ttf",
        args.xml or APK_DIR / "libSPenLibxml2.so",
        args.widget or APK_DIR / "libSPenWidget.so",
        args.content or APK_DIR / "libSPenContent.so",
        args.view or APK_DIR / "libSPenView.so",
        args.composer or APK_DIR / "libSPenComposer.so",
    ]
    return subprocess.check_output(
        [
            str(DRIVER),
            str(args.model or APK_DIR / "libSPenModel.so"),
            "--code-parent-layout",
            *map(str, paths),
        ],
        cwd=ROOT,
        text=True,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--write-fixture", action="store_true")
    group.add_argument("--check-fixture", action="store_true")
    for name in (
        "model",
        "base",
        "drawing",
        "bodytext",
        "cpp",
        "text",
        "skia",
        "font",
        "xml",
        "widget",
        "content",
        "view",
        "composer",
    ):
        parser.add_argument("--" + name, type=Path)
    args = parser.parse_args()
    encoded = json.dumps(json.loads(capture(args)), indent=2, separators=(",", ": ")) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    elif FIXTURE.read_text() != encoded:
        raise SystemExit("native CodeBlock parent capture differs from conformance/code-parent-layout.json")
    else:
        print(f"verified {FIXTURE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()

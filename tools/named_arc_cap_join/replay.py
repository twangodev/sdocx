import os
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[2]
package = Path(__file__).resolve().parent
lib = Path(sys.argv[1]).resolve()
unicorn = Path(sys.argv[2]).resolve()
assert (
    hashlib.sha256((unicorn / "libunicorn.so.2").read_bytes()).hexdigest()
    == "ddb196ec82b52e502c18e4a34478bf7b9f61c83c2ebaa95c74d8ded45a95da9c"
)
expected_sources = json.loads((package / "sources.json").read_text())
cases = {f"{cap}-{join}-{dash}": (cap, join, dash) for cap, join, dash in [(1, 0, 0), (2, 0, 0), (0, 1, 0), (0, 2, 0), (1, 2, 2), (2, 1, 2)]}
with tempfile.TemporaryDirectory(prefix="sdocx-arc-moved-") as tmpdir:
    tmp = Path(tmpdir)
    (tmp / "conformance").mkdir()
    shutil.copy(
        root / "conformance/native_table.rs", tmp / "conformance/native_table.rs"
    )
    shutil.copytree(
        root / "conformance/native_table",
        tmp / "conformance/native_table",
        symlinks=True,
    )
    shutil.copytree(
        root / "tools", tmp / "tools", symlinks=True, copy_function=os.symlink
    )
    (tmp / "tools/named_arc_drawing/capture.rs").unlink()
    shutil.copy(package / "capture.rs", tmp / "tools/named_arc_drawing/capture.rs")
    (tmp / "libunicorn.so").symlink_to(unicorn / "libunicorn.so.2")
    subprocess.run(
        [
            "rustc",
            "--edition=2024",
            "--emit=dep-info",
            "conformance/native_table.rs",
            "-o",
            "sources.d",
        ],
        cwd=tmp,
        check=True,
    )
    dependencies = (
        (tmp / "sources.d").read_text().splitlines()[0].split(": ", 1)[1].split()
    )
    actual_sources = {os.path.normpath(name) for name in dependencies}
    assert actual_sources == set(expected_sources), actual_sources ^ set(
        expected_sources
    )
    for name, digest in expected_sources.items():
        assert hashlib.sha256((tmp / name).read_bytes()).hexdigest() == digest, name
    subprocess.run(
        [
            "rustc",
            "--edition=2024",
            "conformance/native_table.rs",
            "-L",
            str(tmp),
            "-o",
            str(tmp / "probe"),
        ],
        cwd=tmp,
        check=True,
    )
    for name, (cap, join, dash) in cases.items():
        for fill in [0, 85, 165, 255, 0]:
            env = os.environ | dict(
                LD_LIBRARY_PATH=str(unicorn),
                ARC_OUTPUT=str(tmp / "frame"),
                ARC_SECOND="1",
                ARC_WIDTH="6",
                ARC_DASH=str(dash),
                CAP=str(cap),
                JOIN=str(join),
                ARC_COLOR="80009955",
                ARC_FILL=str(fill),
            )
            run = subprocess.run(
                [
                    str(tmp / "probe"),
                    str(lib / "libSPenModel.so"),
                    "--named-arc-drawing",
                    str(lib / "libSPenBase.so"),
                    str(lib / "libSPenSkia.so"),
                ],
                env=env,
                capture_output=True,
                check=True,
            )
            receipt = (
                "\n".join(
                    line
                    for line in run.stderr.decode().splitlines()
                    if line.startswith(
                        (
                            "CAPJOIN ",
                            "DASH ",
                            "EFFECT ",
                            "PAINT_BYTES ",
                            "PAINT ",
                            "WIDTH_SETTERS ",
                            "canvas ",
                            "ACTIVE ",
                            "SOURCE_CONTROLS ",
                            "LEDGER ",
                            "HOST_CALLS ",
                            "reached plt ",
                        )
                    )
                )
                + "\n"
            )
            assert (
                receipt == (package / "receipts" / (name + ".receipt")).read_text()
            ), name
            assert (tmp / "frame").read_bytes() == (
                root
                / "crates/sdocx/tests/fixtures"
                / ("native-arc-cap-join-" + name + ".bin")
            ).read_bytes(), name
result = subprocess.run(
    [
        sys.executable,
        str(root / "tools/named_arc_moved/audit.py"),
        str(lib),
        str(root / "tools/named_arc_dash/target-trace.txt"),
    ],
    capture_output=True,
    check=True,
)
assert result.stdout == (root / "tools/named_arc_dash/audit.txt").read_bytes()
print("Native Arc cap and join recordings and effective-target audit matched.")

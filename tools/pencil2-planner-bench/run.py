#!/usr/bin/env python3
"""Benchmark the pinned preflight commit against its parent in a temporary crate."""
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
COMMIT = "504b0654ee0423e057c0bbf2532b02b377946755"


def source(revision, name):
    return subprocess.check_output(
        ["git", "show", f"{revision}:crates/sdocx/src/ink/{name}"],
        cwd=ROOT,
        text=True,
    )


with tempfile.TemporaryDirectory(prefix="sdocx-pencil-bench-") as tmp:
    crate = Path(tmp)
    ink = crate / "src/ink"
    ink.mkdir(parents=True)
    (crate / "Cargo.toml").write_text(
        '[package]\nname="sdocx-pencil-bench"\nversion="0.1.0"\n'
        'edition="2024"\n[dependencies]\nlibm="0.2"\nserde_json="1"\n'
    )
    (crate / "Cargo.lock").write_bytes((ROOT / "tools/pencil2-planner-bench/Cargo.lock").read_bytes())
    (crate / "src/main.rs").write_text((ROOT / "tools/pencil2-planner-bench/main.rs").read_text())
    (ink / "mod.rs").write_text(
        "pub mod before_path; pub mod after_path; pub mod before; pub mod after;\n"
    )
    for label, revision in (("before", COMMIT + "^"), ("after", COMMIT)):
        (ink / f"{label}_path.rs").write_text(source(revision, "path.rs"))
        pencil = source(revision, "pencil2.rs")
        pencil = pencil[: pencil.index('#[cfg(all(test, feature = "serde"))]\nfn ')]
        pencil = pencil.replace("mod source;\npub(crate) use source::prepare_stroke;", "")
        pencil = pencil.replace("super::path::{P, Quad}", f"super::{label}_path::{{P, Quad}}")
        pencil += "\npub fn run(input: &SavedInput, alpha: AlphaState) -> Result<Vec<SampledRecord>, SamplerError> {\n"
        if label == "after":
            pencil += "plan_records(input.points.len(), |i| input.points[i])?;\n"
        pencil += "sample(input, alpha)\n}\n"
        (ink / f"{label}.rs").write_text(pencil)
    fixture = crate / "fresh-source.json"
    fixture.write_text(subprocess.check_output(
        ["git", "show", f"{COMMIT}:crates/sdocx/src/ink/pencil2/assets/fresh-source.json"],
        cwd=ROOT, text=True,
    ))
    env = os.environ.copy()
    env.pop("CARGO_ENCODED_RUSTFLAGS", None)
    env.update(RUSTFLAGS="", CARGO_PROFILE_RELEASE_OPT_LEVEL="3", CARGO_PROFILE_RELEASE_DEBUG="0",
               CARGO_TARGET_DIR="/tmp/sdocx-agent-target", CARGO_INCREMENTAL="0",
               CARGO_PROFILE_DEV_DEBUG="0", CARGO_PROFILE_TEST_DEBUG="0")
    subprocess.run(["rustc", "--version"], env=env, check=True)
    subprocess.run(["cargo", "run", "--release", "--offline", "--locked", "--manifest-path",
                    str(crate / "Cargo.toml"), "--", str(fixture)], env=env, check=True)

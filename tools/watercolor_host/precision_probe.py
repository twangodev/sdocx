"""Observe the dense 2x control's remaining mask pixel after every native stamp."""

import hashlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "tools/watercolor_host/prefix_probe.py"
PIN = "84c82b8c13550d5ee28f0330fa65f1382f28ba1569161201f6f0118fdcaaaf58"
source = SOURCE.read_text()
assert hashlib.sha256(source.encode()).hexdigest() == PIN
source = source.replace("1920x1080_1_32", "768x640_2_32")
source = source.replace("1920", "768").replace("1080", "640").replace("1079", "631")
source = source.replace("glReadPixels'](10,631", "glReadPixels'](44,631")
source = source.replace(
    '"pixel_bottom_origin": [10, 631]', '"pixel_bottom_origin": [44, 631]'
)
source = source.replace('"--scale",\n        "1",', '"--scale",\n        "2",')
source = source.replace(
    "tools/watercolor_host/prefix_probe.py", "tools/watercolor_host/precision_probe.py"
)
source = source.replace(
    "watercolor-prefix-pixel-native.json", "watercolor-precision-pixel-native.json"
)
source = source.replace(
    '"frame_helper_sha256": PIN,',
    '"frame_helper_sha256": PIN, "prefix_controller_sha256": PRECISION_PARENT_PIN,',
)
exec(
    compile(source, str(SOURCE), "exec"),
    {
        "__name__": "watercolor_precision_probe",
        "__file__": __file__,
        "PRECISION_PARENT_PIN": PIN,
    },
)

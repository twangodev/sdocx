import collections
import pathlib
import re
import struct
import subprocess
import sys

root = pathlib.Path(sys.argv[1]).resolve()
script_dir = pathlib.Path(__file__).resolve().parent
libs = [
    ("model", "libSPenModel.so", 0, 0),
    ("drawing", "libSPenDrawing.so", 0x1000000, 0),
    ("base", "libSPenBase.so", 0x4000000, 0),
    ("skia", "libSPenSkia.so", 0x6000000, 0),
    ("cxx", "libc++_shared.so", 0x8000000, 0x4000),
    ("pen", "libSPenPenCommon.so", 0xA000000, 0),
]
d = {}
for n, p, base, shift in libs:
    f = root / p
    rel = subprocess.check_output(["readelf", "-Wr", str(f)], text=True)
    syms = subprocess.check_output(["readelf", "-Ws", str(f)], text=True)
    reloc = {
        int(k, 16): v.split("@")[0]
        for k, v in re.findall(
            r"^([0-9a-f]+)\s+\S+\s+R_AARCH64_\w+\s+\S+\s+(\S+)", rel, re.M
        )
    }
    byname = collections.defaultdict(set)
    for addr, ndx, name in re.findall(
        r"^\s*\d+:\s+([0-9a-f]+)\s+\d+\s+FUNC\s+\S+\s+\S+\s+(\S+)\s+(\S+)", syms, re.M
    ):
        if ndx != "UND":
            byname[name.split("@")[0]].add(int(addr, 16))
    d[n] = (f.read_bytes(), reloc, byname, base, shift)
trace_path = (
    pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else script_dir / "target-trace.txt"
)
trace = trace_path.read_text().split("reached plt {")[1].split("}")[0]
pairs = [
    tuple(int(x, 16) for x in z) for z in re.findall(r"([0-9a-f]+): ([0-9a-f]+)", trace)
]
for pc, target in pairs:
    n, (_, _, _, base, shift) = next(
        (n, v) for n, v in d.items() if v[3] <= pc < v[3] + 0x1000000
    )
    binary, reloc, _, _, _ = d[n]
    addr = pc - base
    adrp, ldr = struct.unpack_from("<II", binary, addr - shift)
    if (adrp & 0x9F00001F) != 0x90000010 or (ldr & 0xFFC003FF) != 0xF9400211:
        raise AssertionError((n, hex(pc), "unresolved"))
    imm = ((adrp >> 29) & 3) | (((adrp >> 5) & 0x7FFFF) << 2)
    imm = imm - (1 << 21) if imm & (1 << 20) else imm
    got = (addr & ~4095) + (imm << 12) + (((ldr >> 10) & 0xFFF) * 8)
    name = reloc.get(got, "?")
    matches = [
        lib
        for lib, (_, _, symbols, b, _) in d.items()
        if target - b in symbols.get(name, set())
    ]
    host = {
        0x10030000: {"_ZdlPv", "_ZdaPv", "_ZdlPvRKSt9nothrow_t"},
        0x10030010: {"_Znwm", "_Znam", "_ZnwmRKSt9nothrow_t", "_ZnamRKSt9nothrow_t"},
        0x10030020: {"free"},
        0x10030040: {"memcpy"},
        0x10030050: {"memmove", "memcpy"},
        0x10030060: {"realloc"},
        0x10030070: {"sincosf"},
        0x100300A0: {"tanf"},
        0x100300B0: {"atan2f"},
        0x100300C0: {"sincos"},
        0x100300D0: {
            "__android_log_print",
            "__cxa_atexit",
            "pthread_mutex_init",
            "pthread_mutex_destroy",
            "pthread_mutex_lock",
            "pthread_mutex_unlock",
            "pthread_mutexattr_init",
            "pthread_mutexattr_settype",
            "pthread_mutexattr_destroy",
            "_ZNSt6__ndk115recursive_mutexC1Ev",
            "_ZNSt6__ndk115recursive_mutexD1Ev",
            "_ZNSt6__ndk115recursive_mutex4lockEv",
            "_ZNSt6__ndk115recursive_mutex6unlockEv",
            "_ZN13Spen_SkCanvas4saveEv",
            "_ZN13Spen_SkCanvas5scaleEff",
            "_ZN13Spen_SkCanvas9translateEff",
            "_ZN13Spen_SkCanvas7restoreEv",
        },
        0x100300E0: {"__emutls_get_address"},
        0x100300F0: {"strlen"},
        0x10030100: {"pow"},
        0x10030110: {"uuid_create"},
        0x10030120: {"uuid_make"},
        0x10030130: {"uuid_export"},
        0x10030140: {"uuid_destroy"},
        0x10030150: {"memcmp"},
        0x10030160: {"acosf"},
        0x10030170: {"__cxa_guard_acquire"},
        0x10030180: {"__cxa_guard_release"},
    }
    kind = (
        "HOST"
        if name in host.get(target, set())
        else ("MATCH" if matches else "MISMATCH")
    )
    assert kind != "MISMATCH", (n, hex(pc), name, hex(target))
    print(" ".join([n, hex(pc), name, hex(target), kind, ",".join(matches)]).rstrip())

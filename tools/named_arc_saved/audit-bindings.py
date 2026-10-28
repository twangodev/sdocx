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
    ("base", "libSPenBase.so", 0x4000000, 0),
    ("skia", "libSPenSkia.so", 0x6000000, 0),
    ("cxx", "libc++_shared.so", 0x8000000, 0x4000),
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
    byaddr = collections.defaultdict(set)
    byname = collections.defaultdict(set)
    for addr, name in re.findall(
        r"^\s*\d+:\s+([0-9a-f]+)\s+\d+\s+FUNC\s+\S+\s+\S+\s+\S+\s+(\S+)", syms, re.M
    ):
        name = name.split("@")[0]
        addr = int(addr, 16)
        byaddr[addr].add(name)
        byname[name].add(addr)
    d[n] = (f.read_bytes(), reloc, byaddr, byname, base, shift)


def lookup(n, addr):
    binary, reloc, _, _, base, shift = d[n]
    addr -= base
    adrp, ldr = struct.unpack_from("<II", binary, addr - shift)
    assert adrp & 0x9F00001F == 0x90000010 and ldr & 0xFFC003FF == 0xF9400211, (
        n,
        hex(addr),
        hex(adrp),
        hex(ldr),
    )
    imm = ((adrp >> 29) & 3) | (((adrp >> 5) & 0x7FFFF) << 2)
    imm = imm - (1 << 21) if imm & (1 << 20) else imm
    got = (addr & ~4095) + (imm << 12) + (((ldr >> 10) & 0xFFF) * 8)
    return got, reloc.get(got, "?")


s = (script_dir / "capture.rs").read_text()
line = next(x for x in s.splitlines() if "4696848" in x)
pairs = [tuple(map(int, x)) for x in re.findall(r"\((\d+), (\d+)\)", line)]
print("NUMERIC MODEL BINDINGS: PLT imported symbol -> destination; alias status")
for a, t in pairs:
    got, name = lookup("model", a)
    target = next(
        (
            (n, t - base, byaddr.get(t - base, set()), byname.get(name, set()))
            for n, (_, _, byaddr, byname, base, _) in d.items()
            if base <= t < base + 0x1000000
        ),
        None,
    )
    valid = (
        target is not None
        and t - target[1] in [x[2] for x in libs]
        and target[1] in target[3]
    )
    assert valid, (hex(a), name, hex(t))
    print(
        f"{a:#x} {got:#x} {name} -> {t:#x} {target[0] if target else '?'} {'MATCH' if valid else 'MISMATCH'}"
    )
print(
    "\nTRACED PLT ENTRIES: address GOT symbol (original ELF; later patches may change destination)"
)
trace = (script_dir / "receipt.txt").read_text().split("reached plt {")[1].split("}")[0]
for addr in [int(x.strip(), 16) for x in trace.split(",")]:
    for n, _, base, _ in libs:
        if base <= addr < base + 0x1000000:
            try:
                got, name = lookup(n, addr)
                print(n, hex(addr), hex(got), name)
            except (AssertionError, struct.error):
                print(n, hex(addr), "not original PLT entry")
            break

"""Small synthetic tests for conformance.elf_plt's AArch64 ELF resolver."""

import struct
import tempfile
import unittest
from pathlib import Path

from conformance import elf_plt


def _elf(symbols, got_targets, relocation_targets=None):
    """Build a minimal ELF64 with one BTI/NOP-prefixed veneer per target."""
    assert len(symbols) == len(got_targets)
    relocation_targets = relocation_targets or got_targets
    assert len(symbols) == len(relocation_targets)
    string_table = bytearray(b"\0")
    symbol_entries = [bytes(24)]
    for name in symbols:
        name_offset = len(string_table)
        string_table.extend(name.encode() + b"\0")
        symbol_entries.append(struct.pack("<IBBHQQ", name_offset, 0x12, 0, 0, 0, 0))
    dynsym = b"".join(symbol_entries)

    plt = bytearray()
    relas = bytearray()
    plt_address = 0x1100
    for index, (name, got, relocation_got) in enumerate(
        zip(symbols, got_targets, relocation_targets)
    ):
        if index == 0:
            plt.extend(struct.pack("<II", 0xD503245F, 0xD503201F))  # BTI c; NOP
        else:
            plt.extend(struct.pack("<I", 0xD503201F))
        pc = plt_address + len(plt)
        page_delta = ((got & ~0xFFF) - (pc & ~0xFFF)) >> 12
        immediate = page_delta & 0x1F_FFFF
        adrp = 0x90000010 | ((immediate & 3) << 29) | (((immediate >> 2) & 0x7_FFFF) << 5)
        load_imm = (got & 0xFFF) // 8
        ldr = 0xF9400000 | (load_imm << 10) | (16 << 5) | 17
        add = 0x91000000 | ((got & 0xFFF) << 10) | (16 << 5) | 16
        plt.extend(struct.pack("<IIII", adrp, ldr, add, 0xD61F0220))  # br x17
        symbol_index = index + 1
        relas.extend(struct.pack("<QQq", relocation_got, (symbol_index << 32) | 1026, 0))

    section_names = b"\0.dynstr\0.dynsym\0.rela.plt\0.plt\0.shstrtab\0"
    name_offsets = {
        name.decode(): section_names.index(name)
        for name in (b".dynstr", b".dynsym", b".rela.plt", b".plt", b".shstrtab")
    }
    binary = bytearray(64)
    sections = []

    def add(data, align=1):
        while len(binary) % align:
            binary.append(0)
        offset = len(binary)
        binary.extend(data)
        return offset

    str_offset = add(bytes(string_table))
    sym_offset = add(dynsym, 8)
    rela_offset = add(bytes(relas), 8)
    plt_offset = add(bytes(plt), 16)
    shstr_offset = add(section_names)
    sections.append((0, 0, 0, 0, 0, 0, 0, 0, 0, 0))
    sections.append((name_offsets[".dynstr"], 3, 2, 0, str_offset, len(string_table), 0, 0, 1, 0))
    sections.append((name_offsets[".dynsym"], 11, 2, 0, sym_offset, len(dynsym), 1, 1, 8, 24))
    sections.append((name_offsets[".rela.plt"], 4, 0, 0, rela_offset, len(relas), 2, 0, 8, 24))
    sections.append((name_offsets[".plt"], 1, 6, plt_address, plt_offset, len(plt), 0, 0, 16, 0))
    sections.append((name_offsets[".shstrtab"], 3, 0, 0, shstr_offset, len(section_names), 0, 0, 1, 0))
    while len(binary) % 8:
        binary.append(0)
    shoff = len(binary)
    for section in sections:
        binary.extend(struct.pack("<IIQQQQIIQQ", *section))
    binary[:16] = b"\x7fELF" + bytes([2, 1, 1, 0, 0]) + bytes(7)
    struct.pack_into("<HHIQQQIHHHHHH", binary, 16, 3, 183, 1, 0, 0, shoff, 0, 64, 0, 0, 64, len(sections), 5)
    return bytes(binary)


class ElfPltTests(unittest.TestCase):
    def test_raw_symbol_and_prefix_entry(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "probe.so"
            path.write_bytes(_elf(["_Znative_probe"], [0x5120]))
            veneers = elf_plt.resolve_veneers(path)
            self.assertEqual(len(veneers), 1)
            self.assertEqual(veneers[0].symbol, "_Znative_probe")
            self.assertEqual(veneers[0].entry_pc, 0x1100)
            self.assertEqual(veneers[0].adrp_pc, 0x1108)
            self.assertEqual(
                elf_plt.resolve(path),
                {
                    0x1100: "_Znative_probe",
                    0x1104: "_Znative_probe",
                    0x1108: "_Znative_probe",
                },
            )

    def test_unmatched_jump_slot_fails_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "probe.so"
            path.write_bytes(_elf(["missing_veneer"], [0x5120], [0x6120]))
            with self.assertRaisesRegex(elf_plt.PltError, "no decoded veneer"):
                elf_plt.resolve(path)

    def test_add_that_clobbers_branch_target_is_not_a_plt_veneer(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "probe.so"
            binary = bytearray(_elf(["_Znative_probe"], [0x5120]))
            plt = next(s for s in elf_plt._sections(binary) if s.name == ".plt")
            add_offset = plt.offset + (0x1108 - plt.address) + 8
            add = struct.unpack_from("<I", binary, add_offset)[0]
            bad_add = (add & ~0x3FF) | (17 << 5) | 17
            struct.pack_into("<I", binary, add_offset, bad_add)
            path.write_bytes(binary)
            with self.assertRaisesRegex(elf_plt.PltError, "no decoded veneer"):
                elf_plt.resolve(path)

    def test_conflicting_relocations_for_one_got_slot_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "probe.so"
            path.write_bytes(_elf(["first", "second"], [0x5120, 0x5120]))
            with self.assertRaisesRegex(elf_plt.PltError, "conflicting symbols"):
                elf_plt.resolve(path)


if __name__ == "__main__":
    unittest.main()

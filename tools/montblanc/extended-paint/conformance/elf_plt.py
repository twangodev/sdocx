"""Resolve AArch64 ELF PLT veneer entry PCs from bytes and JUMP_SLOT RELAs.

Unlike disassembler-generated ``@plt`` labels, this maps the veneer that loads
each GOT slot to the symbol named by the ELF relocation. Symbol names remain
raw dynsym names; callers decide how to demangle them.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import struct


class PltError(ValueError):
    """The ELF has an unsupported or ambiguous PLT mapping."""


@dataclass(frozen=True)
class Section:
    name: str
    kind: int
    address: int
    offset: int
    size: int
    link: int
    entry_size: int


@dataclass(frozen=True)
class Veneer:
    entry_pc: int
    adrp_pc: int
    got_address: int
    symbol: str
    section: str


def _u32(data: bytes, offset: int) -> int:
    return struct.unpack_from("<I", data, offset)[0]


def _cstring(data: bytes, offset: int, label: str) -> str:
    if offset < 0 or offset >= len(data):
        raise PltError(f"{label}: string offset {offset:#x} outside table")
    end = data.find(b"\0", offset)
    if end < 0:
        raise PltError(f"{label}: unterminated string")
    return data[offset:end].decode("utf-8", errors="surrogateescape")


def _section_bytes(binary: bytes, section: Section) -> bytes:
    end = section.offset + section.size
    if section.offset < 0 or end > len(binary):
        raise PltError(f"{section.name}: section bytes are outside the file")
    return binary[section.offset:end]


def _sections(binary: bytes) -> list[Section]:
    if len(binary) < 64 or binary[:4] != b"\x7fELF":
        raise PltError("not an ELF file")
    if binary[4:6] != b"\x02\x01":
        raise PltError("expected little-endian ELF64")
    if struct.unpack_from("<H", binary, 18)[0] != 183:
        raise PltError("expected AArch64 ELF")
    shoff = struct.unpack_from("<Q", binary, 40)[0]
    shentsize, shnum, shstrndx = struct.unpack_from("<HHH", binary, 58)
    if shentsize < 64 or shnum == 0 or shstrndx >= shnum:
        raise PltError("unsupported ELF section table")
    if shoff + shentsize * shnum > len(binary):
        raise PltError("section table is outside the file")
    raw = [
        struct.unpack_from("<IIQQQQIIQQ", binary, shoff + index * shentsize)
        for index in range(shnum)
    ]
    shstr = raw[shstrndx]
    names = binary[shstr[4] : shstr[4] + shstr[5]]
    result = []
    for values in raw:
        name_offset, kind, _, address, offset, size, link, _, _, entry_size = values
        result.append(
            Section(
                _cstring(names, name_offset, "section name"),
                kind,
                address,
                offset,
                size,
                link,
                entry_size,
            )
        )
    return result


def _jump_slots(binary: bytes, sections: list[Section]) -> dict[int, str]:
    slots: dict[int, str] = {}
    for relocation_section in sections:
        if not relocation_section.name.startswith(".rela.plt"):
            continue
        if relocation_section.entry_size not in (0, 24):
            raise PltError(
                f"{relocation_section.name}: unexpected RELA entry size "
                f"{relocation_section.entry_size}"
            )
        if relocation_section.link >= len(sections):
            raise PltError(f"{relocation_section.name}: invalid dynsym link")
        dynsym = sections[relocation_section.link]
        if dynsym.kind != 11:
            raise PltError(f"{relocation_section.name}: linked section is not DYNSYM")
        if dynsym.link >= len(sections):
            raise PltError(f"{dynsym.name}: invalid dynstr link")
        dynstr = _section_bytes(binary, sections[dynsym.link])
        symbytes = _section_bytes(binary, dynsym)
        stride = dynsym.entry_size or 24
        if stride < 24 or len(symbytes) % stride:
            raise PltError(f"{dynsym.name}: malformed symbol table")
        relabytes = _section_bytes(binary, relocation_section)
        relastride = relocation_section.entry_size or 24
        if len(relabytes) % relastride:
            raise PltError(f"{relocation_section.name}: malformed RELA table")
        for offset in range(0, len(relabytes), relastride):
            got_address, info, _ = struct.unpack_from("<QQq", relabytes, offset)
            if info & 0xFFFF_FFFF != 1026:  # R_AARCH64_JUMP_SLOT
                continue
            symbol_index = info >> 32
            symbol_offset = symbol_index * stride
            if symbol_offset + 24 > len(symbytes):
                raise PltError(f"{relocation_section.name}: dynsym index out of range")
            name_offset = struct.unpack_from("<I", symbytes, symbol_offset)[0]
            symbol = _cstring(dynstr, name_offset, dynsym.name)
            previous = slots.get(got_address)
            if previous is not None and previous != symbol:
                raise PltError(
                    f"GOT slot {got_address:#x} has conflicting symbols "
                    f"{previous!r} and {symbol!r}"
                )
            slots[got_address] = symbol
    return slots


def _sign_extend(value: int, bits: int) -> int:
    sign = 1 << (bits - 1)
    return (value ^ sign) - sign


def _adrp(word: int, pc: int) -> tuple[int, int] | None:
    if word & 0x9F00_0000 != 0x9000_0000:
        return None
    immediate = (((word >> 5) & 0x7_FFFF) << 2) | ((word >> 29) & 3)
    page_delta = _sign_extend(immediate, 21) << 12
    return word & 31, (pc & ~0xFFF) + page_delta


def _ldr_u64(word: int) -> tuple[int, int, int] | None:
    if word & 0xFFC0_0000 != 0xF940_0000:
        return None
    target = word & 31
    base = (word >> 5) & 31
    displacement = ((word >> 10) & 0xFFF) * 8
    return target, base, displacement


def _is_nop_or_bti(word: int) -> bool:
    return word == 0xD503_201F or word in (0xD503_245F, 0xD503_249F, 0xD503_24DF)


def _is_br(word: int, register: int) -> bool:
    return word & 0xFFFF_FC1F == 0xD61F_0000 and ((word >> 5) & 31) == register


def _is_plt_add_imm(word: int, register: int) -> bool:
    return (
        word & 0x7F00_0000 == 0x1100_0000
        and word & 0x8000_0000 != 0  # 64-bit ADD
        and word & (1 << 29) == 0  # flags are not written
        and word & (1 << 22) == 0  # unshifted low-12-bit immediate
        and word & 31 == register
        and ((word >> 5) & 31) == register
    )


def _veneer_candidates(binary: bytes, section: Section) -> list[tuple[int, int, int]]:
    data = _section_bytes(binary, section)
    if len(data) % 4:
        raise PltError(f"{section.name}: instruction section size is not word-aligned")
    words = [_u32(data, offset) for offset in range(0, len(data), 4)]
    found = []
    for index, word in enumerate(words):
        pc = section.address + index * 4
        page = _adrp(word, pc)
        if page is None or index + 1 >= len(words):
            continue
        adrp_register, base_page = page
        load = _ldr_u64(words[index + 1])
        if load is None:
            continue
        target_register, load_base, displacement = load
        if load_base != adrp_register:
            continue
        branch_index = None
        for following in range(index + 2, min(index + 5, len(words))):
            candidate = words[following]
            if _is_br(candidate, target_register):
                branch_index = following
                break
            if not (
                _is_plt_add_imm(candidate, adrp_register)
                or _is_nop_or_bti(candidate)
            ):
                break
        if branch_index is None:
            continue
        found.append((pc, base_page + displacement, section.address + branch_index * 4))
    return found


def resolve_veneers(path: str | Path) -> tuple[Veneer, ...]:
    """Return byte-decoded PLT veneers, raising on missing/ambiguous mappings.

    Each veneer records the actual ADRP instruction PC and the first callable
    entry PC. BTI/NOP prefix words immediately preceding ADRP are included in
    the entry address. :func:`resolve` maps each instruction boundary in that
    prefix as well as the ADRP instruction itself.
    """
    binary = Path(path).read_bytes()
    sections = _sections(binary)
    slots = _jump_slots(binary, sections)
    if not slots:
        return ()
    plt_sections = [s for s in sections if s.name in (".plt", ".plt.sec") and s.size]
    if not plt_sections:
        raise PltError(f"{path}: JUMP_SLOT relocations exist without .plt/.plt.sec")
    veneers: list[Veneer] = []
    decoded_slots: dict[int, list[Veneer]] = {}
    for section in plt_sections:
        data = _section_bytes(binary, section)
        words = [_u32(data, offset) for offset in range(0, len(data) - 3, 4)]
        for adrp_pc, got_address, _branch_pc in _veneer_candidates(binary, section):
            symbol = slots.get(got_address)
            if symbol is None:
                continue
            index = (adrp_pc - section.address) // 4
            entry_index = index
            while entry_index > 0 and _is_nop_or_bti(words[entry_index - 1]):
                entry_index -= 1
            entry_pc = section.address + entry_index * 4
            veneer = Veneer(entry_pc, adrp_pc, got_address, symbol, section.name)
            veneers.append(veneer)
            decoded_slots.setdefault(got_address, []).append(veneer)
    missing = sorted(set(slots) - set(decoded_slots))
    if missing:
        details = ", ".join(f"{slot:#x}={slots[slot]}" for slot in missing[:8])
        raise PltError(f"{path}: no decoded veneer for {len(missing)} JUMP_SLOT(s): {details}")
    return tuple(sorted(veneers, key=lambda item: (item.entry_pc, item.adrp_pc)))


def resolve(path: str | Path) -> dict[int, str]:
    """Map callable PLT entry PCs (ELF virtual offsets) to raw dynsym names."""
    result: dict[int, str] = {}
    for veneer in resolve_veneers(path):
        # Prefix words can be direct branch targets too; retain every
        # instruction boundary from the first BTI/NOP through the ADRP.
        for address in range(veneer.entry_pc, veneer.adrp_pc + 4, 4):
            previous = result.get(address)
            if previous is not None and previous != veneer.symbol:
                raise PltError(
                    f"{path}: PLT entry {address:#x} maps to both "
                    f"{previous!r} and {veneer.symbol!r}"
                )
            result[address] = veneer.symbol
    return result

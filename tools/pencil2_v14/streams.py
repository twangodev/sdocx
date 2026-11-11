"""Synthetic Pencil2 record streams shared by maintained capture controls."""

import struct

RECORD_BYTES = 56
RECORD_COUNT = 84
STREAM_BYTES = RECORD_BYTES * RECORD_COUNT
ALPHA_OFFSET = 16
ISOLATED_ORDINALS = (17, 20, 32, 35)


def mix_records(sources):
    if len(sources) != 5 or any(len(source) < STREAM_BYTES for source in sources):
        raise ValueError("Expected five complete 84-record Pencil2 sources")
    return b"".join(sources[index % 5][index * RECORD_BYTES:(index + 1) * RECORD_BYTES]
                    for index in range(RECORD_COUNT))


def isolate_record(stream, ordinal):
    if len(stream) != STREAM_BYTES or not 1 <= ordinal <= RECORD_COUNT:
        raise ValueError("Expected one 84-record stream and a one-based record ordinal")
    isolated = bytearray(stream)
    for index in range(RECORD_COUNT):
        if index != ordinal - 1:
            struct.pack_into("<f", isolated, index * RECORD_BYTES + ALPHA_OFFSET, 0.0)
    return bytes(isolated)


def load_control_streams(root):
    conformance = root / "conformance"
    colored = (conformance / "pencil2-v14-color-80336699-native.bin").read_bytes()[:STREAM_BYTES]
    second = (conformance / "pencil2-v14-color-ffdd4411-native.bin").read_bytes()
    black = (conformance / "pencil2-v14-native.bin").read_bytes()
    sources = [colored, second, black[3 * STREAM_BYTES:4 * STREAM_BYTES],
               black[4 * STREAM_BYTES:5 * STREAM_BYTES], black[5 * STREAM_BYTES:6 * STREAM_BYTES]]
    mixed = mix_records(sources)
    return {"mixed": mixed, **{f"record_{ordinal}": isolate_record(mixed, ordinal)
                              for ordinal in ISOLATED_ORDINALS}}

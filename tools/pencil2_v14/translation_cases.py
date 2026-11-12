"""Ordered framebuffer translations for the maintained expanded-viewport control."""

from dataclasses import dataclass

from streams import ISOLATED_ORDINALS, RECORD_COUNT, isolate_record, load_control_streams

TRANSLATIONS = ((0, 0), (0, 1), (0, 2), (0, 4), (0, 8), (0, 16), (0, 32), (32, 32))


@dataclass(frozen=True)
class TranslationCase:
    """One stream offset right/down in nominal framebuffer pixels."""

    stream_name: str
    x_pixels: int
    down_pixels: int
    stream: bytes

    @property
    def receipt_key(self):
        return f"{self.x_pixels}_{self.down_pixels}_{self.stream_name}"


def load_translation_cases(root):
    mixed = load_control_streams(root)["mixed"]
    streams = {"mixed": mixed, **{f"record_{ordinal}": isolate_record(mixed, ordinal)
                                  for ordinal in range(1, RECORD_COUNT + 1)}}
    selected = {"mixed", *(f"record_{ordinal}" for ordinal in ISOLATED_ORDINALS)}
    return tuple(TranslationCase(name, x, down, stream)
                 for x, down in TRANSLATIONS
                 for name, stream in streams.items()
                 if down == 2 or name in selected)

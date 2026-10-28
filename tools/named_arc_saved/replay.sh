#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
lib=${1:?supply native library directory}
unicorn=${2:?supply Unicorn library directory}
tmp=$(mktemp -d /tmp/sdocx-arc-saved.XXXXXX)
trap 'rm -rf "$tmp"' EXIT
cd "$root"
printf '%s\n' '722441aa6e9e64dc5c275d04e77566645fa4e998bc397556099a6fa3e72d8e0a  conformance/native_table.rs' '982bab3e53c91743ce911bc929459bec2ec0afedf6ee780b11c213f985838d54  conformance/native_table/shape_paths.rs' '1f605d2770657dbeef414b7dda0dbed935d81a4ae12ed8272183174ef436fde3  tools/named_arc_saved/capture.rs' | sha256sum -c
printf '%s  %s\n' ddb196ec82b52e502c18e4a34478bf7b9f61c83c2ebaa95c74d8ded45a95da9c "$unicorn/libunicorn.so.2" | sha256sum -c
ln -s "$unicorn/libunicorn.so.2" "$tmp/libunicorn.so"
rustc --edition=2024 conformance/native_table.rs -L "$tmp" -o "$tmp/probe"
export LD_LIBRARY_PATH="$unicorn" ARC_OUTPUT="$tmp/frame"
for fill in 0 85 165 255 0; do
 ARC_FILL="$fill" "$tmp/probe" "$lib/libSPenModel.so" --named-arc-saved "$lib/libSPenBase.so" "$lib/libSPenSkia.so" 2>"$tmp/run"
 cmp crates/sdocx/tests/fixtures/native-arc-frame.bin "$tmp/frame"
 rg '^(ACTIVE |HOST_CALLS |LEDGER |reached plt )' "$tmp/run" > "$tmp/active"
 if [ -f "$tmp/baseline" ]; then cmp "$tmp/baseline" "$tmp/active"; else cp "$tmp/active" "$tmp/baseline"; fi
done
cmp tools/named_arc_saved/receipt.txt "$tmp/baseline"
printf '%s\n' 'Saved Arc native roundtrip matched across five allocation fills.'

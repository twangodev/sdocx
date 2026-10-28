#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
lib=${1:?supply native library directory}
unicorn=${2:?supply Unicorn library directory}
tmp=$(mktemp -d /tmp/sdocx-arc-drawing.XXXXXX)
trap 'rm -rf "$tmp"' EXIT
cd "$root"
printf '%s\n' '5726e44277b6b7daeddb8e75e2429580e984852cb75638092db43d8240ba00c4  conformance/native_table.rs' '4e90665effc605483a5d2589a5b85bebb016025e1250b7659b0e269e28593f0c  conformance/native_table/shape_paths.rs' '5bc6ef0b8422b929626345e54fae84df754da1b391cde3d924c8a57f451f2ee6  tools/named_arc_drawing/capture.rs' | sha256sum -c
printf '%s  %s\n' ddb196ec82b52e502c18e4a34478bf7b9f61c83c2ebaa95c74d8ded45a95da9c "$unicorn/libunicorn.so.2" | sha256sum -c
ln -s "$unicorn/libunicorn.so.2" "$tmp/libunicorn.so"
rustc --edition=2024 conformance/native_table.rs -L "$tmp" -o "$tmp/probe"
export LD_LIBRARY_PATH="$unicorn" ARC_OUTPUT="$tmp/frame"
for case in default circle tall; do
 case "$case" in default) fixture=native-arc-filled-frame.bin;; circle) fixture=native-arc-circle-frame.bin;; tall) fixture=native-arc-tall-frame.bin;; esac
 for fill in 0 85 165 255 0; do
  ARC_CASE="$case" ARC_FILL="$fill" "$tmp/probe" "$lib/libSPenModel.so" --named-arc-drawing "$lib/libSPenBase.so" "$lib/libSPenSkia.so" 2>"$tmp/run"
  cmp "crates/sdocx/tests/fixtures/$fixture" "$tmp/frame"
  rg '^(drawing |PAINT |canvas |LEDGER |HOST_CALLS |ACTIVE )' "$tmp/run" > "$tmp/record"
  cmp "tools/named_arc_drawing/$case-receipt.txt" "$tmp/record"
  rg '^reached plt ' "$tmp/run" > "$tmp/targets"
  cmp tools/named_arc_drawing/target-trace.txt "$tmp/targets"
 done
done
python3 tools/named_arc_drawing/audit.py "$lib" > "$tmp/audit"
cmp tools/named_arc_drawing/audit.txt "$tmp/audit"
printf '%s\n' 'Native Arc recording calls and effective-target audit matched.'

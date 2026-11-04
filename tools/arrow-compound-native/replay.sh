#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
lib=${1:?native arm64 library directory required}
unicorn=${2:?Unicorn library directory required}
tmp=$(mktemp -d /tmp/sdocx-arrow-compound.XXXXXX)
trap 'rm -rf "$tmp"' EXIT
cd "$root"
sha256sum -c tools/arrow-compound-native/sources.sha256
printf '%s  %s\n' ddb196ec82b52e502c18e4a34478bf7b9f61c83c2ebaa95c74d8ded45a95da9c "$unicorn/libunicorn.so.2" | sha256sum -c
ln -s "$root/conformance/native_table" "$tmp/native_table"
ln -s "$root/tools/arrow-compound-native/capture.rs" "$tmp/arrow.rs"
ln -s "$unicorn/libunicorn.so.2" "$tmp/libunicorn.so"
sed 's@native_table/shape_arrows.rs@arrow.rs@' conformance/native_table.rs > "$tmp/main.rs"
rustc --edition=2024 "$tmp/main.rs" -L "$tmp" -o "$tmp/probe"
for run in 1 2; do
 LD_LIBRARY_PATH="$unicorn" "$tmp/probe" "$lib/libSPenModel.so" --shape-arrows "$lib/libSPenDrawing.so" > "$tmp/receipt"
 cmp tools/arrow-compound-native/receipt.txt "$tmp/receipt"
done
printf '%s\n' 'Native compound dispatch controls matched twice.'

#!/bin/sh
# Build the static library for the Go binding and place it under
# rgbconsignment/lib/<GOOS>_<GOARCH>/ so that the per-platform #cgo directives
# in rgbconsignment/parse.go pick it up.
#
# Usage: scripts/build-lib.sh            # host platform
#        GOOS=linux GOARCH=arm64 scripts/build-lib.sh   # label only; cross-compiling
#                                                       # still needs a matching --target.
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

goos="${GOOS:-$(go env GOOS)}"
goarch="${GOARCH:-$(go env GOARCH)}"

# Keep the objects linkable from Go toolchains that target an older macOS SDK,
# otherwise ld prints "built for newer macOS version" for every object file.
if [ "$goos" = "darwin" ]; then
    export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"
fi

# --no-default-features drops the CLI-only deps (clap, env_logger) from the archive.
cargo build --release --lib --no-default-features ${CARGO_BUILD_TARGET:+--target "$CARGO_BUILD_TARGET"}

target_dir="${CARGO_TARGET_DIR:-target}"
src="$target_dir/${CARGO_BUILD_TARGET:+$CARGO_BUILD_TARGET/}release/librgb_consignment.a"
dest="$root/rgbconsignment/lib/${goos}_${goarch}/librgb_consignment.a"

mkdir -p "$(dirname "$dest")"
cp "$src" "$dest"
echo "copied $src -> $dest ($(du -h "$dest" | cut -f1))"

#!/bin/sh
# Build the static archive for the Go binding and place it under
# rgbconsignment/lib/<GOOS>_<GOARCH>/librgb_consignment.a, where the per-platform
# #cgo directives in rgbconsignment/parse.go expect it. The archives are
# committed, so `go get` consumers need no Rust toolchain.
#
#   scripts/build-lib.sh                         # host platform
#   scripts/build-lib.sh x86_64-apple-darwin     # cross-compile (rustup target add first)
#   scripts/build-lib.sh aarch64-unknown-linux-gnu
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

target="${1:-${CARGO_BUILD_TARGET:-}}"
if [ -n "$target" ]; then
    case "$target" in
        aarch64-apple-darwin)        label=darwin_arm64 ;;
        x86_64-apple-darwin)         label=darwin_amd64 ;;
        aarch64-unknown-linux-gnu)   label=linux_arm64 ;;
        x86_64-unknown-linux-gnu)    label=linux_amd64 ;;
        *) echo "unsupported target $target" >&2; exit 1 ;;
    esac
else
    # No explicit target: the archive is for the host, whatever GOOS/GOARCH say.
    label="$(go env GOHOSTOS)_$(go env GOHOSTARCH)"
fi

case "$label" in
    darwin_*)
        # Keep the objects linkable from Go toolchains targeting an older SDK,
        # otherwise ld warns "built for newer macOS version" per object.
        export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"
        ;;
esac

# --no-default-features drops the CLI-only deps (clap, env_logger).
# --crate-type staticlib alone lets LTO in [profile.lib] strip everything
# unreachable from the exported C ABI.
cargo rustc --profile lib --lib --no-default-features --crate-type staticlib \
    ${target:+--target "$target"}

target_dir="${CARGO_TARGET_DIR:-target}"
src="$target_dir/${target:+$target/}lib/librgb_consignment.a"
dest="$root/rgbconsignment/lib/$label/librgb_consignment.a"

mkdir -p "$(dirname "$dest")"
cp "$src" "$dest"
echo "copied $src -> $dest ($(du -h "$dest" | cut -f1))"

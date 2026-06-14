#!/usr/bin/env bash
#
# build a Zig 0.16.0 toolchain with the experimental LLVM M68k backend
#
# steps:
# 1. build llvm 21.x + clang + lld from source
# 2. build zig 0.16.0 from source against that llvm
#
# notes:
#  - zig 0.16.0 should be built against llvm 21 using release/21.x branch
#  - llvm m68k flag: -DLLVM_EXPERIMENTAL_TARGETS_TO_BUILD=M68k
#  - zig m68k flag: -Dllvm-has-m68
#
# override any default via environment variables, e.g.:
#   WORK_DIR=~/m68k LLVM_PREFIX=~/local/llvm21-m68k ./scripts/build-m68k-zig.sh

set -euo pipefail

WORK_DIR="${WORK_DIR:-$HOME/m68k-build}"
LLVM_PREFIX="${LLVM_PREFIX:-$HOME/local/llvm21-m68k}"
ZIG_PREFIX="${ZIG_PREFIX:-$HOME/local/zig-m68k}"
LLVM_BRANCH="${LLVM_BRANCH:-release/21.x}"
ZIG_VERSION="${ZIG_VERSION:-0.16.0}"
ZIG_SRC_SHA256="${ZIG_SRC_SHA256:-43186959edc87d5c7a1be7b7d2a25efffd22ce5807c7af99067f86f99641bfdf}"
LINK_JOBS="${LINK_JOBS:-2}"
BOOTSTRAP_ZIG="${BOOTSTRAP_ZIG:-zig}"

log() { printf '\n\033[32m== %s ==\033[0m\n' "$*"; }

require() {
    command -v "$1" >/dev/null 2>&1 || { echo "error: required tool '$1' not on PATH" >&2; exit 1; }
}

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}'
    else shasum -a 256 "$1" | awk '{print $1}'; fi
}

log "Checking tools"
require git
require cmake
require "$BOOTSTRAP_ZIG"
GENERATOR=()
if command -v ninja >/dev/null 2>&1; then GENERATOR=(-G Ninja); else echo "note: ninja not found, using default generator"; fi
require curl

mkdir -p "$WORK_DIR"

LLVM_SRC="$WORK_DIR/llvm-project"
LLVM_BUILD="$WORK_DIR/llvm-build"

if [ ! -f "$LLVM_SRC/llvm/CMakeLists.txt" ]; then
    log "Cloning llvm-project ($LLVM_BRANCH, shallow)"
    git clone --depth 1 --branch "$LLVM_BRANCH" https://github.com/llvm/llvm-project.git "$LLVM_SRC"
else
    log "Reusing existing llvm-project checkout at $LLVM_SRC"
fi

if [ ! -f "$LLVM_PREFIX/lib/libLLVMM68kCodeGen.a" ]; then
    log "Configuring LLVM"
    cmake "${GENERATOR[@]}" \
        -S "$LLVM_SRC/llvm" \
        -B "$LLVM_BUILD" \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_INSTALL_PREFIX="$LLVM_PREFIX" \
        -DLLVM_ENABLE_PROJECTS="lld;clang" \
        -DLLVM_EXPERIMENTAL_TARGETS_TO_BUILD=M68k \
        -DLLVM_ENABLE_DIA_SDK=OFF \
        -DLLVM_ENABLE_LIBXML2=OFF \
        -DLLVM_ENABLE_ZLIB=OFF \
        -DLLVM_ENABLE_ZSTD=OFF \
        -DLLVM_ENABLE_LIBEDIT=OFF \
        -DLLVM_INCLUDE_BENCHMARKS=OFF \
        -DLLVM_INCLUDE_EXAMPLES=OFF \
        -DLLVM_INCLUDE_TESTS=OFF \
        -DLLVM_PARALLEL_LINK_JOBS="$LINK_JOBS"

    log "Building + installing LLVM (this is the long part)"
    cmake --build "$LLVM_BUILD" --target install
else
    log "LLVM with M68k already installed at $LLVM_PREFIX; skipping"
fi

for stub in libz.a libzstd.a; do
    [ -f "$LLVM_PREFIX/lib/$stub" ] || "$LLVM_PREFIX/bin/llvm-ar" rcs "$LLVM_PREFIX/lib/$stub"
done

ZIG_TAR="$WORK_DIR/zig-$ZIG_VERSION.tar.xz"
ZIG_SRC="$WORK_DIR/zig-$ZIG_VERSION"

if [ ! -f "$ZIG_SRC/build.zig" ]; then
    if [ ! -f "$ZIG_TAR" ]; then
        log "Downloading Zig $ZIG_VERSION source"
        curl -fSL "https://ziglang.org/download/$ZIG_VERSION/zig-$ZIG_VERSION.tar.xz" -o "$ZIG_TAR"
    fi
    got="$(sha256_of "$ZIG_TAR")"
    if [ "$got" != "$ZIG_SRC_SHA256" ]; then
        echo "error: Zig source checksum mismatch" >&2
        echo "  expected $ZIG_SRC_SHA256" >&2
        echo "  got      $got" >&2
        exit 1
    fi
    log "Extracting Zig source"
    tar -xf "$ZIG_TAR" -C "$WORK_DIR"
fi

log "Building Zig with -Dllvm-has-m68k"
(
    cd "$ZIG_SRC"
    "$BOOTSTRAP_ZIG" build \
        -p "$ZIG_PREFIX" \
        --search-prefix "$LLVM_PREFIX" \
        --zig-lib-dir lib \
        -Dstatic-llvm \
        -Dllvm-has-m68k \
        -Doptimize=ReleaseFast
)

NEW_ZIG="$ZIG_PREFIX/bin/zig"
log "Smoke test: $NEW_ZIG"
"$NEW_ZIG" version
PROBE="$WORK_DIR/probe.zig"
printf 'export fn _t() callconv(.c) u32 { return 42; }\n' > "$PROBE"
"$NEW_ZIG" build-obj -target m68k-freestanding -femit-bin="$WORK_DIR/probe.o" "$PROBE"

cat <<EOF

SUCCESS.
M68k-enabled Zig: $NEW_ZIG
Build the cartridge with:  "$NEW_ZIG" build rom
Or put "$ZIG_PREFIX/bin" first on PATH to make it the default zig.
EOF

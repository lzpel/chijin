#!/bin/sh
# docker/entrypoint.sh — shared entrypoint for all docker/Dockerfile_<target> images.
#
# Runs inside the container with:
#   /src    = cadrum source tree (rw bind mount)
#   /out    = artifact output directory (rw bind mount)
#   $TARGET = rust target triple (set by the Dockerfile)
#
# Produces /out/cadrum-occt-<slug>-<TARGET>.tar.gz whose top-level directory
# is `cadrum-occt-<slug>-<TARGET>/`, matching the extraction path used by
# build.rs's prebuilt download path.

set -eu

: "${TARGET:?TARGET env var must be set by the Dockerfile}"

# 1. Extract OCCT_VERSION from build.rs (single source of truth: the const).
OCCT_VERSION=$(grep -oE 'OCCT_VERSION: &str = "[^"]+"' /src/build.rs | sed -E 's/.*"([^"]+)".*/\1/')
if [ -z "$OCCT_VERSION" ]; then
    echo "entrypoint.sh: failed to extract OCCT_VERSION from /src/build.rs" >&2
    exit 1
fi
OCCT_SLUG=$(echo "$OCCT_VERSION" | tr 'A-Z' 'a-z' | tr -d '_')

DEST="/tmp/cadrum-occt-${OCCT_SLUG}-${TARGET}"
TARBALL="/out/cadrum-occt-${OCCT_SLUG}-${TARGET}.tar.gz"

export CARGO_TARGET_DIR=/tmp/target
export OCCT_ROOT="$DEST"

echo "=== Phase 1: building OCCT from source into $DEST ==="
cd /src

# prebuilt feature is off here — we are the side that produces the tarball.
case "$TARGET" in
    *-pc-windows-msvc)
        cargo xwin build --release --no-default-features --features color --target "$TARGET"
        ;;
    *)
        cargo build --release --no-default-features --features color --target "$TARGET"
        ;;
esac

if [ ! -d "$DEST" ]; then
    echo "entrypoint.sh: expected OCCT install dir $DEST was not created" >&2
    exit 1
fi

echo "=== Phase 2: creating tarball $TARBALL ==="
mkdir -p /out
tar czf "$TARBALL" -C /tmp "cadrum-occt-${OCCT_SLUG}-${TARGET}"
ls -lh "$TARBALL"

echo "=== Phase 3: smoke test (prebuilt download path via file://) ==="
rm -rf /tmp/target "$DEST"
unset OCCT_ROOT

# Now cargo check with default features (prebuilt on). build.rs should fetch
# from the file:// URL, extract into target/cadrum-occt-<slug>-<target>/, and
# link against it without triggering a source build.
CADRUM_PREBUILT_URL="file://${TARBALL}" \
    cargo check --release --target "$TARGET" 2>&1 | tee /tmp/smoke.log

if grep -q "Building from source" /tmp/smoke.log; then
    echo "entrypoint.sh: smoke test FAILED — build.rs fell back to source build" >&2
    exit 1
fi

echo "=== entrypoint.sh: success ==="

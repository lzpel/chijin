#!/bin/bash
# docker/entrypoint.sh — thin wrapper that source-builds OCCT via cargo and
# packages the resulting target/cadrum-occt-* directory into a tarball.
#
# Inside the container:
#   /src    = cadrum source tree (rw bind mount)
#   /out    = artifact output directory (rw bind mount)
#   $TARGET = rust target triple (set by the Dockerfile)
#
# Output:
#   /out/<TARGET>.log                       — full build log
#   /out/cadrum-occt-<slug>-<TARGET>.tar.gz — prebuilt tarball

set -euo pipefail

: "${TARGET:?TARGET env var must be set by the Dockerfile}"

mkdir -p /out
LOGFILE="/out/${TARGET}.log"
: > "$LOGFILE"
exec > >(tee -a "$LOGFILE") 2>&1

cd /src

# Isolate cargo's target dir per container. /src is a bind mount shared by
# all parallel make-jobs, so writing to /src/target would serialize every
# container on cargo's file lock and occasionally cross-pollute caches.
# Using /tmp (inside the container's own writable layer) gives each
# container a private build dir and makes `make -j` actually parallel.
# build.rs reads CARGO_TARGET_DIR to decide where to install OCCT.
export CARGO_TARGET_DIR=/tmp/target-$TARGET

CARGO=(cargo)
[[ "$TARGET" == *-pc-windows-msvc ]] && CARGO=(cargo xwin)

echo "=== Building OCCT from source for $TARGET ==="
"${CARGO[@]}" build --release --no-default-features \
    --features source-build,color --target "$TARGET"

# build.rs installs OCCT into $CARGO_TARGET_DIR/cadrum-occt-<slug>-<TARGET>/.
# We don't know <slug> here — glob for the directory.
shopt -s nullglob
DIRS=("$CARGO_TARGET_DIR"/cadrum-occt-*-"$TARGET")
if [ ${#DIRS[@]} -ne 1 ]; then
    echo "entrypoint.sh: expected exactly one $CARGO_TARGET_DIR/cadrum-occt-*-$TARGET dir, found: ${DIRS[*]}" >&2
    exit 1
fi
DIR="${DIRS[0]}"
NAME="$(basename "$DIR")"
TARBALL="/out/${NAME}.tar.gz"

echo "=== Creating tarball $TARBALL from $DIR ==="
tar -czvf "$TARBALL" -C "$CARGO_TARGET_DIR" "$NAME"
ls -lh "$TARBALL"
echo "=== entrypoint.sh: success ==="

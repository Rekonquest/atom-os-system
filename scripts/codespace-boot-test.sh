#!/usr/bin/env bash
set -euo pipefail

SYSROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD=/workspaces/atom-os-build
BOOT_COMMIT=b8540ed

command -v qemu-system-x86_64 >/dev/null 2>&1 || {
    sudo apt-get update && sudo apt-get install -y qemu-system-x86
}
cargo bootimage --version >/dev/null 2>&1 || cargo install bootimage
rustup toolchain list | grep -q '^nightly' || \
    rustup toolchain install nightly --component rust-src llvm-tools-preview --profile minimal

rm -rf "$BUILD"
mkdir -p "$BUILD"
cd "$BUILD"

git clone --quiet https://github.com/Rekonquest/atom-os-field-substrate.git "ATOM OS"
git clone --quiet https://github.com/Lucerna-Labs/atom-os-kernel.git
git -C atom-os-kernel checkout --quiet "$BOOT_COMMIT"
git -C atom-os-kernel apply "$SYSROOT/patches/0001-int80-yield-use-switch-result.patch"

for p in hello daemon shell fieldmon; do
    (cd "$SYSROOT/programs/$p" && cargo +nightly build -Zjson-target-spec --release --quiet)
done

REL="$BUILD/atom-os-kernel/target/x86_64-os/release"
mkdir -p "$REL"
cp "$SYSROOT/target/x86_64-atom-user/release/shell" "$REL/payload"
cp "$SYSROOT/target/x86_64-atom-user/release/daemon" "$REL/daemon"

(cd "$BUILD/atom-os-kernel/x86_64-kernel" && cargo +nightly bootimage -Zjson-target-spec --release)

cd "$BUILD/atom-os-kernel"
timeout 30 qemu-system-x86_64 \
    -drive format=raw,file=target/x86_64-os/release/bootimage-x86_64-kernel.bin \
    -display none \
    -serial file:qemu.log || true

echo "==== SERIAL LOG (first 4000 bytes) ===="
head -c 4000 qemu.log || true
echo
echo "==== GATES ===="
pass=0
if grep -q "ATOM OS System shell" qemu.log; then echo "BANNER_OK"; pass=1; else echo "BANNER_MISSING"; fi
if grep -q "10,000 SYS_YIELDs took" qemu.log; then echo "BENCH_OK"; pass=1; else echo "BENCH_MISSING"; fi
if grep -q "Daemon" qemu.log; then echo "DAEMON_OK"; else echo "DAEMON_MISSING (non-fatal)"; fi
[ "$pass" = 1 ] && echo "RUNTIME_SMOKE_PASS" || { echo "RUNTIME_SMOKE_FAIL"; exit 1; }

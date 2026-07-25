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
git -C atom-os-kernel apply "$SYSROOT/patches/0002-exec-reset-cr3-and-inject-extra-elfs.patch"

for p in hello daemon shell fieldmon; do
    (cd "$SYSROOT/programs/$p" && cargo +nightly build -Zjson-target-spec --release --quiet)
done

REL="$BUILD/atom-os-kernel/target/x86_64-os/release"
mkdir -p "$REL"
cp "$SYSROOT/target/x86_64-atom-user/release/shell" "$REL/payload"
cp "$SYSROOT/target/x86_64-atom-user/release/daemon" "$REL/daemon"
cp "$SYSROOT/target/x86_64-atom-user/release/hello" "$REL/hello"
cp "$SYSROOT/target/x86_64-atom-user/release/fieldmon" "$REL/fieldmon"

(cd "$BUILD/atom-os-kernel/x86_64-kernel" && cargo +nightly bootimage -Zjson-target-spec --release)

cd "$BUILD/atom-os-kernel"
rm -f qemu.log qmp.sock
qemu-system-x86_64 \
    -drive format=raw,file=target/x86_64-os/release/bootimage-x86_64-kernel.bin \
    -display none \
    -serial file:qemu.log \
    -qmp unix:qmp.sock,server,nowait &
QEMU_PID=$!

for i in $(seq 1 60); do
    if [ -S qmp.sock ] && grep -q 'HEAP_OK' qemu.log 2>/dev/null \
       && grep -q '10,000 SYS_YIELDs took' qemu.log 2>/dev/null; then
        break
    fi
    sleep 0.5
done

python3 - <<'PY'
import json, socket, time

def qmp(cmd):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect("qmp.sock")
    s.settimeout(5)
    s.recv(4096)
    s.sendall(b'{"execute":"qmp_capabilities"}\n')
    s.recv(4096)
    payload = {"execute": cmd} if isinstance(cmd, str) else cmd
    s.sendall((json.dumps(payload) + "\n").encode())
    s.recv(4096)
    s.close()

def sendkey(key):
    qmp({"execute": "send-key", "arguments": {
        "keys": [{"type": "qcode", "data": key}]
    }})
    time.sleep(0.08)

for k in list("help") + ["ret"]:
    sendkey(k)
time.sleep(1.0)
for k in list("ls") + ["ret"]:
    sendkey(k)
time.sleep(1.0)
for k in list("run hello.elf") + ["ret"]:
    if k == " ":
        sendkey("spc")
    elif k == ".":
        sendkey("dot")
    else:
        sendkey(k)
time.sleep(3.0)
PY

sleep 2
kill "$QEMU_PID" 2>/dev/null || true
wait "$QEMU_PID" 2>/dev/null || true

echo "==== SERIAL LOG (first 4000 bytes) ===="
head -c 4000 qemu.log || true
echo
echo "==== GATES ===="
fail=0
if grep -q "ATOM OS System shell" qemu.log; then echo "BANNER_OK"; else echo "BANNER_MISSING"; fail=1; fi
if grep -q "HEAP_OK" qemu.log; then echo "HEAP_OK"; else echo "HEAP_MISSING"; fail=1; fi
if grep -q "10,000 SYS_YIELDs took" qemu.log; then echo "BENCH_OK"; else echo "BENCH_MISSING"; fail=1; fi
if grep -q "Daemon" qemu.log; then echo "DAEMON_OK"; else echo "DAEMON_MISSING (non-fatal)"; fi
if grep -q "commands:" qemu.log; then echo "KEYBOARD_OK"; else echo "KEYBOARD_MISSING"; fail=1; fi
if grep -q "hello.elf" qemu.log && grep -q "fieldmon.elf" qemu.log; then echo "RAMFS_OK"; else echo "RAMFS_MISSING"; fail=1; fi
if grep -q "hello from ATOM OS System" qemu.log; then echo "EXEC_OK"; else echo "EXEC_MISSING"; fail=1; fi
[ "$fail" = 0 ] && echo "RUNTIME_SMOKE_PASS" || { echo "RUNTIME_SMOKE_FAIL"; exit 1; }

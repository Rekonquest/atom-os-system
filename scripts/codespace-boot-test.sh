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
git -C atom-os-kernel apply "$SYSROOT/patches/0003-ipc-byte-mailboxes-and-frame-recv-pages.patch"
git -C atom-os-kernel apply "$SYSROOT/patches/0004-xmm-save-restore-in-interrupt-wrappers.patch"
git -C atom-os-kernel apply "$SYSROOT/patches/0005-sys-spawn-runtime-process-creation.patch"
git -C atom-os-kernel apply "$SYSROOT/patches/0006-reap-spawned-task-resources-on-exit.patch"
git -C atom-os-kernel apply "$SYSROOT/patches/0007-free-exec-replaced-address-spaces.patch"

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
for k in list("msg hello atom") + ["ret"]:
    if k == " ":
        sendkey("spc")
    else:
        sendkey(k)
time.sleep(2.5)
for k in list("spawn hello.elf") + ["ret"]:
    if k == " ":
        sendkey("spc")
    elif k == ".":
        sendkey("dot")
    else:
        sendkey(k)
time.sleep(2.5)
# Second spawn: the first hello has exited, so its slot is Terminated and
# the scheduler reaps it (patch 0006) before reuse — the REAP/LEAK gates
# below read the diagnostics this prints on serial.
for k in list("spawn hello.elf") + ["ret"]:
    if k == " ":
        sendkey("spc")
    elif k == ".":
        sendkey("dot")
    else:
        sendkey(k)
time.sleep(2.5)
# Patch 0007: exec fieldmon so EXECFREE fires with spawned children
# still holding the old root as baseline (the rebaseline path).
for k in list("run fieldmon.elf") + ["ret"]:
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
if grep -q "fieldmon: observing" qemu.log; then echo "FIELD_EXEC_OK"; else echo "FIELD_EXEC_MISSING"; fail=1; fi
efreed=$(grep -o "EXECFREE pid=[0-9]* freed=[0-9]*" qemu.log | head -1 | sed "s/.*freed=//")
edefer=$(grep -o "EXECFREE pid=[0-9]* freed=[0-9]* deferred=[0-9]*" qemu.log | head -1 | sed "s/.*deferred=//")
if [ -n "$efreed" ] && [ "$efreed" -ge 8 ] && [ "${edefer:-X}" = "0" ]; then echo "EXECFREE_OK (freed=$efreed deferred=$edefer)"; else echo "EXECFREE_BAD (freed=${efreed:-none} deferred=${edefer:-none})"; fail=1; fi
if grep -q "Received IPC: hello atom" qemu.log; then echo "IPC_OK"; else echo "IPC_MISSING (non-fatal — daemon RECV-cycle timing)"; fi
if grep -q "spawned pid" qemu.log && [ "$(grep -c "hello from ATOM OS System" qemu.log)" -ge 2 ]; then echo "SPAWN_OK"; else echo "SPAWN_MISSING"; fail=1; fi
if grep -q "REAP pid=" qemu.log; then echo "REAP_OK"; else echo "REAP_MISSING"; fail=1; fi
freed=$(grep -o "REAP pid=[0-9]* freed=[0-9]*" qemu.log | head -1 | sed "s/.*freed=//")
if [ -n "$freed" ] && [ "$freed" -ge 8 ]; then echo "REAP_FRAMES_OK (freed=$freed)"; else echo "REAP_FRAMES_LOW (freed=${freed:-none})"; fail=1; fi
# Exact leak-closed invariant: REAP prints the free count right after
# returning the dead task's frames; the LAST SPAWN line is the spawn whose
# slot reuse performed that reap, its free_before captured just before its
# own allocations — so equality proves the reap returned precisely what a
# spawn consumes.
before=$(grep -o "SPAWN pid=[0-9]* free_before=[0-9]*" qemu.log | tail -1 | sed "s/.*free_before=//")
after=$(grep -o "REAP pid=[0-9]* freed=[0-9]* free=[0-9]*" qemu.log | tail -1 | sed "s/.*free=//")
if [ -n "$before" ] && [ "$after" = "$before" ]; then echo "LEAK_CLOSED (free_before=$before free_after_reap=$after)"; else echo "LEAK_OPEN (free_before=${before:-none} free_after_reap=${after:-none})"; fail=1; fi
[ "$fail" = 0 ] && echo "RUNTIME_SMOKE_PASS" || { echo "RUNTIME_SMOKE_FAIL"; exit 1; }

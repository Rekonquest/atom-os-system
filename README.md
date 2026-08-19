# ATOM OS System

Userspace system for the Atom OS kernel (`C:\Projects\atom-os-kernel`).
Zero external dependencies — `core` + `alloc` only.

## Status (read this first)

**Boot-tested end-to-end: banner, heap, bench, keyboard, RamFS inject, `run`/EXEC.**
The workflow `.github/workflows/boot-test.yml` clones
`atom-os-kernel@b8540ed` + the field substrate, applies the patches under
`patches/`, builds this userspace from source, embeds it via the kernel's
`include_bytes!` paths, builds the bootimage, boots QEMU, injects PS/2
keystrokes over QMP, and greps serial output.

Gates: `BANNER_OK`, `HEAP_OK`, `BENCH_OK`, `DAEMON_OK`, `KEYBOARD_OK`,
`RAMFS_OK`, `EXEC_OK`, `IPC_OK`, `SPAWN_OK`, `REAP_OK`,
`REAP_FRAMES_OK`, `LEAK_CLOSED` — all twelve green (2026-08-19,
KVM-verified): `run hello.elf` execs, prints `hello from ATOM OS
System`, and exits cleanly while the daemon keeps running; `spawn`
runs a second process beside the shell; and the second spawn reaps
the exited task's frames — the allocator free-count returns exactly
to its pre-spawn value (patch 0006). The old runtime #GP was never
kernel-side: the `program!` macro's `_start` was an ordinary Rust fn
compiled assuming `rsp ≡ 8 (mod 16)` at entry (the CALL convention),
while iretq process entry starts with `rsp ≡ 0` — every stack-slot
alignment downstream was off by 8, and the first rsp-relative `movaps`
(zeroing the path buffer in `sys::exec`) faulted with #GP(0). Fixed by
making `_start` an asm stub that CALLs the Rust entry. Patch 0002's
exec machinery (frame reset, CR3 reload, page-table path unsharing) is
verified correct.

**Required kernel patches** (applied by the workflow after checkout):

1. `patches/0001-int80-yield-use-switch-result.patch` — int-0x80 handler
   must use `switch_context`'s return value; without it pid 1 is starved.
2. `patches/0002-exec-reset-cr3-and-inject-extra-elfs.patch` — SYS_EXEC
   resets the user trap frame (rsp/GPRs), unshares the page-table path
   before `map_segment`, and reloads CR3 on syscall return; RamFS also
   injects `hello.elf` / `fieldmon.elf`.
3. `patches/0003-ipc-byte-mailboxes-and-frame-recv-pages.patch` — IPC
   delivers message bytes via per-pid mailboxes alongside the field's
   energy transport; RECV pages come from one cached frame per pid
   mapped at `0x00007F0000000000` (fixes the per-RECV page leak and the
   reserved-bit / supervisor-only #PFs of the old 0x300000 slab page).
4. `patches/0004-xmm-save-restore-in-interrupt-wrappers.patch` — the
   timer/syscall/keyboard wrappers save the interrupted task's 16 XMM
   registers below the trap frame (movdqu chain — kernel stacks are
   align-1, so FXSAVE cannot be used) and restore the resumed task's
   state before iretq. The kernel never touches MXCSR/AVX, so the XMM
   regs are the full user-visible SSE state.
5. `patches/0005-sys-spawn-runtime-process-creation.patch` — SYS_SPAWN
   (21): loads a RamFS ELF as a brand-new task (own pid, kernel stack,
   trap frame, private address space) while the caller keeps running.
   The EXEC image-loading code is extracted into `load_user_image` and
   shared by both syscalls; the scheduler reclaims Terminated slots and
   hands out the lowest free pid 1..15.
6. `patches/0006-reap-spawned-task-resources-on-exit.patch` — when a
   Terminated slot is reused, the dead task's resources return to the
   frame allocator: its frame-backed kernel stack, image span, user
   stack, and every private page-table frame (diff-walk against the
   parent CR3; shared subtrees, leaf data frames, and cached IPC RECV
   frames untouched). Spawned kernel stacks move from the un-freeable
   kernel heap to the frame allocator, and the redundant second
   `unshare_page_path` in the loader (same 2 MiB path as the first)
   stops orphaning three frames per image load. Address spaces replaced
   by EXEC still leak (documented in ARCHITECTURE.md).

These patches are not yet landed on `atom-os-kernel` itself (except the
yield fix on HEAD); applying them here is the integration path.

Remaining caveats: IPC also transports `len * 0.02` energy through the
field (by design — bytes and energy both flow), and `run` replaces the
shell process, so pid 1 ends when the exec'd program exits (exec
semantics, not a crash).

## What this is

A userspace layer that replaces the kernel's monolithic `payload` shell
and `daemon` with a runtime crate (`atom-rt`) and thin programs on top.
All syscall asm is confined to one module (`atom-rt::sys`); everything
else is pure Rust that unit-tests on the host.

```
crates\atom-rt      no_std userspace runtime (syscalls, io, fmt, heap, parser)
programs\hello      smoke program
programs\daemon     IPC daemon (replaces kernel's daemon)
programs\shell      command shell (replaces kernel's payload)
programs\fieldmon   field-substrate observer (syscalls 17-20)
scripts\build.ps1   build all programs, stage image\*.elf
scripts\test.ps1    host unit tests for atom-rt
scripts\check-elf.ps1   static ELF header checks vs loader requirements
scripts\install-to-kernel.ps1   drop ELFs where the kernel embeds them
```

## Build

```powershell
scripts\build.ps1
scripts\check-elf.ps1   # static loader-compatibility gate
```

Produces `image\hello.elf`, `image\daemon.elf`, `image\shell.elf`,
`image\fieldmon.elf` — x86_64 ET_EXEC ELFs linked at
`0xFFFFFFFF80100000` (the kernel's userspace link address).

## Test

```powershell
scripts\test.ps1
```

Host unit tests for the pure parts of atom-rt (u64/fixed-6 formatting,
command tokenizer, bump-allocator pointer math). These tests exercise
**no** syscall, **no** I/O, and none of the programs' logic.

## Boot (GitHub Actions — the proven path)

Push to `main` or dispatch the `Boot Test` workflow; it reproduces the
verified boot end-to-end. A Codespaces variant of the same flow lives in
`scripts/codespace-boot-test.sh`.

## Boot (local, operator action)

With patch `0002` applied, the kernel embeds binaries at
`target\x86_64-os\release\{payload,daemon,hello,fieldmon}` via
`include_bytes!` and injects them as `shell.elf` / `daemon.elf` /
`hello.elf` / `fieldmon.elf` into the RamFS.

```powershell
scripts\install-to-kernel.ps1          # dry run
scripts\install-to-kernel.ps1 -Force   # copy
# then rebuild ONLY the x86_64-kernel crate and boot QEMU
```

Expected on first boot: shell banner, `HEAP_OK`, automatic boot bench
printing `10,000 SYS_YIELDs took (CPU cycles): N`, `> ` prompt, daemon
heartbeat. From the prompt: `help`, `ls` (shows all four ELFs),
`run hello.elf` (replaces the shell; prints hello then exits).

Caveats:

- `run <file.elf>` **replaces the shell process** (exec semantics) —
  the shell does not come back.
- If cargo rebuilds the kernel's own `payload`/`daemon` crates it
  overwrites the installed files; build only `x86_64-kernel`.
- IPC is energy-magnitude transport, not message bytes: the shell's
  `msg` text is discarded kernel-side and the daemon prints a truncated
  integer magnitude (usually `0`). See ARCHITECTURE.md.

## Shell commands

```
help              show commands
ls                list RamFS
clear             clear screen
cat <file>        print file
edit <file>       append-only line editor ('.' saves; files >1024 B refused)
echo <text> > <f> append text to file
msg <text>        send IPC to daemon (pid 2) — see IPC caveat above
run <file.elf>    exec REPLACES the shell
spawn <file.elf>  run as a new process; shell stays alive
bench             10,000 SYS_YIELD cycle count (also runs once at boot)
xtest             XMM liveness probe (baseline/syscall/preempt)
```

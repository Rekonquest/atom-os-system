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
`RAMFS_OK`, `EXEC_OK`.

**Required kernel patches** (applied by the workflow after checkout):

1. `patches/0001-int80-yield-use-switch-result.patch` — int-0x80 handler
   must use `switch_context`'s return value; without it pid 1 is starved.
2. `patches/0002-exec-reset-cr3-and-inject-extra-elfs.patch` — SYS_EXEC
   resets the user trap frame (rsp/GPRs) and the syscall return path
   loads the new CR3; RamFS also injects `hello.elf` / `fieldmon.elf`.

These patches are not yet landed on `atom-os-kernel` itself (except the
yield fix on HEAD); applying them here is the integration path.

Remaining kernel-side caveats: IPC is energy-magnitude transport (not
message bytes), IPC_RECV page leak, XMM clobber across syscalls,
fieldmon still terminates pid 1 on exit after `run`.

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
bench             10,000 SYS_YIELD cycle count (also runs once at boot)
```

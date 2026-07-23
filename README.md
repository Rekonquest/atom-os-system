# ATOM OS System

Userspace system for the Atom OS kernel (`C:\Projects\atom-os-kernel`).
Zero external dependencies — `core` + `alloc` only.

## Status (read this first)

**Boot-tested on GitHub Actions (2026-07-23, run 30032503929): PASS.**
The workflow `.github/workflows/boot-test.yml` clones
`atom-os-kernel@b8540ed` + the field substrate, builds this userspace
from source, embeds it via the kernel's `include_bytes!` paths, builds
the bootimage, boots QEMU, and greps serial output. Observed on the
passing run:

```
Booting Fearless Hypatia...
Field substrate Initialized.
Ring 3 Multi-Tasking Spawned.
ATOM OS System shell (type 'help')
10,000 SYS_YIELDs took (CPU cycles): 177274864
> [Daemon] Heartbeat... [Daemon] Heartbeat...
```

Gates: `BANNER_OK`, `BENCH_OK`, `DAEMON_OK` — shell (pid 1) and daemon
(pid 2) both schedule and produce output.

**Required kernel patch:** `patches/0001-int80-yield-use-switch-result.patch`
(applied by the workflow after checkout). Without it, the kernel's
int-0x80 handler discards `switch_context`'s return value
(`x86_64-kernel/src/main.rs:416` @ b8540ed): the CPU always resumes the
yielding task while scheduler bookkeeping advances, so pid 1 is starved
forever — the shell never prints. This was found by this project's
runtime test and is a kernel-side bug, not a userspace one. The patch
is not yet applied to `atom-os-kernel` itself; landing it there is an
operator action.

Remaining unproven surfaces: interactive keyboard input (CI injects
none), `run`/SYS_EXEC (kernel-side CR3/register reset gap),
`hello.elf`/`fieldmon.elf` exec from RamFS (kernel injects only
shell/daemon), the `.bss` heap path (no shipped program allocates).

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
verified boot end-to-end in ~6 minutes. A Codespaces variant of the
same flow lives in `scripts/codespace-boot-test.sh`.

## Boot (local, operator action)

The unmodified kernel embeds whatever binaries sit at
`target\x86_64-os\release\{payload,daemon}` via `include_bytes!` and
injects them as `shell.elf` / `daemon.elf` into the RamFS.

```powershell
scripts\install-to-kernel.ps1          # dry run
scripts\install-to-kernel.ps1 -Force   # copy
# then rebuild ONLY the x86_64-kernel crate and boot QEMU
```

Expected on first boot (predictions, not results): shell banner,
automatic boot bench printing `10,000 SYS_YIELDs took (CPU cycles): N`
(preserves the kernel CI's serial grep), `> ` prompt, daemon heartbeat.

Caveats:

- The stock kernel injects only `shell.elf` and `daemon.elf` into the
  RamFS; `hello.elf` and `fieldmon.elf` cannot be exec'd until the
  kernel injects more files (kernel-side change, out of scope).
- `run daemon.elf` **replaces the shell process** (exec semantics) —
  the shell does not come back. See the CR3 blocker above.
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
run <file.elf>    exec REPLACES the shell (see blockers above)
bench             10,000 SYS_YIELD cycle count (also runs once at boot)
```

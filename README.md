# ATOM OS System

Userspace system for the Atom OS kernel (`C:\Projects\atom-os-kernel`).
Zero external dependencies — `core` + `alloc` only.

## Status (read this first)

**Statically checked, never booted.** Everything below has passed host
unit tests and static ELF header/program-header checks against the
kernel loader's requirements. **No binary from this project has ever
executed on the kernel.** There is no QEMU smoke gate yet. Words like
"works" are earned by `scripts\install-to-kernel.ps1 -Force` + a kernel
rebuild + a QEMU boot with serial capture — none of which has happened.

Known kernel-side blockers that this userspace cannot fix (found by
adversarial review, 2026-07-23):

- **SYS_EXEC does not reload CR3 or reset registers on the int-0x80
  path** — `run <file>` from the shell is unreliable until the kernel
  handler is fixed (kernel-orchestrator `syscall.rs:316`,
  x86_64-kernel `main.rs:488-508`).
- **The int-0x80 YIELD path discards `switch_context`'s return value**
  (`main.rs:491`), a scheduler state desync in the same family as the
  old GAP-5 bug. The daemon issues 500k yields per loop and stresses
  this path hard.

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

## Boot (operator action, never yet performed)

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

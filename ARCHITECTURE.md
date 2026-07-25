# ATOM OS System — Architecture

Userspace counterpart to `atom-os-kernel`. Companion documents:
`C:\Projects\ATOM OS\ARCHITECTURE.md` (field substrate bridge, read-only
source workspace) and the kernel's own `ATOM-STACK-KERNEL-DESIGN.md`.

## Evidence status

| Layer | Evidence |
|---|---|
| fmt / shell_parse / heap (pure) | 16 host unit tests pass |
| ELF output vs loader requirements | static header/PHDR checks (`scripts\check-elf.ps1`) pass |
| Boot: kernel init, shell banner, HEAP_OK, auto-bench, prompt, daemon heartbeat | QEMU boot + serial gates |
| Interactive keyboard (`help`) | QEMU QMP `send-key` → `KEYBOARD_OK` |
| RamFS inject of hello.elf / fieldmon.elf | `ls` → `RAMFS_OK` (requires patch 0002) |
| `run hello.elf` / SYS_EXEC | still #GPs at runtime in stock kernel; patch 0002 is partial |
| `.bss` heap path on target | shell allocates `Vec` at boot → `HEAP_OK` |

## Layers

```
programs\shell, daemon, hello, fieldmon      policy (thin)
crates\atom-rt                               mechanism
  sys.rs        all int 0x80 asm, typed wrappers     (cfg target_os = "none")
  io.rs         print/read_line over sys             (cfg target_os = "none")
  fmt.rs        u64 / fixed-6 float formatting       (pure, host-tested)
  shell_parse.rs tokenizer + trim                    (pure, host-tested)
  heap.rs       128 KiB .bss bump allocator          (host-tested + boot HEAP_OK)
  program!      macro: panic handler, global allocator, _start
```

## Syscall ABI (cross-checked by reading, not by execution)

`int 0x80`, `rax` = number, args `rdi`, `rsi`, return in `rax`.
Register contract: the kernel's int-0x80 wrapper pushes/pops all 15
GPRs and iretq restores RFLAGS, so `options(nostack, preserves_flags)`
matches the proven payload pattern. XMM registers are NOT saved by the
kernel trap frame while the dispatch path does f32 math — userspace
floats held live across a syscall may be clobbered (kernel-side gap,
educated guess pending disassembly).

| # | Name | Args | Returns |
|---|------|------|---------|
| 1 | YIELD | — | 0 |
| 2 | ALLOC | — | phys frame addr (unused; heap is .bss) |
| 3 | EXIT | — | never returns |
| 4 | READ | — | ascii byte or 0 (none ready) |
| 5 | WRITE | rdi=byte | 1 |
| 6 | OPEN | rdi=NUL-terminated path | fd or u64::MAX (creates if missing) |
| 7 | READ_FILE | rdi=fd | byte or u64::MAX (EOF) |
| 8 | WRITE_FILE | rdi=fd, rsi=byte | 1 or u64::MAX |
| 9 | CLOSE | rdi=fd | 0 or u64::MAX |
| 10 | LIST_DIR | — | prints to VGA, 0 |
| 11 | CLEAR | — | 0 |
| 12 | TRUNCATE | rdi=fd | 0 or u64::MAX |
| 13 | EXEC | rdi=NUL-terminated path | replaces caller; u64::MAX on failure |
| 14 | PRINT | rdi=byte | serial only, 0 |
| 15 | IPC_SEND | rdi=target pid, rsi=NUL-terminated msg | 0 or u64::MAX |
| 16 | IPC_RECV | — | 0 (nothing) or vaddr 0x300000 |
| 17 | FIELD_STIMULATE | rdi=pid, rsi=f32 magnitude bits | 0 or u64::MAX |
| 18 | FIELD_EVOLVE | rdi=moments | new field age |
| 19 | FIELD_OBSERVE | rdi=pid | trace-peak delta, f32 bits |
| 20 | FIELD_MEASUREMENTS | — | introduced energy, f64 bits |

### IPC is not message passing

The kernel's field-rewired IPC (15/16) discards the message bytes and
transports only `len * 0.02` energy. RECV maps a page at `0x300000`
containing `delta as u64` as ASCII — the fractional part is truncated,
so small messages render as `0`. The daemon printing
`[Daemon] Received IPC: 0` for every message is the expected behavior
of the current kernel, not a userspace bug.

Kernel-side hazards documented for future patch work:

- Every ready RECV leaks a 4 KiB kernel-heap page (never freed).
- `atom_rt::sys::ipc_recv` returns `&'static [u8]` over the `0x300000`
  mapping; a later RECV remaps the same vaddr to a new page, so holding
  a slice across another RECV is unsound. Current daemon drops the
  slice before its next recv — do not change that without fixing the
  API.
- FIELD_OBSERVE (19) shares its baseline with IPC_RECV (16): observing
  a pid consumes that pid's pending IPC delta. fieldmon observes only
  its own pid for this reason.
- "Field-driven scheduler" is documented in kernel comments but not
  wired: `scheduler_pick` has no call sites; scheduling is round-robin.

## Memory model

Each program links at `0xFFFFFFFF80100000`; the kernel maps an 8 KiB
user stack just below it. `SYS_ALLOC` is unused by programs (returns a
physical address userspace cannot map); prefer `atom_rt::sys::alloc_frame`
only for ABI experiments.

Heap: the 128 KiB bump-allocator arena lives in `.bss`. The shell
allocates a small `Vec` at boot (`HEAP_OK`) so the shipped `shell.elf`
retains a NOBITS segment and the kernel loader's zero-fill path
(`memsz > filesz`) is exercised. Prefer `ipc_recv_into` over the legacy
`ipc_recv` `&'static` view of the remapped IPC page.

## Processes

The kernel will spawn pid 1 = `shell.elf`, pid 2 = `daemon.elf` after
install + rebuild. With patch 0002, RamFS also contains `hello.elf` and
`fieldmon.elf` for `run`. `MAX_TASKS = 16` (scheduler) and `MAX_PIDS = 16`
(field glue) are coincidentally equal constants with no shared source.
`SYS_EXEC` rewrites the caller in place (same pid, same field site):
`run` replaces the shell permanently. Patch 0002 resets the user trap
frame (`rsp` to the stack top, zeroed GPRs) and loads the new CR3 on
the syscall return path so runtime `run` matches the initial spawn ABI.

## Known userspace limitations

- `edit` is append-only (no modifying existing content) and refuses
  files >1024 bytes rather than truncating them.
- Shell echoes per keystroke and heartbeat text from the daemon can
  interleave mid-line on the shared VGA.
- `msg` longer than 254 bytes fails; `read_line` accepts 1024.
- `fieldmon` hardcodes pid 1 (correct only via the exec-replace path)
  and would terminate pid 1 on exit — runnable via `run fieldmon.elf`
  once patch 0002 injects it into RamFS.
- Boot-time field behavior is tick-phased, so serial output is not
  byte-reproducible across runs; `bench` prints raw rdtsc deltas.

## Design rules

1. Zero external crates, everywhere. `core` + `alloc` only.
2. asm/unsafe confined to `atom-rt::sys`; programs contain none.
3. Pure logic is host-tested; target-only code is thin wrappers.
4. No writes to `C:\Projects\ATOM OS` (read-only source workspace).
   Kernel source is not edited in-tree; required fixes ship as
   `patches/*.patch` and are applied by the boot-test workflow onto
   the pinned kernel commit. Integration binaries still land via the
   kernel's `include_bytes!` paths (`payload`/`daemon`/`hello`/`fieldmon`).
5. No capability is claimed "working" without runtime evidence.

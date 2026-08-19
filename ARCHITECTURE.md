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
| `run hello.elf` / SYS_EXEC | QEMU boot + serial gate `EXEC_OK`: execs, prints, exits; daemon survives (2026-08-18). Prior #GP was userspace `_start` stack misalignment, fixed in atom-rt; patch 0002 verified correct |
| `.bss` heap path on target | shell allocates `Vec` at boot → `HEAP_OK` |
| IPC byte delivery (`msg hello atom`) | QEMU QMP keys → `[Daemon] Received IPC: hello atom` (requires patch 0003; 2026-08-18) |
| `run fieldmon.elf` field observation | 10 observe rounds print, clean exit, daemon survives (2026-08-18) |
| XMM liveness across syscalls/preemption | `xtest` probe: XMM_OK baseline/syscall/preempt×3 with patch 0004; same probe caught clobbers without it (2026-08-18) |
| `spawn hello.elf` / SYS_SPAWN | `spawned pid 3`, hello prints, shell prompt returns; pid/slot reclaimed on second spawn; fieldmon spawns, runs 10 rounds, exits, shell survives (patch 0005; 2026-08-18) |
| Reap of exited tasks' resources | Gates `REAP_OK` / `REAP_FRAMES_OK` / `LEAK_CLOSED`: a second spawn reaps the dead task's slot — 8 frames return (private page tables, kernel stack, image span, user stack) and the allocator free-count returns exactly to the spawning spawn's pre-allocation value; fieldmon reaps identically (patch 0006; 2026-08-19) |
| Free of EXEC-replaced address spaces | Gates `EXECFREE_OK` / `FIELD_EXEC_OK`: `run fieldmon.elf` with two spawned children still holding the shell's root as baseline frees exactly 42 frames (PML4 + 3 private-chain + 36 image span + 2 user stack — matches the shell ELF's PT_LOAD span arithmetic), defers nothing, rebaselines the children, and fieldmon then runs 10 field rounds on the new root (patch 0007; 2026-08-19) |
| IPC mailbox queueing | Gate `IPC_QUEUE_OK`: two messages sent back-to-back (both in the daemon's mailbox before its next RECV cycle) are both delivered, in order — `Received IPC: alpha one` then `Received IPC: beta two` in the focused run; the pre-0008 single-slot mailbox lost the first (patch 0008; 2026-08-19) |

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
matches the proven payload pattern. XMM registers ARE preserved across
kernel entry since patch 0004: the timer/syscall/keyboard wrappers save
the interrupted task's 16 XMM regs below the trap frame and restore the
resumed task's state before iretq (movdqu chains — kernel stacks are
align-1 bump allocations, so FXSAVE's 16-byte alignment cannot be
assumed; the kernel never touches MXCSR/AVX, so XMM is the full
user-visible SSE state). Verified by the shell's `xtest` probe: before
the patch it caught kernel field-evolve working values leaking into
xmm0 on both the syscall and preemption paths; after, baseline,
100k-yield syscall, and ~2 s preempt rounds all stay clean.

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
| 16 | IPC_RECV | — | 0 (nothing) or RECV-page vaddr (0x00007F0000000000 since patch 0003) |
| 21 | SPAWN | rdi=NUL-terminated path | new pid or u64::MAX (patch 0005; caller keeps running) |
| 17 | FIELD_STIMULATE | rdi=pid, rsi=f32 magnitude bits | 0 or u64::MAX |
| 18 | FIELD_EVOLVE | rdi=moments | new field age |
| 19 | FIELD_OBSERVE | rdi=pid | trace-peak delta, f32 bits |
| 20 | FIELD_MEASUREMENTS | — | introduced energy, f64 bits |

### IPC is dual transport: bytes plus field energy

Since patch 0003, SYS_IPC_SEND delivers the message bytes to a per-pid
mailbox AND injects `len * 0.02` energy into the field (the energy
transport is unchanged). Since patch 0008 that mailbox is a FIFO ring
of up to 4 messages: back-to-back sends queue instead of overwriting,
and each SYS_IPC_RECV pops the oldest. When the ring is full the OLDEST
message is dropped (newest kept) — a deliberate generalization of the
old single-slot latest-wins behavior, so a slow receiver never loses
the freshest traffic. SYS_IPC_RECV returns the mailbox bytes when
present; otherwise it falls back to the field radiation path, which
encodes the magnitude `delta as u64` as ASCII — the fractional part is
truncated, so energy-only signals render as `0`.
The RECV payload lands in a per-pid cached frame mapped at
`0x00007F0000000000` (one frame per pid, reused — no leak). The
historical `0x300000` address is retired: its PML4[0] chain comes from
the bootloader and is supervisor-only (x86-64 requires U/S at every
level for ring-3 access), and the old slab page's physical address came
from `virt_to_phys` outside the phys-offset window — together they
produced reserved-bit and user-access #PFs the first time RECV returned
a page.

Kernel-side hazards documented for future patch work:

- `atom_rt::sys::ipc_recv` returns `&'static [u8]` over the RECV page
  mapping; a later RECV overwrites that page in place, so holding a
  slice across another RECV is unsound. Current daemon drops the slice
  before its next recv — do not change that without fixing the API.
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

Since patch 0005, `SYS_SPAWN` (21) loads a RamFS ELF as a brand-new
task — own pid, frame-backed 4 KiB kernel stack, fresh trap frame,
private address space built from the active CR3 — and hands it to the
scheduler; the caller keeps running (`spawn` in the shell). EXEC and
SPAWN share the `load_user_image` loader. The scheduler reclaims
Terminated task slots and `alloc_pid` hands out the lowest free pid
1..15, so short-lived spawns do not exhaust the table; a reused pid
inherits its old field-site history (accepted, documented here).

Since patch 0006, slot reclaim also reaps the dead task's resources.
`Context` records everything `load_user_image` allocates (parent CR3,
image span, user-stack run, kernel-stack frame), and when the scheduler
overwrites a Terminated slot, `reap_context` returns the frames to the
allocator: the kernel-stack frame, the image and user-stack runs, and
every PRIVATE page-table frame, recovered by a diff-walk against the
parent CR3 (subtrees whose parent slot points at the same frame are
shared and untouched; leaf data frames are never freed by the walk;
per-pid IPC RECV frames stay globally cached). Reap-at-reuse is the
safe point: another task has run since the termination, so the dead
CR3 is not loaded and nothing references the dead stack. Spawned
kernel stacks come from the frame allocator instead of the kernel heap
because the heap cannot free individual blocks. Diagnostics: every
successful spawn prints `SPAWN pid=N free_before=F` on serial and
every reap prints `REAP pid=N freed=K free=F`; CI asserts `freed ≥ 8`
and exact free-count equality across the reap window.

Since patch 0007, address spaces REPLACED by `run`/EXEC are freed too,
which closes the last loader leak. Two loader changes make this safe.
First, `load_user_image` now zeroes the source's image PTEs in the
fresh copy's private PT right after the unshare (both EXEC and SPAWN),
so a new root holds no stale aliases into the source's image frames.
Second, EXEC frees the replaced root with a three-part invariant:
(1) every root copied from it already had those aliases cut; (2) tasks
holding it as their diff-walk baseline are rebaselined to ITS parent
first — sound because each such task replaced the only region where it
differs (the image/stack chain) with its own private chain, so its
diff-walk frees the identical frame set against either baseline; and
(3) any private frame some other root still reaches (e.g. an inherited
IPC RECV chain) is deferred, never freed. EXEC then prints
`EXECFREE pid=N freed=K deferred=M`; CI asserts `freed ≥ 8` and
`deferred == 0`. Known limit: if the shell ever RECVs, then spawns,
then execs, such an inherited RECV chain can be freed later by a reap
of the exec lineage; no current userspace program RECVs except the
daemon, which never spawns or execs. With 0007 the loader allocates
and frees cleanly — `spawn` churn and `run` both return their frames.

## Known userspace limitations

- `edit` is append-only (no modifying existing content) and refuses
  files >1024 bytes rather than truncating them.
- Shell echoes per keystroke and heartbeat text from the daemon can
  interleave mid-line on the shared VGA.
- `msg` longer than 254 bytes fails; `read_line` accepts 1024.
- `fieldmon` via `run` replaces the shell (exec semantics); prefer
  `spawn fieldmon.elf` — it runs its 10 observation rounds and exits
  with the shell alive, and its slot is reaped on the next spawn
  (patches 0005/0006, verified 2026-08-18/19).
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

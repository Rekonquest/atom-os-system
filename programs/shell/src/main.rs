#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;

use atom_rt::shell_parse::{tokens, trim};
use atom_rt::{io, sys};

const HELP: &str = "commands:\n  help              show this text\n  ls                list files\n  clear             clear screen\n  cat <file>        print file\n  edit <file>       line editor ('.' on its own line saves)\n  echo <text> > <f> append text to file\n  msg <text>        send IPC to daemon (pid 2)\n  run <file.elf>    exec a program from the RamFS (replaces shell)\n  spawn <file.elf>  run a program as a new process (shell stays)\n  bench             10,000 SYS_YIELD cycle count\n  xtest             XMM liveness probe (baseline/syscall/preempt)\n";

fn prompt() {
    io::print("> ");
}

fn cmd_cat(name: &[u8]) {
    let Some(fd) = sys::open(name) else {
        io::print("cannot open file\n");
        return;
    };
    while let Some(b) = sys::read_file_byte(fd) {
        sys::write_byte(b);
    }
    io::print("\n");
    sys::close(fd);
}

fn cmd_echo(line: &[u8]) {
    let mut gt = None;
    for (i, &b) in line.iter().enumerate() {
        if b == b'>' {
            gt = Some(i);
            break;
        }
    }
    let Some(gt) = gt else {
        io::print("usage: echo <text> > <file>\n");
        return;
    };
    let text = trim(&line[4..gt]);
    let Some(file) = tokens(&line[gt + 1..]).next() else {
        io::print("usage: echo <text> > <file>\n");
        return;
    };
    let Some(fd) = sys::open(file) else {
        io::print("cannot open file\n");
        return;
    };
    for &b in text {
        sys::write_file_byte(fd, b);
    }
    sys::write_file_byte(fd, b'\n');
    sys::close(fd);
}

fn cmd_msg(line: &[u8]) {
    let text = trim(&line[3..]);
    if text.is_empty() {
        io::print("usage: msg <text>\n");
        return;
    }
    if sys::ipc_send(2, text) {
        io::print("Message sent to Daemon\n");
    } else {
        io::print("Failed to send message\n");
    }
}

fn cmd_bench() {
    let start = sys::rdtsc();
    for _ in 0..10_000 {
        sys::yield_now();
    }
    let end = sys::rdtsc();
    io::print("10,000 SYS_YIELDs took (CPU cycles): ");
    io::print_u64(end - start);
    io::print("\n");
}

fn cmd_run(name: &[u8]) {
    if !sys::exec(name) {
        io::print("exec failed\n");
    }
}

fn cmd_spawn(name: &[u8]) {
    match sys::spawn(name) {
        Some(pid) => {
            io::print("spawned pid ");
            io::print_u64(pid);
            io::print("\n");
        }
        None => io::print("spawn failed\n"),
    }
}

/// XMM liveness probe. Fills xmm0..15 with a known pattern, runs the
/// workload, snapshots the registers, and compares. Baseline proves the
/// probe itself is sound; syscall crosses the kernel dispatch path
/// (which does f32/f64 math); preempt idles so timer interrupts — whose
/// handler evolves the field substrate — can fire mid-window.
fn xmm_probe(label: &str, seed: u64, workload: fn()) -> bool {
    // The fill→snapshot window must contain NO stack stores of our own:
    // LLVM vectorizes array zero-inits with xorps/movaps and is free to
    // sink them into the window (disassembly confirmed it does), which
    // clobbers the very registers under test. Snapshot therefore writes
    // into uninitialized storage; the array is only materialized after.
    let mut snap_store = core::mem::MaybeUninit::<[u64; 32]>::uninit();
    atom_rt::xmm::fill(seed);
    workload();
    unsafe { atom_rt::xmm::snapshot(snap_store.as_mut_ptr() as *mut u64) };
    let snap = unsafe { snap_store.assume_init() };
    // Computed after the window: even a 16-byte scalar-pair store could
    // be merged into a vector store and touch xmm if it landed mid-window.
    let want = atom_rt::xmm::expected(seed);
    for reg in 0..16usize {
        let lo = snap[reg * 2];
        let hi = snap[reg * 2 + 1];
        if lo != want[0] || hi != want[1] {
            io::print("XMM_CLOBBERED ");
            io::print(label);
            io::print(" xmm");
            io::print_u64(reg as u64);
            io::print(" got=");
            io::print_u64(lo);
            io::print(":");
            io::print_u64(hi);
            io::print(" want=");
            io::print_u64(want[0]);
            io::print(":");
            io::print_u64(want[1]);
            io::print("\n");
            return false;
        }
    }
    io::print("XMM_OK ");
    io::print(label);
    io::print("\n");
    true
}

fn cmd_xtest() {
    let mut ok = true;
    // baseline: no kernel traffic — validates the probe mechanism.
    ok &= xmm_probe("baseline", sys::rdtsc() ^ 0x5EED_BA5E, || {});
    // syscall: 100,000 yields (~0.7 s) cross the dispatch/switch path and
    // span ~12 timer ticks, so the every-10th-tick field evolve (heavy SSE:
    // World::evolve + libm) fires inside the window. Integer-only syscalls
    // on purpose — a float-returning call would legitimately consume xmm0
    // per the SysV ABI and fake a clobber.
    ok &= xmm_probe("syscall", sys::rdtsc() ^ 0x0FF1CE, || {
        for _ in 0..100_000 {
            sys::yield_now();
        }
    });
    // preempt: ~2 s idle windows (≈4 field evolves each at the default
    // ~18 Hz PIT / EVOLUTION_DIVISOR=10) so timer IRQs interrupt the spin.
    for round in 0..3u64 {
        ok &= xmm_probe("preempt", sys::rdtsc() ^ (0xC1CADA << (round & 7)), || {
            let start = sys::rdtsc();
            while sys::rdtsc().wrapping_sub(start) < 7_000_000_000 {
                core::hint::spin_loop();
            }
        });
    }
    if ok {
        io::print("XTEST_PASS\n");
    } else {
        io::print("XTEST_FAIL\n");
    }
}

fn cmd_edit(name: &[u8]) {
    let Some(fd) = sys::open(name) else {
        io::print("cannot open file\n");
        return;
    };
    let mut buf = [0u8; 1024];
    let mut len = 0usize;
    let mut overflow = false;
    while let Some(b) = sys::read_file_byte(fd) {
        if len < buf.len() {
            buf[len] = b;
            len += 1;
            sys::write_byte(b);
        } else {
            overflow = true;
        }
    }
    if overflow {
        io::print("\nfile too large to edit (>1024 bytes); left unchanged\n");
        sys::close(fd);
        return;
    }
    io::print("\n-- editing; end with '.' on its own line --\n");
    let mut line = [0u8; 256];
    loop {
        let n = io::read_line(&mut line);
        let l = trim(&line[..n]);
        if l == b"." {
            break;
        }
        for &b in l {
            if len < buf.len() {
                buf[len] = b;
                len += 1;
            }
        }
        if len < buf.len() {
            buf[len] = b'\n';
            len += 1;
        }
    }
    sys::truncate(fd);
    for &b in &buf[..len] {
        sys::write_file_byte(fd, b);
    }
    sys::close(fd);
    io::print("saved\n");
}

fn prove_heap() {
    let mut marker: Vec<u8> = Vec::new();
    marker.extend_from_slice(b"HEAP_OK");
    io::print_bytes(&marker);
    io::print("\n");
}

fn main() -> ! {
    io::print("ATOM OS System shell (type 'help')\n");
    prove_heap();
    cmd_bench();
    let mut line = [0u8; 1024];
    prompt();
    loop {
        let n = io::read_line(&mut line);
        let cmd = trim(&line[..n]);
        if cmd.is_empty() {
            prompt();
            continue;
        }
        let mut t = tokens(cmd);
        let head = t.next().unwrap_or(b"");
        match head {
            b"help" => io::print(HELP),
            b"ls" => sys::list_dir(),
            b"clear" => sys::clear(),
            b"bench" => cmd_bench(),
            b"cat" => match t.next() {
                Some(name) => cmd_cat(name),
                None => io::print("usage: cat <file>\n"),
            },
            b"edit" => match t.next() {
                Some(name) => cmd_edit(name),
                None => io::print("usage: edit <file>\n"),
            },
            b"echo" => cmd_echo(cmd),
            b"msg" => cmd_msg(cmd),
            b"run" => match t.next() {
                Some(name) => cmd_run(name),
                None => io::print("usage: run <file.elf>\n"),
            },
            b"spawn" => match t.next() {
                Some(name) => cmd_spawn(name),
                None => io::print("usage: spawn <file.elf>\n"),
            },
            b"xtest" => cmd_xtest(),
            _ => io::print("Unknown command\n"),
        }
        prompt();
    }
}

atom_rt::program!(main);

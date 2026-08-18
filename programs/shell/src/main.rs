#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;

use atom_rt::shell_parse::{tokens, trim};
use atom_rt::{io, sys};

const HELP: &str = "commands:\n  help              show this text\n  ls                list files\n  clear             clear screen\n  cat <file>        print file\n  edit <file>       line editor ('.' on its own line saves)\n  echo <text> > <f> append text to file\n  msg <text>        send IPC to daemon (pid 2)\n  run <file.elf>    exec a program from the RamFS\n  bench             10,000 SYS_YIELD cycle count\n";

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
            _ => io::print("Unknown command\n"),
        }
        prompt();
    }
}

atom_rt::program!(main);

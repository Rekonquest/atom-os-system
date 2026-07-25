#![no_std]
#![no_main]

extern crate alloc;

use atom_rt::{io, sys};

fn main() -> ! {
    let mut ipc_buf = [0u8; 64];
    loop {
        for _ in 0..500_000 {
            sys::yield_now();
        }
        match sys::ipc_recv_into(&mut ipc_buf) {
            Some(n) => {
                io::print("\n[Daemon] Received IPC: ");
                io::print_bytes(&ipc_buf[..n]);
                io::print("\n");
            }
            None => io::print("[Daemon] Heartbeat... "),
        }
    }
}

atom_rt::program!(main);

#![no_std]
#![no_main]

extern crate alloc;

use atom_rt::{io, sys};

fn main() -> ! {
    loop {
        for _ in 0..500_000 {
            sys::yield_now();
        }
        match sys::ipc_recv() {
            Some(msg) => {
                io::print("\n[Daemon] Received IPC: ");
                io::print_bytes(msg);
                io::print("\n");
            }
            None => io::print("[Daemon] Heartbeat... "),
        }
    }
}

atom_rt::program!(main);

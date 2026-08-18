#![no_std]
#![no_main]

extern crate alloc;

use atom_rt::{io, sys};

const SELF_PID: u64 = 1;
const ROUNDS: u64 = 10;

fn main() -> ! {
    io::print("fieldmon: observing the substrate field\n");
    for _ in 0..ROUNDS {
        sys::field::stimulate(SELF_PID, 0.05);
        let age = sys::field::evolve(25);
        let introduced = sys::field::introduced();
        let trace = sys::field::observe(SELF_PID);
        io::print("age=");
        io::print_u64(age);
        io::print(" introduced=");
        io::print_fixed6(introduced);
        io::print(" self_trace+");
        io::print_fixed6(trace as f64);
        io::print("\n");
    }
    sys::exit();
}

atom_rt::program!(main);

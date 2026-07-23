#![no_std]
#![no_main]

extern crate alloc;

fn main() -> ! {
    atom_rt::io::print("hello from ATOM OS System\n");
    atom_rt::sys::exit();
}

atom_rt::program!(main);

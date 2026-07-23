#![no_std]

extern crate alloc;

pub mod fmt;
pub mod heap;
pub mod shell_parse;

#[cfg(target_os = "none")]
pub mod io;
#[cfg(target_os = "none")]
pub mod sys;

#[macro_export]
macro_rules! program {
    ($main:expr) => {
        #[panic_handler]
        fn __atom_rt_panic(_info: &::core::panic::PanicInfo) -> ! {
            $crate::io::print("panic!\n");
            $crate::sys::exit();
        }

        #[global_allocator]
        static __ATOM_RT_HEAP: $crate::heap::BumpAllocator =
            $crate::heap::BumpAllocator::new();

        #[unsafe(no_mangle)]
        pub extern "C" fn _start() -> ! {
            ($main)()
        }
    };
}

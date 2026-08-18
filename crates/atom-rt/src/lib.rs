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
    ($main:path) => {
        #[panic_handler]
        fn __atom_rt_panic(_info: &::core::panic::PanicInfo) -> ! {
            $crate::io::print("panic!\n");
            $crate::sys::exit();
        }

        #[global_allocator]
        static __ATOM_RT_HEAP: $crate::heap::BumpAllocator =
            $crate::heap::BumpAllocator::new();

        // Process-entry stub. The kernel enters us via iretq with rsp at
        // the stack top, 16-byte aligned and no return address (the SysV
        // process-entry shape). An ordinary Rust fn assumes rsp ≡ 8 (mod 16)
        // at entry (as if CALLed), so if _start itself were Rust code every
        // stack-slot alignment LLVM computes downstream would be off by 8,
        // and the first rsp-relative movaps (e.g. zeroing a [u8; 64] buffer
        // in sys::exec's copy_nul) would #GP(0). CALLing the real entry from
        // this stub restores the invariant: call-site rsp ≡ 0, callee entry
        // rsp ≡ 8.
        core::arch::global_asm!(
            ".global _start",
            "_start:",
            "call {entry}",
            "ud2",
            entry = sym $main,
        );
    };
}

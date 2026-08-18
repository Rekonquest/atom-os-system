//! XMM liveness probes (target-only).
//!
//! Diagnostic for the documented hazard "XMM registers are NOT saved by
//! the kernel trap frame while the dispatch path does f32 math": load a
//! known pattern into all XMM registers, cross a kernel boundary
//! (syscall or preemption), then read the registers back.
//!
//! The asm here is deliberately opaque to LLVM: `fill` declares every
//! XMM as clobbered (which is true — it overwrites them), and
//! `snapshot` reads the registers without declaring inputs, so the
//! compiler can neither supply its own values nor optimize the
//! comparison away. The verdict comes from comparing snapshot output in
//! memory against the expected pattern.

use core::arch::asm;

/// Overwrite xmm0..xmm15 with a known 128-bit pattern derived from `seed`.
pub fn fill(seed: u64) {
    let block = [seed, seed ^ 0xA5A5_A5A5_A5A5_A5A5];
    unsafe {
        asm!(
            "movdqu xmm0, [{p}]",
            "movdqu xmm1, [{p}]",
            "movdqu xmm2, [{p}]",
            "movdqu xmm3, [{p}]",
            "movdqu xmm4, [{p}]",
            "movdqu xmm5, [{p}]",
            "movdqu xmm6, [{p}]",
            "movdqu xmm7, [{p}]",
            "movdqu xmm8, [{p}]",
            "movdqu xmm9, [{p}]",
            "movdqu xmm10, [{p}]",
            "movdqu xmm11, [{p}]",
            "movdqu xmm12, [{p}]",
            "movdqu xmm13, [{p}]",
            "movdqu xmm14, [{p}]",
            "movdqu xmm15, [{p}]",
            p = in(reg) block.as_ptr(),
            out("xmm0") _, out("xmm1") _, out("xmm2") _, out("xmm3") _,
            out("xmm4") _, out("xmm5") _, out("xmm6") _, out("xmm7") _,
            out("xmm8") _, out("xmm9") _, out("xmm10") _, out("xmm11") _,
            out("xmm12") _, out("xmm13") _, out("xmm14") _, out("xmm15") _,
            options(nostack)
        );
    }
}

/// Read xmm0..xmm15 into `out` (two u64 per register, low then high).
/// Takes a raw pointer so callers can hand over uninitialized storage —
/// any stack store inside the fill→snapshot window can be vectorized by
/// LLVM (xorps/movaps) and clobber the very registers under test.
///
/// Safety: `out` must point at 32 writable u64.
pub unsafe fn snapshot(out: *mut u64) {
    unsafe {
        asm!(
            "movdqu [{p}], xmm0",
            "movdqu [{p}+16], xmm1",
            "movdqu [{p}+32], xmm2",
            "movdqu [{p}+48], xmm3",
            "movdqu [{p}+64], xmm4",
            "movdqu [{p}+80], xmm5",
            "movdqu [{p}+96], xmm6",
            "movdqu [{p}+112], xmm7",
            "movdqu [{p}+128], xmm8",
            "movdqu [{p}+144], xmm9",
            "movdqu [{p}+160], xmm10",
            "movdqu [{p}+176], xmm11",
            "movdqu [{p}+192], xmm12",
            "movdqu [{p}+208], xmm13",
            "movdqu [{p}+224], xmm14",
            "movdqu [{p}+240], xmm15",
            p = in(reg) out,
            out("xmm0") _, out("xmm1") _, out("xmm2") _, out("xmm3") _,
            out("xmm4") _, out("xmm5") _, out("xmm6") _, out("xmm7") _,
            out("xmm8") _, out("xmm9") _, out("xmm10") _, out("xmm11") _,
            out("xmm12") _, out("xmm13") _, out("xmm14") _, out("xmm15") _,
            options(nostack)
        );
    }
}

/// Expected snapshot contents for a given seed (mirrors `fill`).
pub fn expected(seed: u64) -> [u64; 2] {
    [seed, seed ^ 0xA5A5_A5A5_A5A5_A5A5]
}

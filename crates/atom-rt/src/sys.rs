use core::arch::asm;

pub const SYS_YIELD: u64 = 1;
pub const SYS_ALLOC: u64 = 2;
pub const SYS_EXIT: u64 = 3;
pub const SYS_READ: u64 = 4;
pub const SYS_WRITE: u64 = 5;
pub const SYS_OPEN: u64 = 6;
pub const SYS_READ_FILE: u64 = 7;
pub const SYS_WRITE_FILE: u64 = 8;
pub const SYS_CLOSE: u64 = 9;
pub const SYS_LIST_DIR: u64 = 10;
pub const SYS_CLEAR: u64 = 11;
pub const SYS_TRUNCATE: u64 = 12;
pub const SYS_EXEC: u64 = 13;
pub const SYS_PRINT: u64 = 14;
pub const SYS_IPC_SEND: u64 = 15;
pub const SYS_IPC_RECV: u64 = 16;
pub const SYS_FIELD_STIMULATE: u64 = 17;
pub const SYS_FIELD_EVOLVE: u64 = 18;
pub const SYS_FIELD_OBSERVE: u64 = 19;
pub const SYS_FIELD_MEASUREMENTS: u64 = 20;

#[inline(always)]
unsafe fn syscall0(n: u64) -> u64 {
    let ret: u64;
    unsafe {
        asm!("int 0x80", inout("rax") n => ret, options(nostack, preserves_flags));
    }
    ret
}

#[inline(always)]
unsafe fn syscall1(n: u64, a0: u64) -> u64 {
    let ret: u64;
    unsafe {
        asm!(
            "int 0x80",
            inout("rax") n => ret,
            in("rdi") a0,
            options(nostack, preserves_flags)
        );
    }
    ret
}

#[inline(always)]
unsafe fn syscall2(n: u64, a0: u64, a1: u64) -> u64 {
    let ret: u64;
    unsafe {
        asm!(
            "int 0x80",
            inout("rax") n => ret,
            in("rdi") a0,
            in("rsi") a1,
            options(nostack, preserves_flags)
        );
    }
    ret
}

pub fn yield_now() {
    unsafe { syscall0(SYS_YIELD) };
}

pub fn exit() -> ! {
    unsafe { syscall0(SYS_EXIT) };
    loop {}
}

pub fn read_char() -> Option<u8> {
    match unsafe { syscall0(SYS_READ) } {
        0 => None,
        c => Some(c as u8),
    }
}

pub fn write_byte(b: u8) {
    unsafe { syscall1(SYS_WRITE, b as u64) };
}

pub fn list_dir() {
    unsafe { syscall0(SYS_LIST_DIR) };
}

pub fn clear() {
    unsafe { syscall0(SYS_CLEAR) };
}

fn copy_nul<const N: usize>(src: &[u8]) -> Option<[u8; N]> {
    if src.len() >= N {
        return None;
    }
    let mut buf = [0u8; N];
    buf[..src.len()].copy_from_slice(src);
    Some(buf)
}

pub fn open(path: &[u8]) -> Option<u64> {
    let buf = copy_nul::<64>(path)?;
    match unsafe { syscall1(SYS_OPEN, buf.as_ptr() as u64) } {
        u64::MAX => None,
        fd => Some(fd),
    }
}

pub fn read_file_byte(fd: u64) -> Option<u8> {
    match unsafe { syscall1(SYS_READ_FILE, fd) } {
        u64::MAX => None,
        b => Some(b as u8),
    }
}

pub fn write_file_byte(fd: u64, b: u8) -> bool {
    (unsafe { syscall2(SYS_WRITE_FILE, fd, b as u64) }) != u64::MAX
}

pub fn truncate(fd: u64) -> bool {
    (unsafe { syscall1(SYS_TRUNCATE, fd) }) != u64::MAX
}

pub fn close(fd: u64) -> bool {
    (unsafe { syscall1(SYS_CLOSE, fd) }) != u64::MAX
}

pub fn exec(path: &[u8]) -> bool {
    let Some(buf) = copy_nul::<64>(path) else {
        return false;
    };
    (unsafe { syscall1(SYS_EXEC, buf.as_ptr() as u64) }) != u64::MAX
}

pub fn ipc_send(target_pid: u64, msg: &[u8]) -> bool {
    let Some(buf) = copy_nul::<255>(msg) else {
        return false;
    };
    (unsafe { syscall2(SYS_IPC_SEND, target_pid, buf.as_ptr() as u64) }) != u64::MAX
}

pub fn ipc_recv() -> Option<&'static [u8]> {
    let v = unsafe { syscall0(SYS_IPC_RECV) };
    if v == 0 || v == u64::MAX {
        return None;
    }
    let ptr = v as *const u8;
    let mut len = 0usize;
    unsafe {
        while *ptr.add(len) != 0 && len < 4096 {
            len += 1;
        }
        Some(core::slice::from_raw_parts(ptr, len))
    }
}

pub fn rdtsc() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() }
}

pub mod field {
    use super::*;

    pub fn stimulate(pid: u64, magnitude: f32) -> bool {
        (unsafe { syscall2(SYS_FIELD_STIMULATE, pid, magnitude.to_bits() as u64) }) != u64::MAX
    }

    pub fn evolve(moments: u64) -> u64 {
        unsafe { syscall1(SYS_FIELD_EVOLVE, moments) }
    }

    pub fn observe(pid: u64) -> f32 {
        f32::from_bits(unsafe { syscall1(SYS_FIELD_OBSERVE, pid) } as u32)
    }

    pub fn introduced() -> f64 {
        f64::from_bits(unsafe { syscall0(SYS_FIELD_MEASUREMENTS) })
    }
}

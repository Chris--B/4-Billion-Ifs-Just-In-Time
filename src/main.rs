#![allow(non_upper_case_globals)]

use libc::*;

use std::ops::{Deref, DerefMut};

const KiB: usize = 1 << 10;

fn main() {
    let mut jit = JitMem::new();

    assert_eq!(std::env::consts::ARCH, "x86_64");
    let mut return_int = [
        0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00, // mov rax, 0x1
        0xC3, // ret
    ];

    unsafe {
        return_int[3] = 7;
        jit[..return_int.len()].copy_from_slice(&return_int);

        let seven = jit.make_fn();
        dbg!(seven());
    }
}

struct JitMem {
    p_mem: *mut u8,
    size: usize,
}

impl JitMem {
    fn new() -> Self {
        Self::new_with_size(16 * KiB)
    }

    fn new_with_size(mut size: usize) -> Self {
        const PAGE_SIZE: usize = 4 * KiB; // ඞ

        if size % PAGE_SIZE != 0 {
            size = (size + PAGE_SIZE) & !(PAGE_SIZE - 1);
        }
        assert_eq!(size % PAGE_SIZE, 0);

        unsafe {
            let mut p_mem: *mut c_void = core::ptr::null_mut();

            // MacOS has alignment requirements on executabe pages

            let _ = posix_memalign(&mut p_mem, PAGE_SIZE, size);
            mprotect(p_mem, size, PROT_EXEC | PROT_READ | PROT_WRITE);

            // x64 'RET'
            // Good luck, Rosetta
            memset(p_mem, 0xC3, size);

            Self {
                p_mem: p_mem as *mut u8,
                size,
            }
        }
    }

    fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.p_mem, self.size) }
    }

    fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.p_mem, self.size) }
    }

    unsafe fn make_fn(&self) -> fn() -> i64 {
        core::mem::transmute(self.p_mem)
    }
}

impl Deref for JitMem {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl DerefMut for JitMem {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_slice_mut()
    }
}

impl Drop for JitMem {
    fn drop(&mut self) {
        unsafe {
            free(self.p_mem as *mut c_void);
        }
    }
}

#![allow(non_upper_case_globals)]

use libc::*;

use std::ops::{Deref, DerefMut};

const KiB: usize = 1 << 10;

#[track_caller]
#[must_use]
fn write(to: &mut [u8], from: impl AsRef<[u8]>) -> &mut [u8] {
    let from = from.as_ref();
    let n = from.len();
    to[..n].copy_from_slice(from);

    &mut to[n..]
}

fn build_is_odd(jit: &mut JitMem) -> fn(i64) -> i64 {
    let header: [u8; 7] = [
        // move rax, 0x0
        0x48, 0xc7, 0xc0, 0x00, 0x00, 0x00, 0x00,
    ];

    #[rustfmt::skip]
    let mut block : [u8; 14]= [
        // cmp rdi, 0x0
        //          ^^^---vvvv
        0x48, 0x83, 0xff, 0x00,
        // Jump relative 0x08 ahead (this matches the end of this array)
        // See: https://faydoc.tripod.com/cpu/jne.htm
        // jne 0x08
        0x75, 0x08,
        // mov rax, 0x1
        //          ^^^---vvvv
        0x48, 0xc7, 0xc0, 0x01, 0x00, 0x00, 0x00,
        // ret
        0xc3,
    ];

    let mut mem = jit.as_slice_mut();

    #[allow(unused_assignments)]
    unsafe {
        mem = write(mem, header);

        for n in 0..=u8::MAX {
            block[3] = n;
            block[9] = n & 1;
            mem = write(mem, block);
        }

        jit.make_fn()
    }
}

fn main() {
    assert_eq!(std::env::consts::ARCH, "x86_64");

    let mut jit = JitMem::new();
    let is_odd = build_is_odd(&mut jit);

    for n in 0..(u8::MAX as i64) {
        let is_n_odd = is_odd(n);
        println!("is_odd({n}) == {}", is_n_odd);
    }
}

#[test]
fn check_u8_nums() {
    let mut jit = JitMem::new();
    let is_odd = build_is_odd(&mut jit);

    for n in 0..(u8::MAX as i64) {
        let is_n_odd = is_odd(n);
        if n & 1 == 0 {
            assert!(
                is_n_odd == 0,
                "is_odd({n}) reports that {n} is odd but it's even!"
            );
        } else {
            assert!(
                is_n_odd == 1,
                "is_odd({n}) reports that {n} is even but it's odd!"
            );
        }
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

            // x64 'RET', anything that lands in "uninit" memory here will immediately return
            // We could also fault....
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

    unsafe fn make_fn(&self) -> fn(i64) -> i64 {
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
        // We can't track function pointer lifetimes, so just... don't
        // unsafe {
        //     free(self.p_mem as *mut c_void);
        // }
    }
}

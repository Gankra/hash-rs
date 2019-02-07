// String hashing by iterating Dietzfelbinger et al.'s multiply-shift
// hashing. The resulting hash value can be used in hash tables by
// shifting it right, but not by taking the low-order bits -- the
// high-order bits are the higher-quality ones for use in
// distinguishing the hashed keys.
//
// TODO: by using Woelfel's multiply-add-shift hashing, we can get the
// lower order bits to be the most usable ones.
//
// TODO: accumulating four hash values at once increases the speed on
// my machine, but it also makes the code more complex.


use std::ptr::copy_nonoverlapping;
//#[stable(feature = "rust1", since = "1.0.0")]
//pub use intrinsics::copy_nonoverlapping;
use std::hash::Hasher;
use std::cmp::min;

// This is called a "Horner" hasher because the iterated
// multiply-shift operation resembles Horner's method for evaluating
// polynomials. In fact, we are computing, more or less,
//
// sum_i (h0 ^ i) \floor{xi * h / 2^64}
//
// where xi is the ith word of the key being hashed.
//
// TODO: explain that equivalence in more detail.
pub struct HornerHasher {
    // A randomly-chosen odd 128-bit number. h0 holds the
    // least-significant bits, so must be odd.
    h0: u64,
    h1: u64,
    // The hash value we have accumulated so far.
    result: [u64; 4],
    accum: [u64; 4],
    // The number of bytes we have seen so far
    count: u64
}

impl Default for HornerHasher {
    fn default() -> HornerHasher {
        // h0 and h1 should be populated from a random source like
        // rand::os::OsRng::next_u64, but this is done in the hash map
        // constructor.
        //
        // h0 must be odd.
        return HornerHasher {h0: 4167967182414233411,
                             h1: 15315631059493996859,
                             result: [0,0,0,0],
                             accum: [0,0,0,0],
                             count: 0};
    }
}

// multiply two 64-bit words and return the 64 most significant bits
// of the 128-bit product.
//
// TODO: implement and test this on other architectures.
#[cfg(target_arch = "x86_64")]
#[inline(always)]
fn hi64mul(x: u64, y: u64) -> u64 {
    let _lo: u64; let hi: u64;
    unsafe { asm!("mulq $2"
                  : "={rax}" (_lo), "={rdx}" (hi)
                  : "r" (x), "{rax}" (y)
                  : "cc" :); }
    hi
}


// Multiply two 128-bit numbers and write 64 bits of the product to
// 'result'. The bits written are those starting from the 64th least
// significant bit (counting from 0) and going up. This is
// multiply-shift hashing ala Dietzfelbinger et al.
#[inline(always)]
fn mult_hi128(result: &mut u64, accum: u64, h0: u64, h1: u64) {
    // Loosely following Woelfel's "A Construction Method for
    // Optimally Universal Hash Families and its Consequences for the
    // Existence of RBIBDs", along with multiply-add-shift hashing, we
    // hash accum by multiplying it by h, then shifting right. We hash
    // result and accum by hashing accum and adding result.
    *result = result.wrapping_add(accum.wrapping_mul(h1).wrapping_add(hi64mul(accum, h0)));
}

/// Load a full u64 word from a byte stream. Use `copy_nonoverlapping`
/// to let the compiler generate the most efficient way to load u64
/// from a possibly unaligned address.
///
/// Unsafe because: unchecked indexing at i..i+8
#[inline]
unsafe fn load_u64(buf: &[u8], i: usize) -> u64 {
    debug_assert!(i + 8 <= buf.len());
    let mut data = 0u64;
    copy_nonoverlapping(buf.get_unchecked(i), &mut data as *mut _ as *mut u8, 8);
    data
}

impl Hasher for HornerHasher {

    fn finish(&self) -> u64 {
        if self.count <= 8 {
            let mut t1 = self.accum[0];
            mult_hi128(&mut t1, self.count, self.h0, self.h1);
            return t1;
        }
        if self.count <= 16 {
            let mut t1 = self.accum[0];
            mult_hi128(&mut t1, self.accum[1], self.h0, self.h1);
            mult_hi128(&mut t1, self.count, self.h0, self.h1);
            return t1;
        }
        if self.count <= 24 {
            let mut t1 = self.accum[0];
            let mut t2 = self.accum[1];
            mult_hi128(&mut t1, self.accum[2], self.h0, self.h1);
            mult_hi128(&mut t2, self.count, self.h0, self.h1);
            mult_hi128(&mut t1, t2, self.h0, self.h1);
            return t1;
        }
        if self.count < 32 {
            let mut t1 = self.accum[0];
            let mut t2 = self.accum[1];
            mult_hi128(&mut t1, self.accum[2], self.h0, self.h1);
            mult_hi128(&mut t2, self.accum[3], self.h0, self.h1);
            mult_hi128(&mut t1, self.count, self.h0, self.h1);
            mult_hi128(&mut t1, t2, self.h0, self.h1);
            return t1;
        }
        // Hashes any data waiting in self.accum and also hashes with
        // the length of the string to prevent engineered collisions
        // by prepending '\000's to hashed keys.
        let mut i: usize = 0;
        let mut result: [u64; 4] = [self.result[0], self.result[1], self.result[2], self.result[3]];

        while i < (((self.count & 31) + 7)/8) as usize {
            mult_hi128(&mut result[i], self.accum[i], self.h0, self.h1);
            i += 1;
        }
        let tmp1 = result[1];
        let tmp3 = result[3];
        mult_hi128(&mut result[0], tmp1, self.h0, self.h1);
        mult_hi128(&mut result[2], tmp3, self.h0, self.h1);
        mult_hi128(&mut result[0], self.count, self.h0, self.h1);
        let f1 = result[1];
        mult_hi128(&mut result[0], f1, self.h0, self.h1);
        return result[0];
    }

    fn write(&mut self, bytes: &[u8]) {
        let mut i = 0;

        // Fill up self.accum, as much as possible
        let n: u64 = min(32 - (self.count & 31), bytes.len() as u64);
        unsafe {
            copy_nonoverlapping(bytes.get_unchecked(i),
                                (&mut self.accum[0] as *mut u64 as *mut u8)
                                .offset((self.count & 31) as isize),
                                n as usize);
        }
        self.count += n;
        i += n as usize;

        self.count += (bytes.len() - i) as u64;

        // If we filled self.accum, hash it and reset it.
        if 0 == self.count & 31 {
            if 32 == self.count {
                self.result[0] = self.accum[0];
                self.result[1] = self.accum[1];
                self.result[2] = self.accum[2];
                self.result[3] = self.accum[3];
            } else {
                mult_hi128(&mut self.result[0], self.accum[0], self.h0, self.h1);
                mult_hi128(&mut self.result[1], self.accum[1], self.h0, self.h1);
                mult_hi128(&mut self.result[2], self.accum[2], self.h0, self.h1);
                mult_hi128(&mut self.result[3], self.accum[3], self.h0, self.h1);
            }
            self.accum[0] = 0;
            self.accum[1] = 0;
            self.accum[2] = 0;
            self.accum[3] = 0;
        }

        // This is the main loop: for each 4 64-byte words we pull
        // from bytes, hash it into self.result.
        while i + 31 < bytes.len() {
            mult_hi128(&mut self.result[0],
                       unsafe {load_u64(bytes, i)},
                       self.h0, self.h1);
            mult_hi128(&mut self.result[1],
                       unsafe {load_u64(bytes, i + 8)},
                       self.h0, self.h1);
            mult_hi128(&mut self.result[2],
                       unsafe {load_u64(bytes, i + 16)},
                       self.h0, self.h1);
            mult_hi128(&mut self.result[3],
                       unsafe {load_u64(bytes, i + 24)},
                       self.h0, self.h1);
            i += 32;
        }

        // Add in the remaining data to self.accum.
        let n = bytes.len() - i;
        unsafe {copy_nonoverlapping(bytes.get_unchecked(i), &mut self.accum[0] as *mut u64 as *mut u8, n);}
    }
}

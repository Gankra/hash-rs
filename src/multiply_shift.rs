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


use std::intrinsics::copy_nonoverlapping;
//#[stable(feature = "rust1", since = "1.0.0")]
//pub use intrinsics::copy_nonoverlapping;
use std::hash::Hasher;

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
    // least-significant bits.
    h0: u64,
    h1: u64,
    // The hash value we have accumulated so far.
    result: u64,
    // We accumulate 8 bytes, then update 'result'. accum holds those
    // bytes in its least-significant bits. This can be thought of
    // (and will be treated) as both a u64 and a [u8; 8].
    accum: u64,
    // The number of bytes we have seen so far
    count: u64
}

impl Default for HornerHasher {
    fn default() -> HornerHasher {
        // h0 and h1 should be populated from a random source like
        // rand::os::OsRng::next_u64, but this is done in the hash map
        // constructor.
        return HornerHasher {h0: 4167967182414233411, 
                             h1: 15315631059493996859,
                             result: 0, accum: 0, count: 0};
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
    unsafe { asm!("mulq $3" 
                  : "=rm" (_lo), "=r" (hi) 
                  : "0rm" (x), "rm" (y) 
                  : "cc" :); }
    return hi;
}

// Multiply two 128-bit numbers and write 64 bits of the product to
// 'result'. The bits written are those starting from the 64th least
// significant bit (counting from 0) and going up. This is
// multiply-shift hashing ala Dietzfelbinger et al.
#[inline(always)]
fn mult_hi128(result: &mut u64, accum: u64, h0: u64, h1: u64) {
    *result = (accum.wrapping_mul(h0))
        .wrapping_add(result.wrapping_mul(h1))
        .wrapping_add(hi64mul(*result, h0))
}

/// Load a full u64 word from a byte stream, in LE order. Use
/// `copy_nonoverlapping` to let the compiler generate the most efficient way
/// to load u64 from a possibly unaligned address.
///
/// Unsafe because: unchecked indexing at i..i+8
#[inline]
unsafe fn load_u64_le(buf: &[u8], i: usize) -> u64 {
    debug_assert!(i + 8 <= buf.len());
    let mut data = 0u64;
    copy_nonoverlapping(buf.get_unchecked(i), &mut data as *mut _ as *mut u8, 8);
    data
}

impl Hasher for HornerHasher {
    
    fn finish(&self) -> u64 {
        // Hashes any characters waiting in self.accum and also hashes
        // with the length of the string to prevent engineered
        // collisions by prepending '\000's to hashed keys.
        let mut result: u64 = self.result;
        if (self.count & 7) > 0 {
            mult_hi128(&mut result, self.accum, self.h0, self.h1);
        }
        mult_hi128(&mut result, self.count, self.h0, self.h1);
        return result;
    }

    fn write(&mut self, bytes: &[u8]) {
        let mut i = 0;
        
        // Fill up self.accum if it is not full.
        let accum = &mut self.accum as *mut u64 as *mut u8;
        while (self.count & 7) > 0 && i < bytes.len() {
            unsafe {
                *(accum.offset((self.count & 7) as isize) as *mut u8) 
                    = bytes[i]; 
            }
            i += 1;
            self.count += 1;
        }

        self.count += (bytes.len() - i) as u64;

        // If we touched self.accum, hash it and reset it.
        if i > 0 {
            mult_hi128(&mut self.result, self.accum, self.h0, self.h1);
            self.accum = 0;
        }
        
        // This is the main loop: for each 64-bits we pull from bytes,
        // hash it into self.result.
        while i + 7 < bytes.len() {
            mult_hi128(&mut self.result, 
                       unsafe {load_u64_le(bytes, i)},
                       self.h0, 
                       self.h1);
            i += 8;
        }
        
        // Add in the remaining characters to self.accum.
        while i < bytes.len() {
            unsafe { 
                *(accum.offset((8 + i - bytes.len()) as isize) as *mut u8) 
                    = bytes[i]; 
            }
            i += 1;
        }
    }
}

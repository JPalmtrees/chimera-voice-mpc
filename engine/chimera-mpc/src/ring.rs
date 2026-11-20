//! A bounded single-producer single-consumer queue of `Copy` values: wait-free on both ends, no
//! allocation after construction, so the audio thread can use it.
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Ring<T: Copy> {
    buf: Box<[UnsafeCell<MaybeUninit<T>>]>,
    mask: usize,
    /// next slot the producer writes (only the producer stores it)
    head: AtomicUsize,
    /// next slot the consumer reads (only the consumer stores it)
    tail: AtomicUsize,
}

// One producer and one consumer, each on its own thread: the slots between tail and head belong to
// the consumer, the rest to the producer, and the acquire/release pairs hand them over.
unsafe impl<T: Copy + Send> Sync for Ring<T> {}
unsafe impl<T: Copy + Send> Send for Ring<T> {}

impl<T: Copy> Ring<T> {
    /// A queue holding up to `capacity` values (rounded up to a power of two).
    pub fn new(capacity: usize) -> Self {
        let n = capacity.max(2).next_power_of_two();
        let buf = (0..n).map(|_| UnsafeCell::new(MaybeUninit::uninit())).collect::<Vec<_>>().into_boxed_slice();
        Ring { buf, mask: n - 1, head: AtomicUsize::new(0), tail: AtomicUsize::new(0) }
    }

    /// Producer: add a value; false (and nothing added) when full.
    pub fn push(&self, v: T) -> bool {
        let h = self.head.load(Ordering::Relaxed);
        let t = self.tail.load(Ordering::Acquire);
        if h.wrapping_sub(t) > self.mask {
            return false;
        }
        unsafe { (*self.buf[h & self.mask].get()).write(v) };
        self.head.store(h.wrapping_add(1), Ordering::Release);
        true
    }

    /// Producer: free slots now.
    pub fn free(&self) -> usize {
        let h = self.head.load(Ordering::Relaxed);
        let t = self.tail.load(Ordering::Acquire);
        self.mask + 1 - h.wrapping_sub(t)
    }

    /// Consumer: take the oldest value.
    pub fn pop(&self) -> Option<T> {
        let t = self.tail.load(Ordering::Relaxed);
        let h = self.head.load(Ordering::Acquire);
        if t == h {
            return None;
        }
        let v = unsafe { (*self.buf[t & self.mask].get()).assume_init() };
        self.tail.store(t.wrapping_add(1), Ordering::Release);
        Some(v)
    }
}

#[cfg(test)]
mod tests {
    use super::Ring;
    use std::sync::Arc;

    #[test]
    fn keeps_order_and_bounds() {
        let r = Ring::new(4);
        for i in 0..4 {
            assert!(r.push(i));
        }
        assert!(!r.push(9));
        assert_eq!(r.pop(), Some(0));
        assert!(r.push(4));
        let got: Vec<i32> = std::iter::from_fn(|| r.pop()).collect();
        assert_eq!(got, [1, 2, 3, 4]);
    }

    #[test]
    fn across_threads() {
        let r = Arc::new(Ring::new(64));
        let p = r.clone();
        let th = std::thread::spawn(move || {
            for i in 0..100_000u32 {
                while !p.push(i) {
                    std::hint::spin_loop();
                }
            }
        });
        let mut want = 0u32;
        while want < 100_000 {
            if let Some(v) = r.pop() {
                assert_eq!(v, want);
                want += 1;
            }
        }
        th.join().unwrap();
    }
}

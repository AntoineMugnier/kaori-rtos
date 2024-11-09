use core::cell::UnsafeCell;
use core::sync::atomic::{self, AtomicBool};

pub struct AsyncRefCell<T>{
    inner: UnsafeCell<T>,
    locked: AtomicBool
}
unsafe impl <T: Send>Sync for AsyncRefCell<T>{
}

impl <T>AsyncRefCell<T>{
    pub const fn new(val: T) -> AsyncRefCell<T>{
        AsyncRefCell{inner :UnsafeCell::new(val), locked: AtomicBool::new(false)}
    }

    pub fn borrow_mut(&self) -> &mut T{
        unsafe{
            if self.locked.compare_exchange(false, true, atomic::Ordering::Acquire, atomic::Ordering::Relaxed).is_err(){
              panic!("Already mutably borrowed")
            };

            &mut *self.inner.get()
        }
    }
}


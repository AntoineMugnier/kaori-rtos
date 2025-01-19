pub use core::borrow::Borrow;
use core::cell::UnsafeCell;
use core::sync::atomic::{self, AtomicBool};

pub struct AsyncCell<T>{
    inner: UnsafeCell<T>
}
impl <T>AsyncCell<T>{
    pub const fn new(inner: T) -> AsyncCell<T>{
        AsyncCell{inner: UnsafeCell::new(inner)}
    }
    pub const fn borrow<'cs>(&'cs self) -> &'cs T {
        unsafe { &*self.inner.get() }
    }
}

// impl <T>Borrow<T> for AsyncCell<T>{
//     fn borrow(&self) -> &T {
//             unsafe { &*self.inner.get() }
//     }
// }

unsafe impl <T: Send>Sync for AsyncCell<T>{
}
// pub struct AsyncRefCell<T>{
//     inner: UnsafeCell<T>,
//     locked: AtomicBool
// }
// unsafe impl <T: Send>Sync for AsyncRefCell<T>{
// }

// impl <T>AsyncRefCell<T>{
//     pub const fn new(val: T) -> AsyncRefCell<T>{
//         AsyncRefCell{inner :UnsafeCell::new(val), locked: AtomicBool::new(false)}
//     }

//     pub const fn borrow_mut(&self) -> &mut T{
//         unsafe{
//             // if self.locked.compare_exchange(false, true, atomic::Ordering::Acquire, atomic::Ordering::Relaxed).is_err(){
//             //   panic!("Already mutably borrowed")
//             // };

//             &mut *self.inner.get()
//         }
//     }
// }


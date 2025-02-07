pub use core::borrow::Borrow;
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::sync::atomic::{self, AtomicBool};

pub struct AsyncArrayCell<T, const SIZE : usize>{
    inner: UnsafeCell<[T; SIZE ]>
}

impl <T, const SIZE : usize>AsyncArrayCell<T, SIZE>{
    pub const fn new(inner: [T; SIZE]) -> AsyncArrayCell<T, SIZE>{
        AsyncArrayCell{inner: UnsafeCell::new(inner)}
    }
    pub const fn get<'cs>(&'cs self) -> *mut [T] {
        self.inner.get()
    }
    pub const fn borrow_mut(&self) -> AsyncArrayCellRef<T> {
        AsyncArrayCellRef{inner: self.inner.get(), marker: PhantomData} 
    }
}

unsafe impl <T, const SIZE : usize > Sync for AsyncArrayCell<T, SIZE>{
}

pub struct AsyncArrayCellRef<'a, T>{
    inner: *mut [T],
    marker: PhantomData<&'a mut T>,
}

impl <'a, T> AsyncArrayCellRef<'a, T> {
    pub fn deref_mut(&self) -> &mut [T] {
        unsafe{
            self.inner.as_mut().unwrap()
        }
    }
}

unsafe impl <'a, T> Sync for AsyncArrayCellRef<'a, T>{
}

impl <'a, T>Deref for AsyncArrayCellRef<'a, T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        unsafe{
           self.inner.as_ref().unwrap() 
        }
    }
}

// impl <'a, T>DerefMut for AsyncArrayCellRef<'a, T> {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         unsafe{
//             self.inner.as_mut().unwrap()
//         }
//     }
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


use core::{mem::size_of, ops::Deref};
use super::allocator::SharedAllocator;


struct EvtBox<T>{
    inner: *mut T
}

unsafe impl <T> Send for EvtBox<T>{

}

static EVENT_ALLOCATOR: SharedAllocator = SharedAllocator::new();

impl <T>EvtBox<T>{
    pub(crate) fn new(element: T) -> EvtBox<T>{
        unsafe{
            let inner = EVENT_ALLOCATOR.allocate(size_of::<T>()).unwrap();
            let inner = inner as *mut T;
            *inner = element;
            EvtBox {inner}
        }
    }
}

impl<T> Drop for EvtBox<T>{
    fn drop(&mut self){
        unsafe{            
            core::ptr::drop_in_place(self.inner);
            EVENT_ALLOCATOR.free(self.inner as *mut u8).unwrap();
        }
    }
}

impl <T>Deref for EvtBox<T>{
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe{
            core::mem::transmute(self.inner)
        }
    }
}

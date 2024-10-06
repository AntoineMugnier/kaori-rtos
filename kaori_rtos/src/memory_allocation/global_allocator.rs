use super::allocator;
use crate::port::interrupt;
use core::cell::RefCell;

pub struct GlobalAllocator{
    inner_allocator: interrupt::Mutex<RefCell<Option<allocator::Allocator<'static>>>>
}

// Wrapper of an allocator that offers interior mutability
impl <'a>GlobalAllocator{
    pub const fn new() -> GlobalAllocator{
        return GlobalAllocator{inner_allocator: interrupt::Mutex::new(core::cell::RefCell::new(None))}
    }

    pub(crate) unsafe fn allocate(&self, size: usize) -> allocator::AllocationResult {
        self.alloc_op(|allocator| {
            return allocator::Allocator::allocate(allocator, size);
        })
    }

    pub(crate) unsafe fn free(&self, ptr: *mut u8) -> allocator::FreeResult {
        self.alloc_op(|allocator| {
            return allocator::Allocator::free(allocator, ptr);
        })
    }

    fn alloc_op<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut allocator::Allocator) -> R,
    {
        interrupt::free(|cs| {
            let mut allocator_opt = self.inner_allocator.borrow(cs).borrow_mut();
            if let Some(allocator) = allocator_opt.as_mut() {
                return f(allocator);
            } else {
                panic!("Calling an uninitialized allocator");
            }
        })
    }
}

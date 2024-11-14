pub mod memory_pool_allocator;
use core::borrow::BorrowMut;

use crate::port::{interrupt, Mutex};

pub(crate) type AllocationResult = Result<*mut u8, AllocationError>;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AllocationError {
    NullAllocation,
    NoMemoryAvailable,
    NoSlotLargeEnough,
}

pub(crate) type FreeResult = Result<(), FreeError>;
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum FreeError {
    UnalignedAddress,
    OutOfRangeAddress
}
pub trait LocalAllocator{

unsafe fn allocate(&mut self, size: usize) -> AllocationResult;
unsafe fn free(&mut self, ptr: *mut u8) -> FreeResult;
}

pub trait  GlobalAllocator<T: LocalAllocator>{

    unsafe fn allocate(&self, size: usize) -> AllocationResult {
        self.alloc_op(|allocator| {
            return LocalAllocator::allocate(allocator, size);
        })
    }

    unsafe fn free(&self, ptr: *mut u8) -> FreeResult {
        self.alloc_op(|allocator| {
            return LocalAllocator::free(allocator, ptr);
        })
    }

    fn acquire_lock(&self) -> &Mutex<core::cell::RefCell<T>>;

    fn alloc_op<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        interrupt::free(|cs| {
            let mutex = self.acquire_lock();
            let mut allocator = mutex.borrow(cs).borrow_mut();
            
            return f(allocator.borrow_mut());
        })
    }
}


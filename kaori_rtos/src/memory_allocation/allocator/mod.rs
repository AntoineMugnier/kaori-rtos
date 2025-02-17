use core::fmt::Debug;

pub mod memory_pool_allocator;


pub trait Allocator<PointerType, FreeErrorType: Debug, AllocationErrorType: Debug>{
    fn allocate(&self, layout: core::alloc::Layout) -> Result<PointerType, AllocationErrorType>;
    unsafe fn free(&self, slot_pointer: PointerType) -> Result<(), FreeErrorType>;
}

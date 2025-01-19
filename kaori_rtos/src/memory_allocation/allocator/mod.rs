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

pub trait  GlobalAllocator{

    unsafe fn allocate(&self, size: usize) -> AllocationResult ;

    unsafe fn free(&self, ptr: *mut u8) -> FreeResult;
}



mod allocator;
mod memory_pool;

pub use memory_pool::{SlotPool,types::MemPoolId,  SlotPointer, MemoryPool, SlotAllocError, SlotAccessError, SlotFreeingError};

pub trait MemoryAccessor<PointerType>{
    fn get_slot_mut(
        &self,
        slot_pointer: &SlotPointer,
    ) -> Result<*mut u8, ()>;
}

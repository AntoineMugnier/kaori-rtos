use self::memory_pool::SlotPointer;

pub mod allocator;
// pub mod global_allocator;
pub mod memory_pool;
pub trait MemoryAccessor<PointerType>{

    fn get_slot_mut(
        &self,
        slot_pointer: SlotPointer,
    ) -> Result<*mut u8, ()>;
}

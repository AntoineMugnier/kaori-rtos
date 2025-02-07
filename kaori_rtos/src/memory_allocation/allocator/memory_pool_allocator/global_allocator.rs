use super::allocator::MemoryPoolAllocator;
use super::global_memory_pool::GlobalMemoryPoolArrayRef;
use crate::{port::{Mutex, interrupt}};
use crate::memory_allocation::allocator::GlobalAllocator;
use crate::sync::AsyncCell;
use core::cell::RefCell;

pub struct GlobalMemoryPoolAllocator<'a>{
    inner_allocator: Mutex<RefCell<MemoryPoolAllocator<'a>>>
}

// Wrapper of an allocator that offers interior mutability
impl <'a>GlobalMemoryPoolAllocator<'a>{

    pub const fn new(memory_pool_array_ref :  GlobalMemoryPoolArrayRef<'a>) -> GlobalMemoryPoolAllocator<'a>{
         let allocator = MemoryPoolAllocator::new(memory_pool_array_ref.inner_array);
        return GlobalMemoryPoolAllocator{inner_allocator: Mutex::new(core::cell::RefCell::new(allocator))}
    }
}

impl <'a> GlobalAllocator for GlobalMemoryPoolAllocator<'a>{
    unsafe fn free(&self, ptr: *mut u8) -> crate::memory_allocation::allocator::FreeResult {
        self.inner_allocator
    }
}

#[cfg(test)]
mod test{

    use super::*;
    use super::super::global_memory_pool::{GlobalMemoryPool, GlobalStaticPool, GlobalMemoryPoolArray}; 
    const POOL0_WORDS_PER_SLOT: usize = 1;
    const POOL0_WORDS_PER_POOL: usize = 2 * POOL0_WORDS_PER_SLOT;
    static STATIC_POOL_0 : GlobalStaticPool::<POOL0_WORDS_PER_POOL> = GlobalStaticPool::new(POOL0_WORDS_PER_SLOT);
    static TEST_POOL0: GlobalMemoryPool = GlobalMemoryPool::new(STATIC_POOL_0.get());
    static TEST_POOL_ARRAY: GlobalMemoryPoolArray::<1> = GlobalMemoryPoolArray::<1>::new([&TEST_POOL0]);
    static TEST_ALLOCATOR0: GlobalMemoryPoolAllocator = GlobalMemoryPoolAllocator::new(TEST_POOL_ARRAY.as_ref());

    #[test]
    fn global_allocator_test() {
    }
}


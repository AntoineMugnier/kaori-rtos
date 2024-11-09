use super::allocator::MemoryPoolAllocator;
use super::global_memory_pool::GlobalMemoryPoolArrayRef;
use crate::{port::{Mutex, interrupt}, memory_allocation::allocator::LocalAllocator};
use core::cell::RefCell;

pub struct GlobalMemoryPoolAllocator<'a>{
    inner_allocator: Mutex<RefCell<Option<MemoryPoolAllocator<'a>>>>
}

// Wrapper of an allocator that offers interior mutability
impl <'a>GlobalMemoryPoolAllocator<'a>{
    pub const fn default() -> GlobalMemoryPoolAllocator<'a>{
        return GlobalMemoryPoolAllocator{inner_allocator: Mutex::new(core::cell::RefCell::new(None))}
    }

    pub  fn set(&self, memory_pool_array_ref :  GlobalMemoryPoolArrayRef<'a>){
         let allocator = MemoryPoolAllocator::new(memory_pool_array_ref.inner_array);
        interrupt::free(|cs| {
            self.inner_allocator.borrow(cs).borrow_mut().insert(allocator);
        })
    }
}

impl <'a> super::super::GlobalAllocator<MemoryPoolAllocator<'a>> for GlobalMemoryPoolAllocator<'a>{
    fn acquire_lock(&self) -> &Mutex<core::cell::RefCell<Option<MemoryPoolAllocator<'a>>>> {
        &self.inner_allocator
    }
}

#[cfg(test)]
mod test{

    use super::*;
    use super::super::global_memory_pool::{GlobalMemoryPool, GlobalStaticPool, GlobalMemoryPoolArray}; 
     const POOL0_SLOT_SIZE: usize = std::mem::size_of::<usize>();
     const POOL0_SIZE: usize = 2 * POOL0_SLOT_SIZE;
    static STATIC_POOL_0 : GlobalStaticPool::<POOL0_SIZE> = GlobalStaticPool::<POOL0_SIZE>::new();
     
    static TEST_POOL0: GlobalMemoryPool = GlobalMemoryPool::default();
    static TEST_POOL_ARRAY_0: GlobalMemoryPoolArray::<1> = GlobalMemoryPoolArray::<1>::default();
    static TEST_ALLOCATOR0: GlobalMemoryPoolAllocator = GlobalMemoryPoolAllocator::default();

    #[test]
    fn global_allocator_test() {
        TEST_POOL0.set(STATIC_POOL_0.as_ref(), POOL0_SLOT_SIZE);
        TEST_POOL_ARRAY_0.set([&TEST_POOL0]);
        TEST_ALLOCATOR0.set(TEST_POOL_ARRAY_0.as_ref());
    }
}


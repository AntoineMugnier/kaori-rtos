use super::{
    memory_pool::{types::MemPoolId, MemoryPool, SlotAllocError, SlotFreeingError, SlotPointer},
    MemoryAccessor,
};
use crate::memory_allocation::allocator::Allocator;
pub(crate) type AllocationResult = Result<SlotPointer, AllocationError>;
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AllocationError {
    NullAllocation,
    NoMemoryAvailable,
    NoSlotLargeEnough,
}

pub(crate) type FreeResult = Result<(), FreeError>;
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum FreeError {
    InvalidSlotIndex,
    InvalidMemoryPoolId,
}

pub(crate) struct MemoryPoolAllocator<'a> {
    memory_pool_array: &'a [&'a MemoryPool<'a>],
}

impl<'a> MemoryPoolAllocator<'a> {
    const fn check_memory_pools_order(
        memory_pool_array: &[&MemoryPool<'a>],
        mut bigger_slot_size: usize,
        expected_mem_pool_id: MemPoolId,
    ) {
        match memory_pool_array {
            [] => {}
            [first, rest @ ..] => {
                assert!(
                    first.get_slot_size() > bigger_slot_size,
                    "Memory pools must be listed in ascending order"
                );
                bigger_slot_size = first.get_slot_size();
                let mem_pool_id = first.get_mem_pool_id();
                assert!(
                    mem_pool_id == expected_mem_pool_id,
                    "Memory pools in the array served to the allocator are not sorted by id"
                );
                Self::check_memory_pools_order(rest, bigger_slot_size, expected_mem_pool_id + 1)
            }
        }
    }

    pub const fn new(memory_pool_array: &'a [&'a MemoryPool<'a>]) -> MemoryPoolAllocator<'a> {
        assert!(
            memory_pool_array.len() > 0,
            "At least one memory pool must be defined"
        );
        let bigger_slot_size = 0;
        Self::check_memory_pools_order(memory_pool_array, bigger_slot_size, 0);
        return MemoryPoolAllocator { memory_pool_array };
    }

    fn get_slot_mut(&self, slot_pointer: &SlotPointer) -> Result<*mut u8, ()> {
        let memory_pool_id = slot_pointer.get_mem_pool_id();
        self.memory_pool_array[memory_pool_id as usize].get_slot_mut(*slot_pointer)
    }

    fn allocate(&self, layout: core::alloc::Layout) -> AllocationResult {
        if layout.size() == 0 {
            return Err(AllocationError::NullAllocation);
        }

        for memory_pool in self.memory_pool_array.iter() {
            match memory_pool.try_allocate_slot(layout) {
                Result::Ok(slot_pointer) => return Ok(slot_pointer),
                Result::Err(err) => match err {
                    SlotAllocError::SlotNotLargeEnough => continue,
                    SlotAllocError::PoolFull => return Err(AllocationError::NoMemoryAvailable),
                },
            }
        }
        return Err(AllocationError::NoSlotLargeEnough);
    }

    unsafe fn free(&self, slot_pointer: SlotPointer) -> FreeResult {
        let memory_pool_index = slot_pointer.get_mem_pool_id() as usize;
        if memory_pool_index >= self.memory_pool_array.len() {
            return Err(FreeError::InvalidMemoryPoolId);
        }
        let memory_pool = self.memory_pool_array[memory_pool_index];

        if let Err(err) = memory_pool.try_free_slot(slot_pointer) {
            match err {
                SlotFreeingError::SlotOutOfRange => return Err(FreeError::InvalidSlotIndex),
            }
        } else {
            Ok(())
        }
    }
}
impl<'a> Allocator<SlotPointer, FreeError, AllocationError> for MemoryPoolAllocator<'a> {
    unsafe fn free(&self, slot_pointer: SlotPointer) -> Result<(), FreeError> {
        Self::free(self, slot_pointer)
    }

    fn allocate(&self, layout: core::alloc::Layout) -> Result<SlotPointer, AllocationError> {
        Self::allocate(self, layout)
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::memory_pool::{types::MemPoolId, SlotPool};
    use super::*;

    // Single pool test
    mod basic_mem_pool_allocator_test {
        use crate::memory_allocation::allocator::memory_pool_allocator::memory_pool::SlotPool;
        use core::alloc::Layout;

        use super::*;
        const POOL0_ID: MemPoolId = 0;
        const POOL0_WORDS_PER_SLOT: usize = 1;
        const POOL0_SLOTS_PER_POOL: usize = 2;
        const POOL0_WORDS_PER_POOL: usize = POOL0_SLOTS_PER_POOL * POOL0_WORDS_PER_SLOT;
        static STATIC_MEMORY_POOL: SlotPool<POOL0_WORDS_PER_POOL> =
            SlotPool::<POOL0_WORDS_PER_POOL>::new(POOL0_WORDS_PER_SLOT, POOL0_ID);
        static MEMORY_POOL_0: MemoryPool = MemoryPool::from(&STATIC_MEMORY_POOL);

        static MEMORY_POOL_ARRAY_0: [&MemoryPool; 1] = [&MEMORY_POOL_0];
        static ALLOCATOR_0: MemoryPoolAllocator = MemoryPoolAllocator::new(&MEMORY_POOL_ARRAY_0);

        #[test]
        fn mem_pool_allocator_test_0() {
            unsafe {
                struct Struct0 {
                    a: usize,
                }

                struct Struct1 {}

                for _ in 0..4 {
                    let res0 = ALLOCATOR_0.allocate(Layout::new::<Struct0>());
                    let res0 = res0.unwrap();
                    let struct0_0: &mut Struct0 = MEMORY_POOL_0.get_slot_transmute(res0).unwrap();
                    *struct0_0 = Struct0 {
                        a: core::usize::MAX,
                    };

                    let res1 = ALLOCATOR_0.allocate(Layout::new::<Struct0>());
                    let res1 = res1.unwrap();
                    let struct0_1: &mut Struct0 = MEMORY_POOL_0.get_slot_transmute(res1).unwrap();
                    *struct0_1 = Struct0 {
                        a: core::usize::MIN,
                    };

                    let res2 = ALLOCATOR_0.allocate(Layout::new::<Struct0>());
                    assert_eq!(res2, Err(AllocationError::NoMemoryAvailable));

                    let res5 = ALLOCATOR_0.allocate(Layout::new::<Struct1>());
                    assert_eq!(res5, Err(AllocationError::NullAllocation));

                    assert_eq!(struct0_0.a, core::usize::MAX);
                    assert_eq!(struct0_1.a, core::usize::MIN);

                    let res1 = ALLOCATOR_0.free(res1);
                    assert_eq!(res1, Ok(()));
                    assert_eq!(struct0_0.a, core::usize::MAX);

                    let res0 = ALLOCATOR_0.free(res0);
                    assert_eq!(res0, Ok(()));
                }
            }
        }
    }
}
//     // Multiple pools test
//     #[test]
//     fn mem_pool_allocator_test_1() {
//         const POOL0_WORDS_PER_SLOT: usize = 1;
//         const POOL0_SLOTS_PER_POOL: usize = 2;
//         let mut static_memory_pool = StaticMemoryPool::<POOL0_SLOTS_PER_POOL>::new(POOL0_WORDS_PER_SLOT);
//         let mut mm0: MemoryPool = MemoryPool::new(&mut static_memory_pool);
//         let pool0_slot_size = mm0.get_slot_size();

//         const POOL1_WORDS_PER_SLOT: usize = 2;
//         const POOL1_SLOTS_PER_POOL: usize = 1;
//         let mut static_memory_pool = StaticMemoryPool::<POOL1_SLOTS_PER_POOL>::new(POOL1_WORDS_PER_SLOT);
//         let mut mm1: MemoryPool = MemoryPool::new(&mut static_memory_pool);
//         let pool1_slot_size = mm1.get_slot_size();
//         let mut m = [&mut mm0, &mut mm1];
//         let mut allocator= MemoryPoolAllocator::new(&mut m);

//         struct Struct0{
//             d: usize
//         }

//         struct Struct1{
//             d0: usize,
//             d1: usize
//         }

//         unsafe{
//             for _ in 0..4{
//                 let res0 = allocator.allocate(pool1_slot_size +1);
//                 assert_eq!(res0, Err(AllocationError::NoSlotLargeEnough));
//                 let res1 = allocator.allocate(pool0_slot_size -1).unwrap();
//                 let struct0_0 : &mut Struct0 = core::mem::transmute(res1);
//                 *struct0_0 = Struct0{d: core::usize::MAX};

//                 let res1_1 = allocator.free(res1.add(1));
//                 assert_eq!(res1_1, Err(FreeError::UnalignedAddress));
//                 assert_eq!(struct0_0.d, core::usize::MAX);
//                 allocator.free(res1).unwrap();

//                 let res2 = allocator.allocate(pool1_slot_size).unwrap();
//                 let struct0_1 : &mut Struct1 = core::mem::transmute(res2);
//                 *struct0_1 = Struct1{d0: 0xBBBBBBBBBBBBBBBB, d1: 0xCCCCCCCCCCCCCCCC};

//                 let res3 = allocator.allocate(pool1_slot_size);
//                 assert_eq!(res3, Err(AllocationError::NoMemoryAvailable));
//
//                 assert_eq!(struct0_1.d0, 0xBBBBBBBBBBBBBBBB);
//                 assert_eq!(struct0_1.d1, 0xCCCCCCCCCCCCCCCC);
//                 allocator.free(res2).unwrap();

//                 let res4 = allocator.allocate(pool1_slot_size).unwrap();
//                 let struct0_2 : &mut Struct1 = core::mem::transmute(res2);
//                 *struct0_2 = Struct1{d0: 0xEEEEEEEEEEEEEEEE, d1: 0x6666666666666666};

//                 let res5 = allocator.allocate(0);
//                 assert_eq!(res5, Err(AllocationError::NullAllocation));

//                 let res4_1 = allocator.free(res4.add(pool1_slot_size));
//                 assert_eq!(res4_1, Err(FreeError::OutOfRangeAddress));

//                 assert_eq!(struct0_2.d0, 0xEEEEEEEEEEEEEEEE);
//                 assert_eq!(struct0_2.d1, 0x6666666666666666);
//                 allocator.free(res4).unwrap();
//             }
//         }
//      }
//  }

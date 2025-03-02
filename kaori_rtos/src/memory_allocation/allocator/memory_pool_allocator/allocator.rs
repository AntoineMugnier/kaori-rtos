use super::{
    memory_pool::{types::MemPoolId, MemoryPool, SlotAllocError, SlotFreeingError, SlotPointer},
    MemoryAccessor,
};
use crate::memory_allocation::allocator::Allocator;
pub type AllocationResult = Result<SlotPointer, AllocationError>;
#[derive(Debug, PartialEq, Eq)]
pub enum AllocationError {
    NullAllocation,
    NoMemoryAvailable,
    NoSlotLargeEnough,
}

pub type FreeResult = Result<(), FreeError>;
#[derive(Debug, PartialEq, Eq)]
pub enum FreeError {
    InvalidSlotIndex,
    InvalidMemoryPoolId,
}

pub struct MemoryPoolAllocator<'a> {
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
        self.memory_pool_array[memory_pool_id as usize].get_slot_mut(slot_pointer)
    }

    fn allocate(&self, layout: core::alloc::Layout) -> AllocationResult {
        if layout.size() == 0 {
            return Err(AllocationError::NullAllocation);
        }

        if layout.size() > self.memory_pool_array.last().unwrap().get_slot_size() {
            return Err(AllocationError::NoSlotLargeEnough);
        }

        for memory_pool in self.memory_pool_array.iter() {
            match memory_pool.allocate(layout) {
                Result::Ok(slot_pointer) => return Ok(slot_pointer),
                Result::Err(err) => match err {
                    SlotAllocError::SlotNotLargeEnough => continue,
                    SlotAllocError::PoolFull => continue,
                },
            }
        }
        return Err(AllocationError::NoMemoryAvailable);
    }

    unsafe fn free(&self, slot_pointer: SlotPointer) -> FreeResult {
        let memory_pool_index = slot_pointer.get_mem_pool_id() as usize;
        if memory_pool_index >= self.memory_pool_array.len() {
            return Err(FreeError::InvalidMemoryPoolId);
        }
        let memory_pool = self.memory_pool_array[memory_pool_index];

        if let Err(err) = memory_pool.free(slot_pointer) {
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

impl<'a> MemoryAccessor<SlotPointer> for MemoryPoolAllocator<'a> {
    fn get_slot_mut(&self, slot_pointer: &SlotPointer) -> Result<*mut u8, ()> {
        self.get_slot_mut(slot_pointer).map_err(|_| ())
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
                    let struct0_0: &mut Struct0 = MEMORY_POOL_0.get_slot_transmute(&res0).unwrap();
                    *struct0_0 = Struct0 {
                        a: core::usize::MAX,
                    };

                    let res1 = ALLOCATOR_0.allocate(Layout::new::<Struct0>());
                    let res1 = res1.unwrap();
                    let struct0_1: &mut Struct0 = MEMORY_POOL_0.get_slot_transmute(&res1).unwrap();
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

    mod single_thread_randomized {

        use crate::memory_allocation::allocator::memory_pool_allocator::memory_pool::tests::PoolTestParams;

        use super::*;
        const POOL0_ID: MemPoolId = 0;
        const POOL0_WORDS_PER_SLOT: usize = 1;
        const POOL0_SLOTS_PER_POOL: usize = 10;
        const POOL0_WORDS_PER_POOL: usize = POOL0_SLOTS_PER_POOL * POOL0_WORDS_PER_SLOT;
        static STATIC_MEMORY_POOL_0: SlotPool<POOL0_WORDS_PER_POOL> =
            SlotPool::<POOL0_WORDS_PER_POOL>::new(POOL0_WORDS_PER_SLOT, POOL0_ID);
        static MEMORY_POOL_0: MemoryPool = MemoryPool::from(&STATIC_MEMORY_POOL_0);

        const POOL1_ID: MemPoolId = 1;
        const POOL1_WORDS_PER_SLOT: usize = 3;
        const POOL1_SLOTS_PER_POOL: usize = 8;
        const POOL1_WORDS_PER_POOL: usize = POOL1_SLOTS_PER_POOL * POOL1_WORDS_PER_SLOT;
        static STATIC_MEMORY_POOL_1: SlotPool<POOL1_WORDS_PER_POOL> =
            SlotPool::<POOL1_WORDS_PER_POOL>::new(POOL1_WORDS_PER_SLOT, POOL1_ID);
        static MEMORY_POOL_1: MemoryPool = MemoryPool::from(&STATIC_MEMORY_POOL_1);

        const POOL2_ID: MemPoolId = 2;
        const POOL2_WORDS_PER_SLOT: usize = 8;
        const POOL2_SLOTS_PER_POOL: usize = 4;
        const POOL2_WORDS_PER_POOL: usize = POOL2_SLOTS_PER_POOL * POOL2_WORDS_PER_SLOT;
        static STATIC_MEMORY_POOL_2: SlotPool<POOL2_WORDS_PER_POOL> =
            SlotPool::<POOL2_WORDS_PER_POOL>::new(POOL2_WORDS_PER_SLOT, POOL2_ID);
        static MEMORY_POOL_2: MemoryPool = MemoryPool::from(&STATIC_MEMORY_POOL_2);

        static MEMORY_POOL_ARRAY_0: [&MemoryPool; 3] =
            [&MEMORY_POOL_0, &MEMORY_POOL_1, &MEMORY_POOL_2];
        static ALLOCATOR_0: MemoryPoolAllocator = MemoryPoolAllocator::new(&MEMORY_POOL_ARRAY_0);
        use super::super::super::memory_pool::tests::{TestParams, Tester};
        #[test]
        fn single_thread_randomized() {
            let pool_test_params_0 = [
                PoolTestParams {
                    max_n_elements: POOL0_SLOTS_PER_POOL,
                    max_element_size: POOL0_WORDS_PER_SLOT * core::mem::size_of::<usize>(),
                    n_initial_elements: 5,
                },
                PoolTestParams {
                    max_n_elements: POOL1_SLOTS_PER_POOL,
                    max_element_size: POOL1_WORDS_PER_SLOT * core::mem::size_of::<usize>(),
                    n_initial_elements: 4,
                },
                PoolTestParams {
                    max_n_elements: POOL2_SLOTS_PER_POOL,
                    max_element_size: POOL2_WORDS_PER_SLOT * core::mem::size_of::<usize>(),
                    n_initial_elements: 2,
                },
            ];

            let test_params = TestParams {
                pool_test_params: &pool_test_params_0,
                n_iterations: 10000,
            };

            let mut tester = Tester::new(&ALLOCATOR_0);
            tester.run(test_params);
        }
    }

    mod multi_thread_randomized {

        use std::thread;

        use crate::memory_allocation::allocator::memory_pool_allocator::memory_pool::tests::PoolTestParams;

        use super::*;
        const POOL0_ID: MemPoolId = 0;
        const POOL0_WORDS_PER_SLOT: usize = 1;
        const POOL0_SLOTS_PER_POOL: usize = 10;
        const POOL0_WORDS_PER_POOL: usize = POOL0_SLOTS_PER_POOL * POOL0_WORDS_PER_SLOT;
        static STATIC_MEMORY_POOL_0: SlotPool<POOL0_WORDS_PER_POOL> =
            SlotPool::<POOL0_WORDS_PER_POOL>::new(POOL0_WORDS_PER_SLOT, POOL0_ID);
        static MEMORY_POOL_0: MemoryPool = MemoryPool::from(&STATIC_MEMORY_POOL_0);

        const POOL1_ID: MemPoolId = 1;
        const POOL1_WORDS_PER_SLOT: usize = 3;
        const POOL1_SLOTS_PER_POOL: usize = 8;
        const POOL1_WORDS_PER_POOL: usize = POOL1_SLOTS_PER_POOL * POOL1_WORDS_PER_SLOT;
        static STATIC_MEMORY_POOL_1: SlotPool<POOL1_WORDS_PER_POOL> =
            SlotPool::<POOL1_WORDS_PER_POOL>::new(POOL1_WORDS_PER_SLOT, POOL1_ID);
        static MEMORY_POOL_1: MemoryPool = MemoryPool::from(&STATIC_MEMORY_POOL_1);

        const POOL2_ID: MemPoolId = 2;
        const POOL2_WORDS_PER_SLOT: usize = 8;
        const POOL2_SLOTS_PER_POOL: usize = 4;
        const POOL2_WORDS_PER_POOL: usize = POOL2_SLOTS_PER_POOL * POOL2_WORDS_PER_SLOT;
        static STATIC_MEMORY_POOL_2: SlotPool<POOL2_WORDS_PER_POOL> =
            SlotPool::<POOL2_WORDS_PER_POOL>::new(POOL2_WORDS_PER_SLOT, POOL2_ID);
        static MEMORY_POOL_2: MemoryPool = MemoryPool::from(&STATIC_MEMORY_POOL_2);

        static MEMORY_POOL_ARRAY_0: [&MemoryPool; 3] =
            [&MEMORY_POOL_0, &MEMORY_POOL_1, &MEMORY_POOL_2];
        static ALLOCATOR_0: MemoryPoolAllocator = MemoryPoolAllocator::new(&MEMORY_POOL_ARRAY_0);
        use super::super::super::memory_pool::tests::{TestParams, Tester};

        const NB_THREADS: usize = 2;
        #[test]
        fn multi_thread_randomized() {
            let mut join_handle_vec = Vec::new();
            for _ in 0..NB_THREADS {
                let join_hande = thread::spawn(move || {
                    let pool_test_params = [
                        PoolTestParams {
                            max_n_elements: POOL0_SLOTS_PER_POOL / NB_THREADS,
                            max_element_size: POOL0_WORDS_PER_SLOT * core::mem::size_of::<usize>(),
                            n_initial_elements: 5 / NB_THREADS,
                        },
                        PoolTestParams {
                            max_n_elements: POOL1_SLOTS_PER_POOL / NB_THREADS,
                            max_element_size: POOL1_WORDS_PER_SLOT * core::mem::size_of::<usize>(),
                            n_initial_elements: 4 / NB_THREADS,
                        },
                        PoolTestParams {
                            max_n_elements: POOL2_SLOTS_PER_POOL / NB_THREADS,
                            max_element_size: POOL2_WORDS_PER_SLOT * core::mem::size_of::<usize>(),
                            n_initial_elements: 2 / NB_THREADS,
                        },
                    ];

                    let test_params = TestParams {
                        pool_test_params: &pool_test_params,
                        n_iterations: 100000,
                    };
                    let mut tester = Tester::new(&ALLOCATOR_0);
                    tester.run(test_params);
                });
                join_handle_vec.push(join_hande);
            }
            for join_handle in join_handle_vec.into_iter(){
                join_handle.join().unwrap();
            }
        }
    }
}

use super::memory_pool::{MemoryPool, SlotFreeingError, SlotAllocError};
use super::super::{FreeResult, FreeError, AllocationResult, AllocationError, LocalAllocator};


pub(crate) struct MemoryPoolAllocator<'a> {
    memory_pool_array: &'a mut [&'a mut MemoryPool<'a>],
}


impl<'a> MemoryPoolAllocator<'a> {

    pub fn new(memory_pool_array: &'a mut [&'a mut MemoryPool<'a>]) -> MemoryPoolAllocator {
        assert!(memory_pool_array.len() > 0, "At least one memory pool must be defined");
        let mut bigger_slot_size = 0;
        for memory_pool in memory_pool_array.iter(){
            assert!(memory_pool.slot_size != bigger_slot_size, "Memory pools cannot have the same slot size");
            assert!(memory_pool.slot_size > bigger_slot_size, "Memory pools must be listed in ascending order");
            bigger_slot_size = memory_pool.slot_size;
        }
        return MemoryPoolAllocator { memory_pool_array };
    }

    pub(crate) unsafe fn allocate(&mut self, size: usize) -> AllocationResult {
        
        if size == 0{
            return Err(AllocationError::NullAllocation);
        }

        for memory_pool in self.memory_pool_array.iter_mut() {
            match memory_pool.try_allocate_slot(size){
                Result::Ok(address) => return Ok(address),
                Result::Err(err) => match err {
                    SlotAllocError::SlotNotLargeEnough => continue,
                    SlotAllocError::PoolFull => return Err(AllocationError::NoMemoryAvailable)
                },
            }
        }
        return Err(AllocationError::NoSlotLargeEnough);
    }

    pub(crate) unsafe fn free(&mut self, ptr: *mut u8) -> FreeResult {
        for memory_pool in self.memory_pool_array.iter_mut() {
            match memory_pool.try_free_slot(ptr) {
                Result::Ok(()) => return Ok(()),
                Result::Err(err) => match err {
                    SlotFreeingError::SlotOutOfRange => continue,
                    SlotFreeingError::UnalignedSlot => return Err(FreeError::UnalignedAddress)
                },
            }
        }
        return Err(FreeError::OutOfRangeAddress)
    }
}
impl <'a> LocalAllocator for MemoryPoolAllocator<'a>{
    unsafe fn free(&mut self, ptr: *mut u8) -> crate::memory_allocation::allocator::FreeResult {
       MemoryPoolAllocator::free(self, ptr)
    }
    unsafe fn allocate(&mut self, size: usize) -> AllocationResult {
        MemoryPoolAllocator::allocate(self, size)
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    // Single pool test
    #[test]
    fn mem_pool_allocator_test_0() {
        const POOL0_SIZE: usize = core::mem::size_of::<usize>();
        let mut pool0: [u8; POOL0_SIZE*2] = [0; POOL0_SIZE * 2];
        let mut mm0: MemoryPool = MemoryPool::new(&mut pool0, POOL0_SIZE);
        let mut m = [&mut mm0];
        let mut allocator = MemoryPoolAllocator::new(&mut m);
    unsafe{
        struct Struct0{
            a: usize
        }

        for _ in 0..4{     
            let res0 =  allocator.allocate(core::mem::size_of::<Struct0>());
            let res0 = res0.unwrap();
            let struct0_0 : &mut Struct0 = core::mem::transmute(res0);
            *struct0_0 = Struct0{a: core::usize::MAX};
            
            let res1 =  allocator.allocate(core::mem::size_of::<Struct0>());
            let res1 = res1.unwrap();
            let struct0_1 : &mut Struct0 = core::mem::transmute(res1);
            *struct0_1 = Struct0{a: core::usize::MIN};

            let res2 =  allocator.allocate(core::mem::size_of::<Struct0>());
            assert_eq!(res2, Err(AllocationError::NoMemoryAvailable));
            
            assert_eq!(struct0_0.a, core::usize::MAX);
            assert_eq!(struct0_1.a, core::usize::MIN);

            let res1 = allocator.free(res1);
            assert_eq!(res1, Ok(()));
            assert_eq!(struct0_0.a, core::usize::MAX);

            let res0 = allocator.free(res0);
            assert_eq!(res0, Ok(()));
            }
        }
    }

    // Multiple pools test
    #[test]
    fn mem_pool_allocator_test_1() {
        const POOL0_SLOT_SIZE: usize = core::mem::size_of::<usize>();
        const POOL0_SIZE: usize = 2 * POOL0_SLOT_SIZE;
        const POOL1_SLOT_SIZE: usize = 16;
        const POOL1_SIZE: usize = 1 * POOL1_SLOT_SIZE;
        let mut pool0 = [0; POOL0_SIZE];
        let mut pool1 = [0; POOL1_SIZE];
        let mut mm0: MemoryPool = MemoryPool::new(&mut pool0, POOL0_SLOT_SIZE);
        let mut mm1: MemoryPool = MemoryPool::new(&mut pool1, POOL1_SLOT_SIZE);
        let mut m = [&mut mm0, &mut mm1];
        let mut allocator= MemoryPoolAllocator::new(&mut m);

        struct Struct0{
            d: usize
        }

        struct Struct1{
            d0: usize,
            d1: usize
        }

        unsafe{
            for _ in 0..4{     
                let res0 = allocator.allocate(POOL1_SLOT_SIZE +1);
                assert_eq!(res0, Err(AllocationError::NoSlotLargeEnough));
                let res1 = allocator.allocate(POOL0_SLOT_SIZE -1).unwrap();
                let struct0_0 : &mut Struct0 = core::mem::transmute(res1);
                *struct0_0 = Struct0{d: core::usize::MAX};

                let res1_1 = allocator.free(res1.add(1));
                assert_eq!(res1_1, Err(FreeError::UnalignedAddress));
                assert_eq!(struct0_0.d, core::usize::MAX);
                allocator.free(res1).unwrap();

                let res2 = allocator.allocate(POOL1_SLOT_SIZE).unwrap();
                let struct0_1 : &mut Struct1 = core::mem::transmute(res2);
                *struct0_1 = Struct1{d0: 0xBBBBBBBBBBBBBBBB, d1: 0xCCCCCCCCCCCCCCCC};

                let res3 = allocator.allocate(POOL1_SLOT_SIZE);
                assert_eq!(res3, Err(AllocationError::NoMemoryAvailable));
                
                assert_eq!(struct0_1.d0, 0xBBBBBBBBBBBBBBBB);
                assert_eq!(struct0_1.d1, 0xCCCCCCCCCCCCCCCC);
                allocator.free(res2).unwrap();

                let res4 = allocator.allocate(POOL1_SLOT_SIZE).unwrap();
                let struct0_2 : &mut Struct1 = core::mem::transmute(res2);
                *struct0_2 = Struct1{d0: 0xEEEEEEEEEEEEEEEE, d1: 0x6666666666666666};

                let res5 = allocator.allocate(0);
                assert_eq!(res5, Err(AllocationError::NullAllocation));

                let res4_1 = allocator.free(res4.add(POOL1_SLOT_SIZE));
                assert_eq!(res4_1, Err(FreeError::OutOfRangeAddress));

                assert_eq!(struct0_2.d0, 0xEEEEEEEEEEEEEEEE);
                assert_eq!(struct0_2.d1, 0x6666666666666666);
                allocator.free(res4).unwrap();
            }
        }
     }
 }

use core::result::Result;

#[derive(Debug, PartialEq, Eq)]
pub enum SlotAllocError {
    PoolFull,
    SlotNotLargeEnough,
}
type SlotAllocResult = Result<*mut u8, SlotAllocError>;

#[derive(Debug, PartialEq, Eq)]
pub enum SlotFreeingError{
    SlotOutOfRange,
    UnalignedSlot
}

type SlotFreeingResult = Result<(), SlotFreeingError>;

#[derive(Clone, Copy, Debug)]
pub struct SlotLinkedList{
    head_idx: usize,
    tail_idx: usize,
}

pub(crate) struct MemoryPool<'a> {
    pub(crate) slot_pool: &'a mut [u8],
    pub(crate) slot_size: usize,
    pub(crate) free_slot_linked_list : Option<SlotLinkedList>
}
struct EmptySlot {
    next_slot: *mut u8,
}

impl<'a> MemoryPool<'a> {
    unsafe fn init_pool_slots(slot_pool: &mut [u8], slot_size: usize){
        let slot_pool_range = slot_pool.as_ptr_range();
        let slot_pool_start = slot_pool_range.start.cast_mut();
        let slot_pool_end = slot_pool_range.end.cast_mut();

        let mut slot_pointer = slot_pool_start;
        loop {
            let empty_slot: &mut EmptySlot = core::mem::transmute(slot_pointer);
            let next_slot = slot_pointer.add(slot_size);

            if next_slot == slot_pool_end {
                *empty_slot = EmptySlot { next_slot: core::ptr::null_mut()};
                break;
            }
            else{
                *empty_slot = EmptySlot { next_slot };
                slot_pointer = next_slot;
            }
        }
    }

    pub fn new(slot_pool: &'a mut [u8], slot_size : usize) -> MemoryPool<'a> {
        
        assert!(slot_size > 0, "Slot size cannot be null");
        assert!(slot_pool.len() > 0, "Slot pool length cannot be null");
        assert!(slot_size % core::mem::size_of::<usize>() == 0, "Slot size must be a multiple of usize");
        assert!(slot_pool.len() % slot_size == 0, "Slot pool length is {} but must be a multiple of slot size which is {})", slot_pool.len(), slot_size);

        unsafe {
            Self::init_pool_slots(slot_pool, slot_size);
                
            let free_slots_head_idx = 0;
            let free_slots_tail_idx = (slot_pool.len()/slot_size) -1;
            let free_slot_linked_list = SlotLinkedList{head_idx: free_slots_head_idx,tail_idx: free_slots_tail_idx};

            MemoryPool {
                slot_pool,
                slot_size,
                free_slot_linked_list: Some(free_slot_linked_list)
            }
        }
    }
    
    unsafe fn get_free_slot_address_by_idx(&mut self, slot_idx: usize) -> *mut u8{
           self.slot_pool.as_mut_ptr().add(slot_idx*self.slot_size)
    }

    unsafe fn get_free_slot_by_address(&mut self, address: *mut u8) -> &mut EmptySlot{
            core::mem::transmute(address)
    }

    unsafe fn get_free_slot_idx_by_address(&mut self, address: *mut u8) -> usize{
            let slot_pool_range = self.slot_pool.as_ptr_range();
            let slot_pool_start = slot_pool_range.start.cast_mut();
            return (address as usize - slot_pool_start as usize)/self.slot_size
    }

    pub(crate) unsafe fn try_allocate_slot(&mut self, size: usize) -> SlotAllocResult {
        if size > self.slot_size{
            return Err(SlotAllocError::SlotNotLargeEnough);
        }
        
            let free_slot_linked_list: SlotLinkedList;
            if let Some(free_slot_linked_list_) = self.free_slot_linked_list.as_mut(){
                free_slot_linked_list = *free_slot_linked_list_;
            }
            else{
                return Err(SlotAllocError::PoolFull)
            }
            
            let slot_list_head_address = self.get_free_slot_address_by_idx(free_slot_linked_list.head_idx);
            let slot_list_head = self.get_free_slot_by_address(slot_list_head_address);
            let new_slot_list_head_address = slot_list_head.next_slot;

            if new_slot_list_head_address.is_null() {
                self.free_slot_linked_list = None;
            }
            else{
                let new_slot_list_head_idx = self.get_free_slot_idx_by_address(new_slot_list_head_address);
                self.free_slot_linked_list = Some(SlotLinkedList{head_idx: new_slot_list_head_idx, tail_idx: free_slot_linked_list.tail_idx});
            }

                return Ok(slot_list_head_address); 
    }

    pub(crate) unsafe fn try_free_slot(&mut self, slot_address: *mut u8) -> SlotFreeingResult {
        let slot_pool_range = self.slot_pool.as_ptr_range();
        let slot_pool_start = slot_pool_range.start.cast_mut();
        let slot_pool_end = slot_pool_range.end.cast_mut();
        
        // Check that the slot address to free is in address range
        if slot_address < slot_pool_start || slot_address >= slot_pool_end{
            return Err(SlotFreeingError::SlotOutOfRange);
        }
        
        // Check that the slot address to free is aligned 
        if slot_address.sub(slot_pool_start as usize) as usize % (self.slot_size) != 0{
            return Err(SlotFreeingError::UnalignedSlot)
        }
        
            //Populate new free slot
            let new_empty_slot = self.get_free_slot_by_address(slot_address);
            *new_empty_slot = EmptySlot{next_slot: core::ptr::null_mut()};

            let new_free_slots_tail_idx = self.get_free_slot_idx_by_address(slot_address);

            let free_slot_linked_list: SlotLinkedList;
            if let Some(free_slot_linked_list_) = self.free_slot_linked_list.as_mut(){
                free_slot_linked_list = *free_slot_linked_list_;
            }
            else{
                self.free_slot_linked_list = Some(SlotLinkedList{head_idx:new_free_slots_tail_idx, tail_idx: new_free_slots_tail_idx});
                return Ok(());
            }

            let slot_list_tail_address = self.get_free_slot_address_by_idx(free_slot_linked_list.tail_idx);
            let slot_list_tail = self.get_free_slot_by_address(slot_list_tail_address);
            slot_list_tail.next_slot = slot_address;
            self.free_slot_linked_list.unwrap().tail_idx = new_free_slots_tail_idx;
            return Ok(())

    }

    fn get_slot_pool_size(&self) -> usize {
        self.slot_pool.len()
    }

    fn get_slot_size(&self) -> usize {
        self.slot_size
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test]

    fn mem_pool_test_0() {
        const POOL0_SLOT_SIZE: usize = core::mem::size_of::<usize>();
        const POOL0_SIZE: usize = core::mem::size_of::<usize>() * 2;
        let mut pool0_storage: [u8; POOL0_SIZE] = [0; POOL0_SIZE];
        let mut pool0: MemoryPool = MemoryPool::new(&mut pool0_storage, POOL0_SLOT_SIZE);
        unsafe{
            struct Struct0{
                a: usize
            }

                let res0 =  pool0.try_allocate_slot(core::mem::size_of::<Struct0>());
                let res0 = res0.unwrap();
                let struct0_0 : &mut Struct0 = core::mem::transmute(res0);
                *struct0_0 = Struct0{a: core::usize::MAX};
                
                let res1 =  pool0.try_allocate_slot(core::mem::size_of::<Struct0>());
                let res1 = res1.unwrap();
                let struct0_1 : &mut Struct0 = core::mem::transmute(res1);
                *struct0_1 = Struct0{a: core::usize::MIN};

                let res2 =  pool0.try_allocate_slot(core::mem::size_of::<Struct0>());
                assert_eq!(res2, Err(SlotAllocError::PoolFull));
                
                assert_eq!(struct0_0.a, core::usize::MAX);
                assert_eq!(struct0_1.a, core::usize::MIN);

                let res1 = pool0.try_free_slot(res1);
                assert_eq!(res1, Ok(()));
                assert_eq!(struct0_0.a, core::usize::MAX);
                
                let res3 = res0.sub(POOL0_SLOT_SIZE);
                let res3 = pool0.try_free_slot(res3);
                assert_eq!(res3, Err(SlotFreeingError::SlotOutOfRange));

                let res4 = res0.add(1);
                let res4 = pool0.try_free_slot(res4);
                assert_eq!(res4, Err(SlotFreeingError::UnalignedSlot));

                let res2 =  pool0.try_allocate_slot(2* core::mem::size_of::<Struct0>());
                assert_eq!(res2, Err(SlotAllocError::SlotNotLargeEnough));
                
                let res4 =  pool0.try_allocate_slot(core::mem::size_of::<Struct0>());
                let res4 = res4.unwrap();
                let struct0_4 : &mut Struct0 = core::mem::transmute(res4);
                *struct0_4 = Struct0{a: 0xAAAAAAAAAAAAAAAA};

                assert_eq!(struct0_0.a, core::usize::MAX);

                let res0 = pool0.try_free_slot(res0);
                assert_eq!(res0, Ok(()));

                assert_eq!(struct0_4.a, 0xAAAAAAAAAAAAAAAA);
                let res4 = pool0.try_free_slot(res4);
                res4.unwrap();
        }
    }
}

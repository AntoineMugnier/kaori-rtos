use core::result::Result;

#[derive(Debug, PartialEq, Eq)]
pub enum SlotAllocError {
    PoolFull,
    SlotNotLargeEnough,
}
type SlotAllocResult = Result<*mut usize, SlotAllocError>;

#[derive(Debug, PartialEq, Eq)]
pub enum SlotFreeingError{
    SlotOutOfRange,
    UnalignedSlot
}

type SlotFreeingResult = Result<(), SlotFreeingError>;

pub(crate) struct StaticMemoryPool<const WORDS_PER_POOL: usize> {
    pub(crate) slot_pool: [usize; WORDS_PER_POOL],
    pub(crate) words_per_slot: usize
}

const NEXT_SLOT_NONE: usize = core::usize::MAX;

impl <const WORDS_PER_POOL: usize> StaticMemoryPool<WORDS_PER_POOL>{

    const unsafe fn init_pool_slots(slot_pool: &mut [usize], words_per_slot: usize, idx: usize){
        match slot_pool{
            [] => {},
            [head, rest @..] =>{
                if idx % words_per_slot == 0
                {
                    if rest.len() == words_per_slot - 1 {
                        let empty_slot: &mut EmptySlot = core::mem::transmute(head);
                        *empty_slot = EmptySlot { next_slot_offset: NEXT_SLOT_NONE};
                    }
                    else{
                        let empty_slot: &mut EmptySlot = core::mem::transmute(head);
                        *empty_slot = EmptySlot {next_slot_offset: idx + words_per_slot};
                    }
                }
                Self::init_pool_slots(rest, words_per_slot, idx + 1);
            }
        }
    }

    pub const fn new(words_per_slot: usize) -> StaticMemoryPool<WORDS_PER_POOL>{
        assert!(words_per_slot > 0, "Slot size cannot be null");
        assert!(WORDS_PER_POOL > 0, "Slot pool length cannot be null");
        assert!(WORDS_PER_POOL % words_per_slot == 0, "Slot pool length must be a multiple of slot size");
        unsafe{
            let mut slot_pool : [usize; WORDS_PER_POOL] = [0; WORDS_PER_POOL];
            Self::init_pool_slots(&mut slot_pool, words_per_slot, 0);
            StaticMemoryPool { slot_pool, words_per_slot}
        }
    }

    pub const fn get_nb_slots(&self) -> usize{
       self.slot_pool.len() / self.words_per_slot 
    }

    pub const fn as_raw(&mut self) -> &mut [usize]{
        &mut self.slot_pool
    }


}

struct EmptySlot {
    next_slot_offset: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct SlotLinkedList{
    head_idx: usize,
    tail_idx: usize,
}

pub(crate) struct MemoryPool<'a> {
    pub(crate) slot_pool: &'a mut [usize],
    pub(crate) words_per_slot: usize,
    pub(crate) free_slot_linked_list : Option<SlotLinkedList>
}

impl<'a> MemoryPool<'a> {

    pub const fn new<const WORDS_PER_POOL: usize>(slot_pool: &'a mut StaticMemoryPool<WORDS_PER_POOL>) -> MemoryPool<'a> {
        let words_per_slot = slot_pool.words_per_slot;

        let free_slots_head_idx = 0;
        let free_slots_tail_idx = slot_pool.get_nb_slots() -1;
        let free_slot_linked_list = SlotLinkedList{head_idx: free_slots_head_idx, tail_idx: free_slots_tail_idx};

        MemoryPool {
            slot_pool: slot_pool.as_raw(),
            words_per_slot: words_per_slot,
            free_slot_linked_list: Some(free_slot_linked_list)
        }
    }
    
    unsafe fn get_free_slot_address_by_idx(&mut self, slot_idx: usize) -> *mut usize{
        let slot_pool_range = self.slot_pool.as_ptr_range();
        let slot_pool_start = slot_pool_range.start.cast_mut();
        slot_pool_start.add(slot_idx*self.words_per_slot)
    }

    unsafe fn get_free_slot_by_address(&mut self, address: *mut usize) -> &mut EmptySlot{
            core::mem::transmute(address)
    }

    unsafe fn get_free_slot_idx_by_address(&mut self, address: *mut usize) -> usize{
            let slot_pool_range = self.slot_pool.as_ptr_range();
            let slot_pool_start = slot_pool_range.start.cast_mut();
            return (address as usize - slot_pool_start as usize)/(self.words_per_slot * core::mem::size_of::<usize>())
    }

    pub(crate) unsafe fn try_allocate_slot(&mut self, size: usize) -> SlotAllocResult {
        if size > (self.words_per_slot * core::mem::size_of::<usize>()) {
            return Err(SlotAllocError::SlotNotLargeEnough);
        }
        
        let free_slot_linked_list: SlotLinkedList;
        if let Some(free_slot_linked_list_) = self.free_slot_linked_list.as_mut(){
            free_slot_linked_list = *free_slot_linked_list_;
        }
        else{
            return Err(SlotAllocError::PoolFull)
        }

        let slot_list_head_idx = free_slot_linked_list.head_idx; 
        let slot_list_head = self.access_slot(free_slot_linked_list.head_idx);
        let new_free_slot_list_head_idx = slot_list_head.next_slot_offset;

        if new_free_slot_list_head_idx == NEXT_SLOT_NONE {
            self.free_slot_linked_list = None;
        }
        else{
            self.free_slot_linked_list = Some(SlotLinkedList{head_idx: new_free_slot_list_head_idx, tail_idx: free_slot_linked_list.tail_idx});
        }
        
        let slot_pool_range = self.slot_pool.as_ptr_range();
        let slot_pool_start = slot_pool_range.start as usize;
        let allocated_address = slot_pool_start + slot_list_head_idx * (self.words_per_slot * core::mem::size_of::<usize>());
        return Ok(allocated_address as *mut usize); 
    }
    
    const unsafe fn access_slot(&mut self, idx: usize) ->  &mut EmptySlot{
            core::mem::transmute(&mut self.slot_pool[idx*self.words_per_slot])
    }

    pub(crate) unsafe fn try_free_slot(&mut self, slot_address: *mut usize) -> SlotFreeingResult {
        let slot_pool_range = self.slot_pool.as_ptr_range();
        let slot_pool_start = slot_pool_range.start.cast_mut();
        let slot_pool_end = slot_pool_range.end.cast_mut();
        
        // Check that the slot address to free is in address range
        if slot_address < slot_pool_start || slot_address >= slot_pool_end{
            return Err(SlotFreeingError::SlotOutOfRange);
        }
        // Check that the slot address to free exists
        if (slot_address as usize - slot_pool_start as usize)  % (self.words_per_slot * core::mem::size_of::<usize>()) != 0{
            return Err(SlotFreeingError::UnalignedSlot)
        }

        let slot_idx = (slot_address as usize - slot_pool_start as usize)/(self.words_per_slot * core::mem::size_of::<usize>());
        
        //Populate new free slot
        let new_free_slot = self.access_slot(slot_idx);
        *new_free_slot = EmptySlot{next_slot_offset: NEXT_SLOT_NONE};

        let new_free_slots_tail_idx = slot_idx;

        let free_slot_linked_list: SlotLinkedList;
        if let Some(free_slot_linked_list_) = self.free_slot_linked_list.as_mut(){
            free_slot_linked_list = *free_slot_linked_list_;
        }
        else{
            self.free_slot_linked_list = Some(SlotLinkedList{head_idx:new_free_slots_tail_idx, tail_idx: new_free_slots_tail_idx});
            return Ok(());
        }

        let slot_list_tail = self.access_slot(free_slot_linked_list.tail_idx);
        slot_list_tail.next_slot_offset = new_free_slots_tail_idx;
        self.free_slot_linked_list.unwrap().tail_idx = new_free_slots_tail_idx;
        return Ok(())

    }

    pub fn get_slot_size(&self) -> usize {
        self.words_per_slot * core::mem::size_of::<usize>()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test]

    fn mem_pool_test_0() {
        const POOL0_WORDS_PER_SLOT: usize = 1;
        const POOL0_SLOTS_PER_POOL: usize = 2;
        let mut static_memory_pool = StaticMemoryPool::<POOL0_SLOTS_PER_POOL>::new(POOL0_WORDS_PER_SLOT);
        let mut pool0: MemoryPool = MemoryPool::new(&mut static_memory_pool);
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
                
                let res3 = res0.sub(POOL0_WORDS_PER_SLOT);
                let res3 = pool0.try_free_slot(res3);
                assert_eq!(res3, Err(SlotFreeingError::SlotOutOfRange));

                let res4 = (res0 as *mut u8).add(1) as *mut usize ;
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

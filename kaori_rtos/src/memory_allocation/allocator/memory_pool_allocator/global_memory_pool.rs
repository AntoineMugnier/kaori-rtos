use crate::sync::AsyncRefCell;
use super::memory_pool::{MemoryPool, StaticMemoryPool};
pub struct GlobalStaticPool<const WORDS_PER_POOL : usize>{
    inner_array: AsyncRefCell<StaticMemoryPool<WORDS_PER_POOL>>
}

impl <const WORDS_PER_POOL : usize>GlobalStaticPool<WORDS_PER_POOL>{
    pub const fn new(words_per_slot: usize) -> GlobalStaticPool<WORDS_PER_POOL>{
        return GlobalStaticPool{inner_array: AsyncRefCell::new(StaticMemoryPool::<WORDS_PER_POOL>::new(words_per_slot))}
    }

    pub const fn get(&self) -> &mut StaticMemoryPool<WORDS_PER_POOL> {
        self.inner_array.borrow_mut()
    }

    pub const fn get_slot_size() -> usize{
            return WORDS_PER_POOL;
    }
}

pub struct GlobalStaticPoolRef<'a>{
    inner_array: &'a mut[u8]
}
pub struct MemoryPoolArray<'a, const SIZE: usize>{
    innner_array: [&'a mut MemoryPool<'a>; SIZE]
}

pub struct GlobalMemoryPoolArray<'a, const SIZE : usize>{
    inner_array: AsyncRefCell<MemoryPoolArray<'a, SIZE>>
}

impl <'a,  const SIZE : usize> GlobalMemoryPoolArray<'a, SIZE>{

    pub const fn as_ref(&'a self) -> GlobalMemoryPoolArrayRef<'a>{
            GlobalMemoryPoolArrayRef { inner_array: &mut self.inner_array.borrow_mut().innner_array }
    }
     
    const unsafe fn init_pool_slots(input_array: &[&'a GlobalMemoryPool<'a>], output_array: &mut [core::mem::MaybeUninit<&mut MemoryPool<'a>>]){
        match input_array{
            [] => {},
            [head, rest @..] =>{
                match output_array{
                    [] => {},
                    [o_head, o_rest @..] =>{
                        let o_hear_ptr = o_head.as_mut_ptr();
                        *o_hear_ptr = head.inner_memory_pool.borrow_mut().as_mut().unwrap();
                        Self::init_pool_slots(rest, o_rest);
                    }
                }
            }
        }
    }
        
    pub const fn new(memory_pool_array: [&'a GlobalMemoryPool<'a>;SIZE]) -> GlobalMemoryPoolArray<'a, SIZE>{
        unsafe{
        
        let mut raw_array: [core::mem::MaybeUninit::<&mut MemoryPool> ;SIZE]  = [ const{core::mem::MaybeUninit::<&mut MemoryPool>::uninit()}; SIZE];
         Self::init_pool_slots(&memory_pool_array, &mut raw_array);
        let end_array = MemoryPoolArray{innner_array: core::mem::transmute_copy(&raw_array)}; 
        GlobalMemoryPoolArray{inner_array: AsyncRefCell::new(end_array)}
        }
    }
}

pub struct GlobalMemoryPoolArrayRef<'a>{
    pub(crate) inner_array: &'a mut [&'a mut MemoryPool<'a>]
}


pub struct GlobalMemoryPool<'a>{
    inner_memory_pool: AsyncRefCell<Option<MemoryPool<'a>>>
}

impl <'a>GlobalMemoryPool<'a>{

    pub const fn new<const WORDS_PER_POOL: usize>(static_memory_pool: &'a mut StaticMemoryPool::<WORDS_PER_POOL>) -> GlobalMemoryPool<'a>{
        let memory_pool = MemoryPool::new(static_memory_pool);
        return GlobalMemoryPool { inner_memory_pool: AsyncRefCell::new(Some(memory_pool))};
        }

    pub fn get(&self) -> &mut MemoryPool<'a>{
        self.inner_memory_pool.borrow_mut().as_mut().unwrap()
    }
}

use crate::sync::AsyncRefCell;
use super::memory_pool::MemoryPool;
pub struct GlobalStaticPool<const SIZE : usize>{
    inner_array: AsyncRefCell<[u8; SIZE]>
}

impl <const SIZE : usize>GlobalStaticPool<SIZE>{
    pub const fn new() -> GlobalStaticPool<SIZE>{
        return GlobalStaticPool{inner_array: AsyncRefCell::new([0; SIZE])}
    }

    pub fn get(&self) -> &mut [u8] {
        self.inner_array.borrow_mut()
    }

    pub fn as_ref(&self) -> GlobalStaticPoolRef{
        GlobalStaticPoolRef{inner_array: self.inner_array.borrow_mut()}
    }
    pub const fn get_slot_size() -> usize{
            return SIZE;
    }
}

pub struct GlobalStaticPoolRef<'a>{
    inner_array: &'a mut[u8]
}
pub struct MemoryPoolArray<'a, const SIZE: usize>{
    innner_array: [&'a mut MemoryPool<'a>; SIZE]
}

pub struct GlobalMemoryPoolArray<'a, const SIZE : usize>{
    inner_array: AsyncRefCell<Option<MemoryPoolArray<'a, SIZE>>>
}

impl <'a,  const SIZE : usize> GlobalMemoryPoolArray<'a, SIZE>{
    pub const fn default() -> GlobalMemoryPoolArray<'a, SIZE>{
        GlobalMemoryPoolArray{inner_array: AsyncRefCell::new(None)}
    }

    pub fn as_ref(&'a self) -> GlobalMemoryPoolArrayRef{
            GlobalMemoryPoolArrayRef { inner_array: &mut self.inner_array.borrow_mut().as_mut().unwrap().innner_array }
    }

    pub fn set(&self, memory_pool_array: [&'a GlobalMemoryPool<'a>;SIZE]){
        let new_memory_pool_array = core::array::from_fn(|i| memory_pool_array[i].inner_memory_pool.borrow_mut().as_mut().unwrap());
        *self.inner_array.borrow_mut() = Some(MemoryPoolArray{innner_array: new_memory_pool_array}); 
    }
}

pub struct GlobalMemoryPoolArrayRef<'a>{
    pub(crate) inner_array: &'a mut [&'a mut MemoryPool<'a>]
}


pub struct GlobalMemoryPool<'a>{
    inner_memory_pool: AsyncRefCell<Option<MemoryPool<'a>>>
}

impl <'a>GlobalMemoryPool<'a>{
    pub const fn default() -> GlobalMemoryPool<'a>{
        return GlobalMemoryPool { inner_memory_pool: AsyncRefCell::new(None)};
    }
    pub fn new(global_static_pool_ref: GlobalStaticPoolRef<'a>, slot_size: usize) -> GlobalMemoryPool<'a>{
         let memory_pool = MemoryPool::new(global_static_pool_ref.inner_array, slot_size);
        return GlobalMemoryPool { inner_memory_pool: AsyncRefCell::new(Some(memory_pool))};
        }

    pub fn set(&self, global_static_pool_ref: GlobalStaticPoolRef<'a>, slot_size: usize){
         let memory_pool = MemoryPool::new(global_static_pool_ref.inner_array, slot_size);
        self.inner_memory_pool.borrow_mut().insert(memory_pool);
    }
    pub fn get(&self) -> &mut MemoryPool<'a>{
        self.inner_memory_pool.borrow_mut().as_mut().unwrap()
    }
}

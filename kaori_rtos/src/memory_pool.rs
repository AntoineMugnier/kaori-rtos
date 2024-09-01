
pub(crate) struct MemoryPool<'a>{
    pool : &'a mut [u8]
}

pub(crate) struct MemoryManager<'a>{
    memory_pools : &'a mut[&'a mut MemoryPool<'a>]
}

pub(crate) trait Allocable{

}
pub struct Slot{
    ptr: *const u8
}


impl <'a>MemoryManager<'a>{

    pub fn allocate<Element_T>(element: Element_T) -> Slot
    where Element_T: Allocable{
        let size = core::mem::size_of::<Element_T>();
        unimplemented!()

    }

    pub fn free(element: impl Allocable){

    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        let mut a: [u8;3] = [5,6,7];
        let mut b: [u8;1] = [6];
        let mut ma: MemoryPool = MemoryPool{pool: &mut a};
        let mut mb: MemoryPool = MemoryPool{pool: &mut b};
        let mut m : [&mut MemoryPool;2] = [&mut ma,&mut mb];
        let mm = MemoryManager{memory_pools: &mut m};
        mm.memory_pools[0].pool[0] = 3;
    }
}

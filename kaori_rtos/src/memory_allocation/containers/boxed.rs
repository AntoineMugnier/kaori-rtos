
 #[macro_export]
 macro_rules! define_box {
     ($box_type:ident, $allocator_instance: ident) => {

         impl <T>$box_type<T>{
             pub fn new(element: T) -> $box_type<T>{
                 unsafe{
                    let slot_pointer = $allocator_instance.allocate(core::alloc::Layout::new::<T>()).unwrap();
                    let allocated_mem: &mut T = $allocator_instance.get_slot_transmute(&slot_pointer).unwrap();
                     *allocated_mem = element;
                     $box_type {inner: slot_pointer, marker: core::marker::PhantomData::default()}
                 }
             }
         }

         impl<T> Drop for $box_type<T>{
             fn drop(&mut self){
                 unsafe{            
                    let allocated_mem: &mut T = $allocator_instance.get_slot_transmute(&self.inner).unwrap();
                     core::ptr::drop_in_place(allocated_mem);
                     $allocator_instance.free(self.inner).unwrap();
                 }
             }
         }

         impl <T>core::ops::Deref for $box_type<T>{
             type Target = T;
             fn deref(&self) -> &Self::Target {
                 unsafe{
                     core::mem::transmute(self.inner)
                 }
             }
         }

         struct $box_type<T>{
             inner: SlotPointer,
             marker: core::marker::PhantomData<T>,
         }

         unsafe impl <T> Send for $box_type<T>{
            
         }
     };
 }


 #[cfg(test)]
 mod tests {
     use std::{thread, sync::mpsc};
     use crate::memory_allocation::allocator::memory_pool_allocator::{SlotPointer, MemPoolId, SlotPool, MemoryPool};

        const POOL0_ID: MemPoolId = 0;
        const POOL0_WORDS_PER_SLOT: usize = 8;
        const POOL0_SLOT_PER_POOL: usize = 30;
        const POOL0_WORDS_PER_POOL: usize = POOL0_SLOT_PER_POOL * POOL0_WORDS_PER_SLOT;
        static STATIC_MEMORY_POOL: SlotPool<POOL0_WORDS_PER_POOL> =
            SlotPool::<POOL0_WORDS_PER_POOL>::new(POOL0_WORDS_PER_SLOT, POOL0_ID);
        static MEMORY_POOL_0: MemoryPool = MemoryPool::from(&STATIC_MEMORY_POOL);

     define_box!(EventBox, MEMORY_POOL_0);

     use super::*;
     struct A{
         a: u8
     }
     struct B{
         b0: usize,
         b1: usize
     }
     struct C{
         c: u32
     }
     
     
     const POOL0_SLOT_SIZE: usize = std::mem::size_of::<usize>();
     const POOL0_SIZE: usize = 2 * POOL0_SLOT_SIZE;

     enum UserEvent{
         A(EventBox<A>),
         B(EventBox<B>),
         C(EventBox<C>)
     }
     
     static mut POOL0: [u8; POOL0_SIZE] = [0; POOL0_SIZE];


     #[test]
     fn evt_box_test_0() {
         let a = A{a:10};
         let evt_primitive_a = EventBox::new(a);
         let e = UserEvent::A(evt_primitive_a);
         let evt_box = EventBox::new(e);
         
         let (tx, rx) = mpsc::channel();
         tx.send(evt_box);

         thread::spawn(move || {
             let evt_box = rx.recv().unwrap();
         });
     }
 }

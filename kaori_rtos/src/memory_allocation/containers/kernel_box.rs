use core::{mem::size_of, ops::Deref};
// use crate::memory_allocation::global_allocator::GlobalAllocator;


// #[macro_export]
// macro_rules! define_mem_box {
//     ($box_type:ident, $allocator_instance: ident) => {

//         impl <T>$box_type<T>{
//             pub(crate) fn new(element: T) -> $box_type<T>{
//                 unsafe{
//                     let inner = $allocator_instance.allocate(size_of::<T>()).unwrap();
//                     let inner = inner as *mut T;
//                     *inner = element;
//                     $box_type {inner}
//                 }
//             }
//         }

//         impl<T> Drop for $box_type<T>{
//             fn drop(&mut self){
//                 unsafe{            
//                     core::ptr::drop_in_place(self.inner);
//                     $allocator_instance.free(self.inner as *mut u8).unwrap();
//                 }
//             }
//         }

//         impl <T>Deref for $box_type<T>{
//             type Target = T;
//             fn deref(&self) -> &Self::Target {
//                 unsafe{
//                     core::mem::transmute(self.inner)
//                 }
//             }
//         }

//         struct $box_type<T>{
//             inner: *mut T
//         }

//         unsafe impl <T> Send for $box_type<T>{

//         }
//     };
// }


// // #[cfg(test)]
// // mod tests {
// //     use std::{thread, sync::mpsc};
// //     use crate::memory_allocation;

// //     static EVENT_ALLOCATOR: GlobalAllocator = GlobalAllocator::new();
// //     define_mem_box!(EventBox, EVENT_ALLOCATOR);

// //     use super::*;
// //     struct A{
// //         a: u8
// //     }
// //     struct B{
// //         b0: usize,
// //         b1: usize
// //     }
// //     struct C{
// //         c: u32
// //     }
// //     
// //     
// //     const POOL0_SLOT_SIZE: usize = std::mem::size_of::<usize>();
// //     const POOL0_SIZE: usize = 2 * POOL0_SLOT_SIZE;

// //     enum UserEvent{
// //         A(EventBox<A>),
// //         B(EventBox<B>),
// //         C(EventBox<C>)
// //     }
// //     
// //     static mut POOL0: [u8; POOL0_SIZE] = [0; POOL0_SIZE];


// //     #[test]
// //     fn evt_box_test_0() {
// //         let a = A{a:10};
// //         let evt_primitive_a = EventBox::new(a);
// //         let e = UserEvent::A(evt_primitive_a);
// //         let evt_box = EventBox::new(e);
// //         
// //         let (tx, rx) = mpsc::channel();
// //         tx.send(evt_box);

// //         thread::spawn(move || {
// //             let evt_box = rx.recv().unwrap();
// //         });
// //     }
// // }

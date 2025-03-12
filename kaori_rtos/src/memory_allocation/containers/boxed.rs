#[macro_export]
macro_rules! define_box {
    ($box_type:ident, $allocator_instance: ident) => {
        impl<T: std::fmt::Debug> $box_type<T> {
            pub fn new(element: T) -> $box_type<T> {
                unsafe {
                    let slot_pointer = $allocator_instance
                        .allocate(core::alloc::Layout::new::<T>())
                        .unwrap();
                    let allocated_mem = $allocator_instance
                        .get_slot_transmute(&slot_pointer)
                        .unwrap();

                    allocated_mem.write(element);

                    // println!("s: {:?}", a);
                    println!("New idx: {}", slot_pointer.get_index().unwrap());
                    $box_type {
                        inner: slot_pointer,
                        marker: core::marker::PhantomData::default(),
                    }
                }
            }
        }

        impl<T> Drop for $box_type<T> {
            fn drop(&mut self) {
                unsafe {
                    println!("Drop idx: {}", self.inner.get_index().unwrap());
                    let allocated_mem: &mut MaybeUninit<T> =
                        $allocator_instance.get_slot_transmute(&self.inner).unwrap();
                    allocated_mem.assume_init_drop();
                    $allocator_instance.free(self.inner).unwrap();
                }
            }
        }

        impl<T> core::ops::Deref for $box_type<T> {
            type Target = T;
            fn deref(&self) -> &Self::Target {
                unsafe {
                    println!("Deref idx: {}", self.inner.get_index().unwrap());
                    let allocated_mem =
                        $allocator_instance.get_slot_transmute(&self.inner).unwrap();
                    allocated_mem.assume_init_ref()
                }
            }
        }

        impl<T> core::ops::DerefMut for $box_type<T> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe {
                    let allocated_mem =
                        $allocator_instance.get_slot_transmute(&self.inner).unwrap();
                    allocated_mem.assume_init_mut()
                }
            }
        }

        impl <T> AsMut<T> for $box_type<T> {
            fn as_mut(&mut self) -> &mut T {
                <Self as core::ops::DerefMut>::deref_mut(self)    
            }
        }

        impl <T> AsRef<T> for $box_type<T> {
            fn as_ref(&self) -> &T {
                <Self as core::ops::Deref>::deref(self)    
            }
        }

        #[derive(Debug)]
        struct $box_type<T> {
            inner: SlotPointer,
            marker: core::marker::PhantomData<T>,
        }


        unsafe impl<T> Send for $box_type<T> {}
    };
}
#[cfg(test)]
mod tests {
    use crate::memory_allocation::allocator::memory_pool_allocator::{
        MemPoolId, MemoryPool, SlotPointer, SlotPool,
    };

    use core::mem::MaybeUninit;
    use std::{sync::mpsc, thread};
    const POOL0_ID: MemPoolId = 0;
    const POOL0_WORDS_PER_SLOT: usize = 8;
    const POOL0_SLOT_PER_POOL: usize = 30;
    const POOL0_WORDS_PER_POOL: usize = POOL0_SLOT_PER_POOL * POOL0_WORDS_PER_SLOT;
    static STATIC_MEMORY_POOL: SlotPool<POOL0_WORDS_PER_POOL> =
        SlotPool::<POOL0_WORDS_PER_POOL>::new(POOL0_WORDS_PER_SLOT, POOL0_ID);
    static MEMORY_POOL_0: MemoryPool = MemoryPool::from(&STATIC_MEMORY_POOL);

    define_box!(EventBox, MEMORY_POOL_0);

    #[derive(Debug)]
    struct A {
        pub a: u8,
    }

    #[derive(Debug)]
    struct B {
        pub b0: usize,
        pub b1: usize,
    }

    #[derive(Debug)]
    struct C {
        pub c0: usize,
        pub c1: u16,
    }

    #[derive(Debug)]
    enum UserEvent {
        A(EventBox<A>),
        B(EventBox<B>),
        C(EventBox<C>),
    }
    const A_VAL: u8 = 10;
    const B0_VAL: usize = 0xFEE0ACB8CDA24E2C;
    const B1_VAL: usize = 0x89FAE3DABAE8BE0C;
    const C0_VAL: usize = 0x11ACCDF33458BC93;
    const C1_VAL: u16 = 0x652E;

    fn handle_event(evt_box: EventBox<UserEvent>) {
        println!("B");
        let i = evt_box.as_ref();
        match i {
            UserEvent::A(a) => {
                assert_eq!(a.a, A_VAL);
            }
            UserEvent::B(b) =>{
                assert_eq!(b.b0, B0_VAL);
                assert_eq!(b.b1, B1_VAL);
            }
            UserEvent::C(c) =>{
                assert_eq!(c.c0, C0_VAL);
                assert_eq!(c.c1, C1_VAL);
            }
        }
    }

    #[test]
    fn evt_box_test_0() {
        let evt_a = EventBox::new(A { a: A_VAL });
        let evt_a = UserEvent::A(evt_a);
        let evt_a = EventBox::new(evt_a);

        let evt_b = EventBox::new(B { b0: B0_VAL, b1: B1_VAL});
        let evt_b = UserEvent::B(evt_b);
        let evt_b = EventBox::new(evt_b);
 
        let evt_c = EventBox::new(C { c0: C0_VAL, c1: C1_VAL});
        let evt_c = UserEvent::C(evt_c);
        let evt_c = EventBox::new(evt_c);

        let (tx, rx) = mpsc::channel();
        tx.send(evt_a).unwrap();
        tx.send(evt_b).unwrap();
        tx.send(evt_c).unwrap();

        thread::spawn(move || {
            for _ in 0..3{
                let evt = rx.recv().unwrap();
                println!("{:?}", *evt);
                handle_event(evt);
            }
        })
        .join()
        .unwrap();
    }
}

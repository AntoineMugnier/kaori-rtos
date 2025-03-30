#[macro_export]
macro_rules! define_arc {
    ($arc_mod:ident, $box_mod:ident) => {
        mod $arc_mod {

            use portable_atomic as atomic;

            impl<T: std::fmt::Debug> Arc<T> {
                pub fn new(element: T) -> Arc<T> {
                    let inner_arc = InnerArc {
                        inner: element,
                        counter: atomic::AtomicUsize::new(1),
                    };
                    let boxed_inner_arc = super::$box_mod::Box::new(inner_arc);

                    Arc {
                        inner: core::mem::ManuallyDrop::new(boxed_inner_arc),
                        marker: core::marker::PhantomData::<T>,
                    }
                }
            }
            
            impl<T> Drop for Arc<T> {
                fn drop(&mut self) {
                    unsafe {
                        if self.inner.counter.fetch_sub(1, atomic::Ordering::Release) != 1 {
                            return;
                        }
                        let _ = self.inner.counter.load(atomic::Ordering::Acquire);
                        core::mem::ManuallyDrop::drop(&mut self.inner)
                    }
                }
            }

            impl<T> core::ops::Deref for Arc<T> {
                type Target = T;
                fn deref(&self) -> &Self::Target {
                    let inner_box = self.inner.deref();
                    let inner_arc = inner_box.deref();
                    &inner_arc.inner
                }
            }

            impl<T> core::ops::DerefMut for Arc<T> {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    let inner_box = self.inner.deref_mut();
                    let inner_arc = inner_box.deref_mut();
                    &mut inner_arc.inner
                }
            }

            impl<T> AsMut<T> for Arc<T> {
                fn as_mut(&mut self) -> &mut T {
                    <Self as core::ops::DerefMut>::deref_mut(self)
                }
            }

            impl<T> AsRef<T> for Arc<T> {
                fn as_ref(&self) -> &T {
                    <Self as core::ops::Deref>::deref(self)
                }
            }

            #[derive(Debug)]
            struct InnerArc<T> {
                inner: T,
                counter: atomic::AtomicUsize,
            }

            #[derive(Debug)]
            pub struct Arc<T> {
                inner: core::mem::ManuallyDrop<super::$box_mod::Box::<InnerArc<T>>>,
                marker: core::marker::PhantomData<T>,
            }

            impl <T>Clone for Arc<T>{
                fn clone(&self) -> Self{
                    self.inner.counter.fetch_add(1, atomic::Ordering::Relaxed);
                    unsafe{
                    Arc {
                        inner: core::mem::ManuallyDrop::new(self.inner.leak()),
                        marker: core::marker::PhantomData::<T>,
                    }
                    }
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::memory_allocation::allocator::memory_pool_allocator::{
        MemPoolId, MemoryPool, SlotPool,
    };
    use std::thread;
    const POOL0_ID: MemPoolId = 0;
    const POOL0_WORDS_PER_SLOT: usize = 8;
    const POOL0_SLOT_PER_POOL: usize = 30;
    const POOL0_WORDS_PER_POOL: usize = POOL0_SLOT_PER_POOL * POOL0_WORDS_PER_SLOT;
    static STATIC_MEMORY_POOL: SlotPool<POOL0_WORDS_PER_POOL> =
        SlotPool::<POOL0_WORDS_PER_POOL>::new(POOL0_WORDS_PER_SLOT, POOL0_ID);
    static MEMORY_POOL_0: MemoryPool = MemoryPool::from(&STATIC_MEMORY_POOL);
    define_box!(test_box, MEMORY_POOL_0);
    define_arc!(test_arc, test_box);

    #[derive(Debug)]
    struct A (u16, usize);

    #[derive(Debug)]
    struct B (test_arc::Arc<A>);

    const A0_VAL: u16 = 0xFEE0;
    const A1_VAL: usize = 0x11ACCDF33458BC93;

    #[test]
    fn arc_test_0() {
        let a = A (A0_VAL, A1_VAL);
        let arc_a = test_arc::Arc::new(a);
        let b = B(arc_a);
        let arc_b = test_arc::Arc::new(b);

        for _ in 0..100{
            let arc_b_clone = arc_b.clone();
            thread::spawn( move ||{
                assert_eq!(arc_b_clone.0.0, A0_VAL);
                assert_eq!(arc_b_clone.0.1, A1_VAL);
            });
        }
        
        assert_eq!(arc_b.0.0, A0_VAL);
        assert_eq!(arc_b.0.1, A1_VAL);
    }
}

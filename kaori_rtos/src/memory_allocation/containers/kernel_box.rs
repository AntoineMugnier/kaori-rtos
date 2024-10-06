use core::{mem::size_of, ops::Deref};

#[macro_export]
macro_rules! define_shared_allocator {
    ($allocator_instance_name:ident, $container_name: ident) => {
    
    struct $container_name<T>{
        inner: *mut T
    }

    unsafe impl <T> Send for $container_name<T>{

    }
        impl <T>$container_name<T>{
            pub(crate) fn new(element: T) -> $container_name<T>{
                unsafe{
                    let inner = $allocator_instance_name.allocate(size_of::<T>()).unwrap();
                    let inner = inner as *mut T;
                    *inner = element;
                    $container_name {inner}
                }
            }
        }

        impl<T> Drop for $container_name<T>{
            fn drop(&mut self){
                unsafe{            
                    core::ptr::drop_in_place(self.inner);
                    $allocator_instance_name.free(self.inner as *mut u8).unwrap();
                }
            }
        }

        impl <T>core::ops::Deref for $container_name<T>{
            type Target = T;
            fn deref(&self) -> &Self::Target {
                unsafe{
                    core::mem::transmute(self.inner)
                }
            }
        }
    }
}


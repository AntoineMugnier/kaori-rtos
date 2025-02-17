use crate::sync::{AsyncArrayCell, AsyncArrayCellRef};
use core::result::Result;
use portable_atomic as atomic;
struct AtomicSlotPointer {
    inner: atomic::AtomicUsize,
}

//TODO SLOT POINTER should not be copiable
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct SlotPointer {
    inner: usize,
}

#[cfg(target_pointer_width = "32")]
pub mod types {
    pub type SlotIndex = u16;
    pub type SlotTag = u16;
    pub const MP_TAG_SH: usize = 16;
    pub const MP_TAG_MSK: usize = 0xff00;
    pub const MP_TAG_MAX_VALUE: u16 = core::u16::MAX;
    pub const MP_TAG_MIN_VALUE: u16 = 0;
    pub const MP_SLOT_IDX_SH: usize = 0;
    pub const MP_SLOT_IDX_MSK: usize = 0x00ff;
    pub const MP_SLOT_IDX_MAX_VAL: u16 = core::u16::MAX - 1;
    pub const MP_SLOT_IDX_MIN_VAL: u16 = 0;
    pub const MP_SLOT_IDX_NEXT_NONE: u16 = SlotIndex::MAX;
}

#[cfg(target_pointer_width = "64")]
pub mod types {
    pub type SlotIndex = u32;
    pub type MemPoolId = u8;
    pub type SlotTag = u32;
    pub const MP_ID_SH: usize = 56;
    pub const MP_ID_MSK: usize = 0xff00000000000000;
    pub const MP_TAG_SH: usize = 20;
    pub const MP_TAG_MSK: usize = 0x00fffffffff00000;
    pub const MP_TAG_MAX_VALUE: SlotTag = (MP_TAG_MSK >> MP_TAG_SH) as u32;
    pub const MP_TAG_MIN_VALUE: SlotTag = 0;
    pub const MP_SLOT_IDX_SH: usize = 0;
    pub const MP_SLOT_IDX_MSK: usize = 0x00000000000fffff;
    pub const MP_SLOT_IDX_MAX_VAL: SlotIndex = MP_SLOT_IDX_MSK as u32 - 1;
    pub const MP_SLOT_IDX_MIN_VAL: SlotIndex = 0;
    pub const MP_SLOT_IDX_NEXT_NONE: SlotIndex = MP_SLOT_IDX_MAX_VAL + 1;
}

use types::*;

impl AtomicSlotPointer {
    const fn new(pool_id: MemPoolId, index: Option<SlotIndex>) -> AtomicSlotPointer {
        let new_index;
        if let Some(index) = index {
            new_index = index;
        } else {
            new_index = MP_SLOT_IDX_NEXT_NONE;
        }
        let slot_index = ((new_index as usize) << MP_SLOT_IDX_SH) & MP_SLOT_IDX_MSK;
        let mem_pool_id = ((pool_id as usize) << MP_ID_SH) & MP_ID_MSK;
        let slot_pointer_val = slot_index | mem_pool_id;

        AtomicSlotPointer {
            inner: atomic::AtomicUsize::new(slot_pointer_val),
        }
    }

    const fn from(slot_pointer: SlotPointer) -> AtomicSlotPointer {
        AtomicSlotPointer {
            inner: atomic::AtomicUsize::new(slot_pointer.inner as usize),
        }
    }

    fn compare_exchange_weak(
        &self,
        current: SlotPointer,
        new: SlotPointer,
        success: atomic::Ordering,
        failure: atomic::Ordering,
    ) -> Result<usize, usize> {
        self.inner
            .compare_exchange_weak(current.inner, new.inner, success, failure)
    }

    fn load(&self, ordering: atomic::Ordering) -> SlotPointer {
        SlotPointer {
            inner: self.inner.load(ordering),
        }
    }

    fn store(&self, slot_pointer: SlotPointer, ordering: atomic::Ordering) {
        self.inner.store(slot_pointer.inner, ordering);
    }
}
impl SlotPointer {
    const fn from(raw_slot_pointer: usize) -> SlotPointer {
        SlotPointer {
            inner: raw_slot_pointer,
        }
    }

    pub(crate) const fn get_mem_pool_id(&self) -> MemPoolId {
        ((self.inner & MP_ID_MSK) >> MP_ID_SH) as MemPoolId
    }

    fn increment_tag(&mut self) {
        let mem_pool_id = ((self.inner & MP_ID_MSK) >> MP_ID_SH) as MemPoolId;
        let mut tag = ((self.inner & MP_TAG_MSK) >> MP_TAG_SH) as SlotTag;
        let index = ((self.inner & MP_SLOT_IDX_MSK) >> MP_SLOT_IDX_SH) as SlotIndex;
        if tag == MP_TAG_MAX_VALUE {
            tag = MP_TAG_MIN_VALUE;
        } else {
            tag += 1;
        }

        self.inner = ((mem_pool_id as usize) << MP_ID_SH)
            | ((tag as usize) << MP_TAG_SH)
            | ((index as usize) << MP_SLOT_IDX_SH);
    }

    fn get_index_raw(&self) -> SlotIndex {
        ((self.inner >> MP_SLOT_IDX_SH) & MP_SLOT_IDX_MSK) as SlotIndex
    }

    fn get_index(&self) -> Option<SlotIndex> {
        let index = ((self.inner >> MP_SLOT_IDX_SH) & MP_SLOT_IDX_MSK) as SlotIndex;
        if index != MP_SLOT_IDX_NEXT_NONE {
            Some(index)
        } else {
            None
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SlotAllocError {
    PoolFull,
    SlotNotLargeEnough,
}
type SlotAllocResult = Result<SlotPointer, SlotAllocError>;

#[derive(Debug, PartialEq, Eq)]
pub enum SlotFreeingError {
    SlotOutOfRange,
}

type SlotFreeingResult = Result<(), SlotFreeingError>;

pub(crate) struct SlotPool<const WORDS_PER_POOL: usize> {
    pub(crate) sto: AsyncArrayCell<usize, WORDS_PER_POOL>,
    pub(crate) words_per_slot: usize,
    pub(crate) pool_id: MemPoolId,
}

const NEXT_SLOT_NONE: usize = core::usize::MAX;

impl<const WORDS_PER_POOL: usize> SlotPool<WORDS_PER_POOL> {
    const unsafe fn init_pool_slots(
        sto: &mut [usize],
        words_per_slot: usize,
        pool_id: MemPoolId,
        idx: usize,
    ) {
        match sto {
            [] => {}
            [head, rest @ ..] => {
                if idx % words_per_slot == 0 {
                    if rest.len() == words_per_slot - 1 {
                        let empty_slot: &mut EmptySlot = core::mem::transmute(head);
                        *empty_slot = EmptySlot {
                            next: AtomicSlotPointer::new(pool_id, None),
                        };
                    } else {
                        let empty_slot: &mut EmptySlot = core::mem::transmute(head);
                        let slot_index = (idx / words_per_slot + 1) as SlotIndex;
                        *empty_slot = EmptySlot {
                            next: AtomicSlotPointer::new(pool_id, Some(slot_index)),
                        };
                    }
                }
                Self::init_pool_slots(rest, words_per_slot, pool_id, idx + 1);
            }
        }
    }

    const fn create_head(&self) -> AtomicSlotPointer {
        AtomicSlotPointer::new(self.pool_id, Some(0))
    }

    pub const fn new(words_per_slot: usize, pool_id: MemPoolId) -> SlotPool<WORDS_PER_POOL> {
        assert!(words_per_slot > 0, "Slot size cannot be null");
        assert!(WORDS_PER_POOL > 0, "Slot pool length cannot be null");
        assert!(
            WORDS_PER_POOL % words_per_slot == 0,
            "Slot pool length must be a multiple of slot size"
        );
        assert!(
            (WORDS_PER_POOL / words_per_slot) <= (MP_SLOT_IDX_MAX_VAL + 1) as usize,
            "Too many slots in slot pool"
        );
        unsafe {
            let mut sto: [usize; WORDS_PER_POOL] = [0; WORDS_PER_POOL];
            Self::init_pool_slots(&mut sto, words_per_slot, pool_id, 0);
            SlotPool {
                sto: AsyncArrayCell::new(sto),
                words_per_slot,
                pool_id,
            }
        }
    }

    pub const fn get_slot_pool_ref(&self) -> AsyncArrayCellRef<usize> {
        self.sto.borrow_mut()
    }
}

struct EmptySlot {
    next: AtomicSlotPointer,
}

pub(crate) struct MemoryPool<'a> {
    id: MemPoolId,
    sto: AsyncArrayCellRef<'a, usize>,
    words_per_slot: usize,
    head: AtomicSlotPointer,
}

pub(crate) enum SlotAccessError {
    SlotOutOfRange,
    SlotNone,
}

impl<'a> MemoryPool<'a> {
    // pub(crate) const fn get_inner(&mut self) -> &mut [usize]{
    //     &mut self.sto
    // }

    pub const fn get_mem_pool_id(&self) -> MemPoolId {
        self.id
    }

    pub const fn from<const WORDS_PER_POOL: usize>(
        slot_pool: &SlotPool<WORDS_PER_POOL>,
    ) -> MemoryPool {
        MemoryPool {
            id: slot_pool.pool_id,
            sto: slot_pool.get_slot_pool_ref(),
            words_per_slot: slot_pool.words_per_slot,
            head: slot_pool.create_head(),
        }
    }
    pub(crate) const fn get_slot_size(&self) -> usize {
        self.words_per_slot * core::mem::size_of::<usize>()
    }

    fn get_nb_slot(&self) -> usize {
        // let sto = &*(*self.sto.get()) as &[usize];
        self.sto.len() / self.words_per_slot
    }

    pub(crate) fn get_slot_raw_mut(
        &self,
        slot_pointer: SlotPointer,
    ) -> Result<*mut u8, SlotAccessError> {
        let slot_index = slot_pointer.get_index_raw();
        if slot_index >= MP_SLOT_IDX_MIN_VAL && slot_index < self.get_nb_slot() as SlotIndex {
            unsafe {
                let sto = self.sto.deref_mut();
                let raw_ptr = sto
                    .as_ptr()
                    .add((slot_index as usize) * self.words_per_slot);
                Ok(core::mem::transmute(raw_ptr))
            }
        } else {
            if slot_index == MP_SLOT_IDX_NEXT_NONE {
                return Err(SlotAccessError::SlotNone);
            } else {
                return Err(SlotAccessError::SlotOutOfRange);
            }
        }
    }

    // Get a slot from the memory pool using a SlotPointer object
    pub(crate) unsafe fn get_slot_transmute<T>(
        &self,
        slot_pointer: SlotPointer,
    ) -> Result<&mut T, ()> {
        let slot_mem_ptr_res = self.get_slot_raw_mut(slot_pointer);
        if let Ok(slot_mem_ptr) = slot_mem_ptr_res {
            Ok((slot_mem_ptr as *mut T).as_mut().unwrap())
        } else {
            Err(())
        }
    }

    fn get_slot(&self, slot_pointer: SlotPointer) -> Result<*const EmptySlot, SlotAccessError> {
        self.get_slot_raw_mut(slot_pointer)
            .map(|x: *mut u8| x as *const EmptySlot)
    }

    fn get_slot_mut(&self, slot_pointer: SlotPointer) -> Result<*mut EmptySlot, SlotAccessError> {
        self.get_slot_raw_mut(slot_pointer)
            .map(|x: *mut u8| x as *mut EmptySlot)
    }

    pub unsafe fn try_free_slot(&self, slot_pointer: SlotPointer) -> SlotFreeingResult {
        println!("Freeing  slot {}", slot_pointer.get_index().unwrap());
        let new_head_slot = self
            .get_slot_mut(slot_pointer)
            .map_err(|_| SlotFreeingError::SlotOutOfRange)?;
        loop {
            let head = self.head.load(atomic::Ordering::Relaxed);
            println!("old_head {:?}", head.get_index());
            *new_head_slot = EmptySlot {
                next: AtomicSlotPointer::from(head),
            };
            if let Err(_) = self.head.compare_exchange_weak(
                head,
                slot_pointer,
                atomic::Ordering::Release,
                atomic::Ordering::Relaxed,
            ) {
                continue;
            } else {
                return Ok(());
            }
        }
    }

    pub fn try_allocate_slot(&self, layout: core::alloc::Layout) -> SlotAllocResult {
        if layout.size() > (self.get_slot_size()) {
            return Err(SlotAllocError::SlotNotLargeEnough);
        }
        loop {
            let mut head = self.head.load(atomic::Ordering::Acquire);

            println!("Allocating slot {}", head.get_index().unwrap());
            if let Ok(head_slot) = self.get_slot(head) {
                unsafe {
                    let head_next = &(*head_slot).next;
                    let new_head = head_next.load(atomic::Ordering::Relaxed);
                    println!("head_next {:?}", new_head.get_index());
                    if let Err(_) = self.head.compare_exchange_weak(
                        head,
                        new_head,
                        atomic::Ordering::Release,
                        atomic::Ordering::Relaxed,
                    ) {
                        continue;
                    }
                }
                head.increment_tag();
                return Ok(head);
            } else {
                return Err(SlotAllocError::PoolFull);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use core::marker::PhantomData;

    use rand::Rng;

    use crate::memory_allocation::allocator::memory_pool_allocator::MemoryAccessor;

    use super::super::super::Allocator;
    use super::*;

    mod basic_mem_pool_test {
        use super::*;
        const POOL0_ID: MemPoolId = 0;
        const POOL0_WORDS_PER_SLOT: usize = 1;
        const POOL0_WORDS_PER_POOL: usize = 2;
        static STATIC_MEMORY_POOL: SlotPool<POOL0_WORDS_PER_POOL> =
            SlotPool::<POOL0_WORDS_PER_POOL>::new(POOL0_WORDS_PER_SLOT, POOL0_ID);
        static MEMORY_POOL_0: MemoryPool = MemoryPool::from(&STATIC_MEMORY_POOL);

        #[test]
        fn mem_pool_test_0() {
            unsafe {
                struct Struct0 {
                    a: usize,
                }
                struct Struct1 {
                    _a: usize,
                    _b: usize,
                }

                let res0 = MEMORY_POOL_0.try_allocate_slot(core::alloc::Layout::new::<Struct0>());
                let res0 = res0.unwrap();
                let struct0_0: &mut Struct0 = MEMORY_POOL_0.get_slot_transmute(res0).unwrap();
                *struct0_0 = Struct0 {
                    a: core::usize::MAX,
                };

                let res1 = MEMORY_POOL_0.try_allocate_slot(core::alloc::Layout::new::<Struct0>());
                let res1 = res1.unwrap();
                let struct0_1: &mut Struct0 = MEMORY_POOL_0.get_slot_transmute(res1).unwrap();
                *struct0_1 = Struct0 {
                    a: core::usize::MIN,
                };

                let res2 = MEMORY_POOL_0.try_allocate_slot(core::alloc::Layout::new::<Struct0>());
                assert_eq!(res2, Err(SlotAllocError::PoolFull));

                assert_eq!(struct0_0.a, core::usize::MAX);
                assert_eq!(struct0_1.a, core::usize::MIN);

                MEMORY_POOL_0.try_free_slot(res1).unwrap();
                assert_eq!(struct0_0.a, core::usize::MAX);

                let res3 = SlotPointer {
                    inner: (res1.get_index_raw() + 1) as usize,
                };
                let res3 = MEMORY_POOL_0.try_free_slot(res3);
                assert_eq!(res3, Err(SlotFreeingError::SlotOutOfRange));

                let res2 = MEMORY_POOL_0.try_allocate_slot(core::alloc::Layout::new::<Struct1>());
                assert_eq!(res2, Err(SlotAllocError::SlotNotLargeEnough));

                let res4 = MEMORY_POOL_0.try_allocate_slot(core::alloc::Layout::new::<Struct0>());
                let res4 = res4.unwrap();
                let struct0_4: &mut Struct0 = MEMORY_POOL_0.get_slot_transmute(res4).unwrap();
                *struct0_4 = Struct0 {
                    a: 0xAAAAAAAAAAAAAAAA,
                };

                assert_eq!((*struct0_0).a, core::usize::MAX);

                let res0 = MEMORY_POOL_0.try_free_slot(res0);
                assert_eq!(res0, Ok(()));

                assert_eq!(struct0_4.a, 0xAAAAAAAAAAAAAAAA);
                let res4 = MEMORY_POOL_0.try_free_slot(res4);
                res4.unwrap();
            }
        }
    }
    impl<'a> Allocator<SlotPointer, SlotFreeingError, SlotAllocError> for MemoryPool<'a> {
        unsafe fn free(&self, slot_pointer: SlotPointer) -> Result<(), SlotFreeingError> {
            self.try_free_slot(slot_pointer)
        }

        fn allocate(&self, layout: core::alloc::Layout) -> Result<SlotPointer, SlotAllocError> {
            self.try_allocate_slot(layout)
        }
    }

    impl<'a> MemoryAccessor<SlotPointer> for MemoryPool<'a> {
        unsafe fn get_slot_transmute<T>(&self, slot_pointer: SlotPointer) -> Result<&mut T, ()> {
            self.get_slot_transmute(slot_pointer)
        }
        fn get_slot_mut(&self, slot_pointer: SlotPointer) -> Result<*mut u8, ()> {
            self.get_slot_raw_mut(slot_pointer).map_err(|_| ())
        }
    }

    struct Tester<
        'a,
        FreeErrorType: core::fmt::Debug,
        AllocationErrorType: core::fmt::Debug,
        AllocatorType,
    >
    where
        AllocatorType: Allocator<SlotPointer, FreeErrorType, AllocationErrorType>
            + MemoryAccessor<SlotPointer>,
    {
        allocator: &'a AllocatorType,
        reference_allocator: Vec<(Vec<u8>, SlotPointer)>,
        phantom_free_err: PhantomData<FreeErrorType>,
        phantom_alloc_err: PhantomData<AllocationErrorType>,
    }

    impl<
            'a,
            FreeErrorType: core::fmt::Debug,
            AllocationErrorType: core::fmt::Debug,
            AllocatorType: Allocator<SlotPointer, FreeErrorType, AllocationErrorType>
                + MemoryAccessor<SlotPointer>,
        > Tester<'a, FreeErrorType, AllocationErrorType, AllocatorType>
    {
        fn new(
            allocator: &'a AllocatorType,
        ) -> Tester<'a, FreeErrorType, AllocationErrorType, AllocatorType> {
            Self {
                allocator,
                reference_allocator: Vec::new(),
                phantom_free_err: PhantomData::default(),
                phantom_alloc_err: PhantomData::default(),
            }
        }

        unsafe fn allocate(&mut self, element: &mut [u8]) {
            let layout = core::alloc::Layout::array::<u8>(element.len()).unwrap();
            let pointer = self.allocator.allocate(layout).unwrap();

            let element_slot = self.allocator.get_slot_mut(pointer).unwrap();
            element_slot.copy_from_nonoverlapping(element.as_ptr(), element.len());
            self.reference_allocator.push((element.to_vec(), pointer));
        }

        fn free(&mut self, element_index: usize) {
            let (_, slot_pointer) = self.reference_allocator.remove(element_index);
            unsafe {
                self.allocator.free(slot_pointer).unwrap();
            }
        }

        fn run_integrity_check(&mut self) {
            for (reference_element, pointer) in self.reference_allocator.iter() {
                let element_slot = self.allocator.get_slot_mut(*pointer).unwrap();
                for i in 0..reference_element.len() {
                    unsafe {
                        let val = *element_slot.offset(i as isize);
                        assert!(val == reference_element[i]);
                    }
                }
            }
        }

        fn run(&mut self, tp: TestParams) {
            let mut rng = rand::rng();

            //Fill the pool
            println!("Initial filling of the pool");
            for _ in 0..tp.n_initial_elements {
                let generated_element_size = rng.random_range(..tp.max_element_size);
                let mut generated_element: Vec<_> = (&mut rng)
                    .random_iter::<u8>()
                    .take(generated_element_size)
                    .collect();
                unsafe {
                    self.allocate(generated_element.as_mut_slice());
                }
            }
            self.run_integrity_check();

            println!("Test loop");
            for _ in 0..tp.n_iterations {
                let n_elements_allocated = self.reference_allocator.len();
                let should_allocate;
                if n_elements_allocated == tp.max_n_elements {
                    should_allocate = false;
                } else if n_elements_allocated == 0 {
                    should_allocate = true;
                } else {
                    should_allocate = rng.random_bool(0.5);
                }

                if should_allocate {
                    let generated_element_size = rng.random_range(..tp.max_element_size);
                    let mut generated_element: Vec<_> = (&mut rng)
                        .random_iter::<u8>()
                        .take(generated_element_size)
                        .collect();
                    unsafe {
                        self.allocate(generated_element.as_mut_slice());
                    }
                } else {
                    let element_index = rng.random_range(..n_elements_allocated);
                    self.free(element_index)
                }
            }
            self.run_integrity_check();
        }
    }

    struct TestParams {
        max_n_elements: usize,
        max_element_size: usize,
        n_initial_elements: usize,
        n_iterations: usize,
    }

    mod single_thread_randomized {
        use super::*;
        const POOL0_ID: MemPoolId = 0;
        const POOL0_WORDS_PER_SLOT: usize = 8;
        const POOL0_SLOT_PER_POOL: usize = 15;
        const POOL0_WORDS_PER_POOL: usize = POOL0_SLOT_PER_POOL * POOL0_WORDS_PER_SLOT;
        static STATIC_MEMORY_POOL: SlotPool<POOL0_WORDS_PER_POOL> =
            SlotPool::<POOL0_WORDS_PER_POOL>::new(POOL0_WORDS_PER_SLOT, POOL0_ID);
        static MEMORY_POOL_0: MemoryPool = MemoryPool::from(&STATIC_MEMORY_POOL);

        #[test]
        fn single_thread_randomized() {
            let test_params = TestParams {
                max_n_elements: POOL0_SLOT_PER_POOL,
                max_element_size: POOL0_WORDS_PER_SLOT * core::mem::size_of::<usize>(),
                n_initial_elements: 10,
                n_iterations: 100,
            };

            let mut tester = Tester::new(&MEMORY_POOL_0);
            tester.run(test_params);
        }
    }
}

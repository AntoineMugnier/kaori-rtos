use crate::memory_allocation::allocator::Allocator;
use crate::memory_allocation::allocator::memory_pool_allocator::MemoryAccessor;
use crate::sync::{AsyncArrayCell, AsyncArrayCellRef};
use core::mem::MaybeUninit;
use core::result::Result;
use portable_atomic as atomic;
struct AtomicSlotPointer {
    inner: atomic::AtomicUsize,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SlotPointer {
    inner: usize,
}

#[cfg(target_pointer_width = "32")]
pub mod types {
    pub type SlotIndex = u16;
    pub type MemPoolId = u8;
    pub type SlotTag = u16;
    pub const MP_ID_SH: usize = 24;
    pub const MP_ID_MSK: usize = 0xFF000000;
    pub const MP_TAG_SH: usize = 12;
    pub const MP_TAG_MSK: usize = 0x00FFF000;
    pub const MP_TAG_MAX_VALUE: SlotTag = (MP_TAG_MSK >> MP_TAG_SH) as u32;
    pub const MP_TAG_MIN_VALUE: SlotTag = 0;
    pub const MP_SLOT_IDX_SH: usize = 0;
    pub const MP_SLOT_IDX_MSK: usize = 0x00000FFF;
    pub const MP_SLOT_IDX_MAX_VAL: SlotIndex = MP_SLOT_IDX_MSK as u32 - 1;
    pub const MP_SLOT_IDX_MIN_VAL: SlotIndex = 0;
    pub const MP_SLOT_IDX_NEXT_NONE: SlotIndex = MP_SLOT_IDX_MAX_VAL + 1;
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

    pub const fn get_mem_pool_id(&self) -> MemPoolId {
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

    pub(crate) fn get_index_raw(&self) -> SlotIndex {
        ((self.inner >> MP_SLOT_IDX_SH) & MP_SLOT_IDX_MSK) as SlotIndex
    }

    pub(crate) fn get_index(&self) -> Option<SlotIndex> {
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
pub type SlotAllocResult = Result<SlotPointer, SlotAllocError>;

#[derive(Debug, PartialEq, Eq)]
pub enum SlotFreeingError {
    SlotOutOfRange,
}

pub type SlotFreeingResult = Result<(), SlotFreeingError>;

pub struct SlotPool<const WORDS_PER_POOL: usize> {
     sto: AsyncArrayCell<usize, WORDS_PER_POOL>,
     words_per_slot: usize,
     pool_id: MemPoolId,
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

pub struct MemoryPool<'a> {
    id: MemPoolId,
    sto: AsyncArrayCellRef<'a, usize>,
    words_per_slot: usize,
    head: AtomicSlotPointer,
}

pub enum SlotAccessError {
    SlotOutOfRange,
    SlotNone,
}

impl<'a> MemoryPool<'a> {

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
    pub const fn get_slot_size(&self) -> usize {
        self.words_per_slot * core::mem::size_of::<usize>()
    }

    fn get_nb_slot(&self) -> usize {
        // let sto = &*(*self.sto.get()) as &[usize];
        self.sto.len() / self.words_per_slot
    }

    pub fn get_slot_raw_mut(
        &self,
        slot_pointer: &SlotPointer,
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
    pub unsafe fn get_slot_transmute<T>(
        &self,
        slot_pointer: &SlotPointer,
    ) -> Result<&mut MaybeUninit<T>, ()> {
        let slot_mem_ptr_res = self.get_slot_raw_mut(slot_pointer);
        if let Ok(slot_mem_ptr) = slot_mem_ptr_res {
            Ok((slot_mem_ptr as *mut MaybeUninit<T>).as_mut().unwrap())
        } else {
            Err(())
        }
    }

    fn get_empty_slot(
        &self,
        slot_pointer: &SlotPointer,
    ) -> Result<*const EmptySlot, SlotAccessError> {
        self.get_slot_raw_mut(slot_pointer)
            .map(|x: *mut u8| x as *const EmptySlot)
    }

    fn get_empty_slot_mut(
        &self,
        slot_pointer: &SlotPointer,
    ) -> Result<*mut EmptySlot, SlotAccessError> {
        self.get_slot_raw_mut(slot_pointer)
            .map(|x: *mut u8| x as *mut EmptySlot)
    }

    pub unsafe fn free(&self, slot_pointer: SlotPointer) -> SlotFreeingResult {
        println!(
            "pool {}: Freeing  slot {:?}",
            self.id,
            slot_pointer.get_index()
        );
        let new_head_slot = self
            .get_empty_slot_mut(&slot_pointer)
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

    pub fn allocate(&self, layout: core::alloc::Layout) -> SlotAllocResult {
        if layout.size() > (self.get_slot_size()) {
            return Err(SlotAllocError::SlotNotLargeEnough);
        }
        loop {
            let mut head = self.head.load(atomic::Ordering::Acquire);

            println!("pool {}: Allocating slot {:?}", self.id, head.get_index());
            if let Ok(head_slot) = self.get_empty_slot(&head) {
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

impl<'a> Allocator<SlotPointer, SlotFreeingError, SlotAllocError> for MemoryPool<'a> {
    unsafe fn free(&self, slot_pointer: SlotPointer) -> Result<(), SlotFreeingError> {
        self.free(slot_pointer)
    }

    fn allocate(&self, layout: core::alloc::Layout) -> Result<SlotPointer, SlotAllocError> {
        self.allocate(layout)
    }
}

impl<'a> MemoryAccessor<SlotPointer> for MemoryPool<'a> {
    fn get_slot_mut(&self, slot_pointer: &SlotPointer) -> Result<*mut u8, ()> {
        self.get_slot_raw_mut(slot_pointer).map_err(|_| ())
    }
}

#[cfg(test)]
pub mod tests {
    use core::marker::PhantomData;

    use rand::Rng;

    use super::super::super::Allocator;
    use super::*;

    mod basic_mem_pool_test {
        use super::*;
        const POOL0_ID: MemPoolId = 0;
        const POOL0_WORDS_PER_SLOT: usize = 1;
        const POOL0_SLOTS_PER_POOL: usize = 2;
        const POOL0_WORDS_PER_POOL: usize = POOL0_SLOTS_PER_POOL * POOL0_WORDS_PER_SLOT;
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

                let res0 = MEMORY_POOL_0.allocate(core::alloc::Layout::new::<Struct0>());
                let res0 = res0.unwrap();
                let struct0_0 = MEMORY_POOL_0.get_slot_transmute(&res0).unwrap();
                struct0_0.write(Struct0 {
                    a: core::usize::MAX,
                });

                let res1 = MEMORY_POOL_0.allocate(core::alloc::Layout::new::<Struct0>());
                let res1 = res1.unwrap();

                let struct0_1 = MEMORY_POOL_0.get_slot_transmute(&res1).unwrap();
                struct0_1.write(Struct0 {
                    a: core::usize::MIN,
                });

                let res2 = MEMORY_POOL_0.allocate(core::alloc::Layout::new::<Struct0>());
                assert_eq!(res2, Err(SlotAllocError::PoolFull));

                assert_eq!(struct0_0.assume_init_mut().a, core::usize::MAX);
                assert_eq!(struct0_1.assume_init_mut().a, core::usize::MIN);

                MEMORY_POOL_0.free(res1).unwrap();
                assert_eq!(struct0_0.assume_init_mut().a, core::usize::MAX);

                let res3 = SlotPointer {
                    inner: (res1.get_index_raw() + 1) as usize,
                };
                let res3 = MEMORY_POOL_0.free(res3);
                assert_eq!(res3, Err(SlotFreeingError::SlotOutOfRange));

                let res2 = MEMORY_POOL_0.allocate(core::alloc::Layout::new::<Struct1>());
                assert_eq!(res2, Err(SlotAllocError::SlotNotLargeEnough));

                let res4 = MEMORY_POOL_0.allocate(core::alloc::Layout::new::<Struct0>());
                let res4 = res4.unwrap();

                
                let struct0_4 = MEMORY_POOL_0.get_slot_transmute(&res4).unwrap();
                struct0_4.write(Struct0 {
                    a: 0xAAAAAAAAAAAAAAAA,
                });

                assert_eq!(struct0_0.assume_init_mut().a, core::usize::MAX);

                let res0 = MEMORY_POOL_0.free(res0);
                assert_eq!(res0, Ok(()));

                assert_eq!(struct0_4.assume_init_mut().a, 0xAAAAAAAAAAAAAAAA);
                let res4 = MEMORY_POOL_0.free(res4);
                res4.unwrap();
            }
        }
    }

    pub struct Tester<
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
        reference_allocator: Vec<Vec<(Vec<u8>, SlotPointer)>>,
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
        pub fn new(
            allocator: &'a AllocatorType,
        ) -> Tester<'a, FreeErrorType, AllocationErrorType, AllocatorType> {
            Self {
                allocator,
                reference_allocator: Vec::new(),
                phantom_free_err: PhantomData::default(),
                phantom_alloc_err: PhantomData::default(),
            }
        }

        unsafe fn allocate(&mut self, pool_idx: usize, element: &mut [u8]) {
            println!("Allocating for {}", pool_idx);
            let layout = core::alloc::Layout::array::<u8>(element.len()).unwrap();
            let pointer = self.allocator.allocate(layout).unwrap();

            let element_slot = self.allocator.get_slot_mut(&pointer).unwrap();
            element_slot.copy_from_nonoverlapping(element.as_ptr(), element.len());
            self.reference_allocator[pool_idx].push((element.to_vec(), pointer));
        }

        fn free(&mut self, pool_idx: usize, element_index: usize) {
            let (_, slot_pointer) = self.reference_allocator[pool_idx].remove(element_index);
            unsafe {
                self.allocator.free(slot_pointer).unwrap();
            }
        }

        pub fn run_integrity_check(&mut self) {
            for reference_pool_allocator in self.reference_allocator.iter() {
                for (reference_element, pointer) in reference_pool_allocator {
                    let element_slot = self.allocator.get_slot_mut(&pointer).unwrap();
                    for i in 0..reference_element.len() {
                        unsafe {
                            let val = *element_slot.offset(i as isize);
                            assert!(val == reference_element[i]);
                        }
                    }
                }
            }
        }

        fn get_previous_pool_max_element_size(&self, index: usize, tp: &TestParams) -> usize {
            if index == 0 {
                1
            } else {
                tp.pool_test_params[index - 1].max_element_size + 1
            }
        }
        pub fn run(&mut self, tp: TestParams) {
            let mut rng = rand::rng();

            //Fill the pool
            println!("Initial filling of the pool");
            for (pool_idx, pool_param) in tp.pool_test_params.iter().enumerate() {
                self.reference_allocator.push(Vec::new());
                let min_element_size = self.get_previous_pool_max_element_size(pool_idx, &tp);
                for _ in 0..pool_param.n_initial_elements {
                    let generated_element_size =
                        rng.random_range(min_element_size..pool_param.max_element_size);
                    let mut generated_element: Vec<_> = (&mut rng)
                        .random_iter::<u8>()
                        .take(generated_element_size)
                        .collect();
                    unsafe {
                        self.allocate(pool_idx, generated_element.as_mut_slice());
                    }
                }
            }

            self.run_integrity_check();

            println!("Test loop");
            for _ in 0..tp.n_iterations {
                let picked_tp_i = rng.random_range(..tp.pool_test_params.len());
                let n_elements_allocated = self.reference_allocator[picked_tp_i].len();
                let picked_tp = &tp.pool_test_params[picked_tp_i];
                let should_allocate;
                if n_elements_allocated == picked_tp.max_n_elements {
                    should_allocate = false;
                } else if n_elements_allocated == 0 {
                    should_allocate = true;
                } else {
                    should_allocate = rng.random_bool(0.5);
                }

                if should_allocate {
                    let min_element_size =
                        self.get_previous_pool_max_element_size(picked_tp_i, &tp);

                    let generated_element_size =
                        rng.random_range(min_element_size..picked_tp.max_element_size);
                    let mut generated_element: Vec<_> = (&mut rng)
                        .random_iter::<u8>()
                        .take(generated_element_size)
                        .collect();
                    unsafe {
                        self.allocate(picked_tp_i, generated_element.as_mut_slice());
                    }
                } else {
                    let element_index = rng.random_range(..n_elements_allocated);
                    self.free(picked_tp_i, element_index)
                }
            }
            self.run_integrity_check();
        }
    }

    pub struct PoolTestParams {
        pub max_n_elements: usize,
        pub max_element_size: usize,
        pub n_initial_elements: usize,
    }
    pub struct TestParams<'a> {
        pub pool_test_params: &'a [PoolTestParams],
        pub n_iterations: usize,
    }

    mod single_thread_randomized {
        use super::*;
        const POOL0_ID: MemPoolId = 0;
        const POOL0_WORDS_PER_SLOT: usize = 8;
        const POOL0_SLOT_PER_POOL: usize = 30;
        const POOL0_WORDS_PER_POOL: usize = POOL0_SLOT_PER_POOL * POOL0_WORDS_PER_SLOT;
        static STATIC_MEMORY_POOL: SlotPool<POOL0_WORDS_PER_POOL> =
            SlotPool::<POOL0_WORDS_PER_POOL>::new(POOL0_WORDS_PER_SLOT, POOL0_ID);
        static MEMORY_POOL_0: MemoryPool = MemoryPool::from(&STATIC_MEMORY_POOL);

        #[test]
        fn single_thread_randomized() {
            let pool_test_params = [PoolTestParams {
                max_n_elements: POOL0_SLOT_PER_POOL,
                max_element_size: POOL0_WORDS_PER_SLOT * core::mem::size_of::<usize>(),
                n_initial_elements: 10,
            }];

            let test_params = TestParams {
                pool_test_params: &pool_test_params,
                n_iterations: 10000,
            };

            let mut tester = Tester::new(&MEMORY_POOL_0);
            tester.run(test_params);
        }
    }
}

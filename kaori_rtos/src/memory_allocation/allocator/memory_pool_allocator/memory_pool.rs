use crate::sync::AsyncCell;
use core::result::Result;
use portable_atomic as atomic;
struct AtomicSlotPointer {
    inner: atomic::AtomicUsize,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct SlotPointer {
    inner: usize,
}

#[cfg(target_pointer_width = "32")]
mod types {
    pub type SlotIndex = u16;
    pub type SlotTag = u16;
    pub const MEM_POOL_TAG_SH: usize = 16;
    pub const MEM_POOL_TAG_MSK: usize = 0xff00;
    pub const TAG_MAX_VALUE: u16 = core::u16::MAX;
    pub const TAG_MIN_VALUE: u16 = 0;
    pub const MEM_POOL_IDX_SH: usize = 0;
    pub const MEM_POOL_IDX_MSK: usize = 0x00ff;
    pub const MEM_POOL_IDX_MAX_VALUE: u16 = core::u16::MAX - 1;
    pub const MEM_POOL_IDX_MIN_VALUE: u16 = 0;
    pub const NEXT_NONE: u16 = SlotIndex::MAX;
}

#[cfg(target_pointer_width = "64")]
mod types {
    pub type SlotIndex = u32;
    pub type SlotTag = u32;
    pub const MEM_POOL_TAG_SH: usize = 32;
    pub const MEM_POOL_TAG_MSK: usize = 0xffff0000;
    pub const TAG_MAX_VALUE: u32 = core::u32::MAX;
    pub const TAG_MIN_VALUE: u32 = 0;
    pub const MEM_POOL_IDX_SH: usize = 0;
    pub const MEM_POOL_IDX_MSK: usize = 0x0000ffff;
    pub const MEM_POOL_IDX_MAX_VALUE: u32 = core::u32::MAX - 1;
    pub const MEM_POOL_IDX_MIN_VALUE: u32 = 0;
    pub const NEXT_NONE: u32 = SlotIndex::MAX;
}

use types::*;

impl AtomicSlotPointer {
    const fn new(index: Option<SlotIndex>) -> AtomicSlotPointer {
        let new_index;
        if let Some(index) = index {
            new_index = index;
        } else {
            new_index = NEXT_NONE;
        }
        AtomicSlotPointer {
            inner: atomic::AtomicUsize::new(new_index as usize),
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
        let tag_and_index = self.inner.load(ordering);
        SlotPointer {
            inner: tag_and_index,
        }
    }

    fn store(&self, slot_pointer: SlotPointer, ordering: atomic::Ordering) {
        self.inner.store(slot_pointer.inner, ordering);
    }
}

impl SlotPointer {
    fn increment_tag(&mut self) {
        let tag_and_index = self.inner;
        let mut tag = ((tag_and_index & MEM_POOL_TAG_MSK) >> MEM_POOL_TAG_SH) as SlotTag;
        let index = ((tag_and_index & MEM_POOL_IDX_MSK) >> MEM_POOL_IDX_SH) as SlotIndex;
        if tag == TAG_MAX_VALUE {
            tag = TAG_MIN_VALUE;
        } else {
            tag += 1;
        }

        let new_tag_and_index =
            ((tag as usize) << MEM_POOL_TAG_SH) | ((index as usize) << MEM_POOL_IDX_SH);
        self.inner = new_tag_and_index;
    }

    fn get_index_raw(&self) -> SlotIndex {
        let tag_and_index = self.inner;
        (tag_and_index >> MEM_POOL_IDX_SH) as SlotIndex
    }

    fn get_index(&self) -> Option<SlotIndex> {
        let tag_and_index = self.inner;
        let index = (tag_and_index >> MEM_POOL_IDX_SH) as SlotIndex;
        if index != NEXT_NONE {
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
    pub(crate) sto: AsyncCell<[usize; WORDS_PER_POOL]>,
    pub(crate) words_per_slot: usize,
}

const NEXT_SLOT_NONE: usize = core::usize::MAX;

impl<const WORDS_PER_POOL: usize> SlotPool<WORDS_PER_POOL> {
    const unsafe fn init_pool_slots(sto: &mut [usize], words_per_slot: usize, idx: usize) {
        match sto {
            [] => {}
            [head, rest @ ..] => {
                if idx % words_per_slot == 0 {
                    if rest.len() == words_per_slot - 1 {
                        let empty_slot: &mut EmptySlot = core::mem::transmute(head);
                        *empty_slot = EmptySlot {
                            next: AtomicSlotPointer::new(None),
                        };
                    } else {
                        let empty_slot: &mut EmptySlot = core::mem::transmute(head);
                        let slot_index = (idx + words_per_slot) as SlotIndex;
                        *empty_slot = EmptySlot {
                            next: AtomicSlotPointer::new(Some(slot_index)),
                        };
                    }
                }
                Self::init_pool_slots(rest, words_per_slot, idx + 1);
            }
        }
    }
    const fn create_head(&self) -> AtomicSlotPointer {
        AtomicSlotPointer::new(Some(0))
    }

    pub const fn new(words_per_slot: usize) -> SlotPool<WORDS_PER_POOL> {
        assert!(words_per_slot > 0, "Slot size cannot be null");
        assert!(WORDS_PER_POOL > 0, "Slot pool length cannot be null");
        assert!(
            WORDS_PER_POOL % words_per_slot == 0,
            "Slot pool length must be a multiple of slot size"
        );
        assert!(
            (WORDS_PER_POOL / words_per_slot) <= (MEM_POOL_IDX_MAX_VALUE + 1) as usize,
            "Too many slots in slot pool"
        );
        unsafe {
            let mut sto: [usize; WORDS_PER_POOL] = [0; WORDS_PER_POOL];
            Self::init_pool_slots(&mut sto, words_per_slot, 0);
            SlotPool {
                sto: AsyncCell::new(sto),
                words_per_slot,
            }
        }
    }

    pub const fn get_nb_slots(&self) -> usize {
        self.sto.borrow().len() / self.words_per_slot
    }
}

struct EmptySlot {
    next: AtomicSlotPointer,
}

pub(crate) struct MemoryPool<'a> {
    sto: &'a [usize],
    words_per_slot: usize,
    head: AtomicSlotPointer,
}

pub(crate) enum SlotAccessError {
    SlotOutOfRange,
    SlotNone,
}

impl<'a> MemoryPool<'a> {
    pub const fn from<const WORDS_PER_POOL: usize>(
        slot_pool: &'a SlotPool<WORDS_PER_POOL>,
    ) -> MemoryPool<'a> {
        MemoryPool {
            sto: slot_pool.sto.borrow(),
            words_per_slot: slot_pool.words_per_slot,
            head: slot_pool.create_head(),
        }
    }
    fn get_slot_size(&self) -> usize {
        self.words_per_slot * core::mem::size_of::<usize>()
    }

    fn get_nb_slot(&self) -> usize {
        self.sto.len() / self.words_per_slot
    }

    pub(crate) fn get_slot_raw_mut(
        &self,
        slot_pointer: SlotPointer,
    ) -> Result<*mut u8, SlotAccessError> {
        let slot_index = slot_pointer.get_index_raw();
        if slot_index >= MEM_POOL_IDX_MIN_VALUE && slot_index < self.get_nb_slot() as SlotIndex {
            unsafe {
                let raw_ptr = self
                    .sto
                    .as_ptr()
                    .add((slot_index as usize) * self.words_per_slot);
                Ok(core::mem::transmute(raw_ptr))
            }
        } else {
            if slot_index == NEXT_NONE {
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
        let new_head_slot = self
            .get_slot_mut(slot_pointer)
            .map_err(|_| SlotFreeingError::SlotOutOfRange)?;

        loop {
            let head = self.head.load(atomic::Ordering::SeqCst);
            *new_head_slot = EmptySlot {
                next: AtomicSlotPointer::from(head),
            };
            if let Err(_) = self.head.compare_exchange_weak(
                head,
                slot_pointer,
                atomic::Ordering::SeqCst,
                atomic::Ordering::SeqCst,
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
            let mut head = self.head.load(atomic::Ordering::SeqCst);
            if let Ok(head_slot) = self.get_slot(head) {
                unsafe {
                    let head_next = &(*head_slot).next;
                    let new_head = head_next.load(atomic::Ordering::SeqCst);
                    if let Err(_) = self.head.compare_exchange_weak(
                        head,
                        new_head,
                        atomic::Ordering::SeqCst,
                        atomic::Ordering::SeqCst,
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
    use super::*;

    const POOL0_WORDS_PER_SLOT: usize = 1;
    const POOL0_SLOTS_PER_POOL: usize = 2;
    static STATIC_MEMORY_POOL: SlotPool<POOL0_SLOTS_PER_POOL> =
        SlotPool::<POOL0_SLOTS_PER_POOL>::new(POOL0_WORDS_PER_SLOT);
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

use crate::memory_pool::MemoryPool;

struct QueueElement {}

pub(crate) struct EvtQueue<MemoryPoolT> {
    head: QueueElement,
    memory_pool: MemoryPoolT,
}

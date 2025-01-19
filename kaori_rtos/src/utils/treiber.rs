use core::mem::ManuallyDrop;

#[cfg_attr(not(arm_llsc), path = "treiber/cas.rs")]
#[cfg_attr(arm_llsc, path = "treiber/llsc.rs")]
mod impl_;

pub use impl_::{AtomicPtr, NonNullPtr};

pub struct Stack
{
    top: AtomicPtr<Node>,
}

impl Stack
{
    pub const fn new() -> Self {
        Self {
            top: AtomicPtr::null(),
        }
    }

    /// # Safety
    /// - `node` must be a valid pointer
    /// - aliasing rules must be enforced by the caller. e.g, the same `node` may not be pushed more than once
    pub unsafe fn push(&self, node: NonNullPtr<Node>) {
        impl_::push(self, node)
    }

    pub fn try_pop(&self) -> Option<NonNullPtr<Node>> {
        impl_::try_pop(self)
    }
}

pub struct Node {
    pub next: ManuallyDrop<AtomicPtr<Node>>,
}

impl Node{

    fn next(&self) -> &AtomicPtr<Self> {
        &self.next
    }

    fn next_mut(&mut self) -> &mut AtomicPtr<Self> {
        &mut self.next
    }
}

#[cfg(test)]
mod tests {
    use core::mem;

    use super::*;

}

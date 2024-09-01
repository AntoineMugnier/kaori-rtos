#![no_std]
mod cortex_m_port;
mod event_queue;
mod memory_pool;
use cortex_m_port as port;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {}
}

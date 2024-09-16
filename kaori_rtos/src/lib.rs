#![cfg_attr(not(test), no_std)]
mod cortex_m_port;
mod memory_allocation;
use cortex_m_port as port;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {}
}

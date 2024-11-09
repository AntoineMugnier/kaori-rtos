//#![feature(const_mut_refs)]
#![cfg_attr(target_os = "none", no_std)]

mod event;
mod memory_allocation;
mod sync;

// #[cfg(
//     all(
//         any(target = "thumbv7em-none-eabi"),
//         target_feature = "avx2"
//     )
// )]



// #[cfg(all(not(armv6m), not(armv8m_base)))]

#[cfg(not(target_os = "none"))]
mod std_lib_port;

#[cfg(not(target_os = "none"))]
use std_lib_port as port;

#[cfg(target_os = "none")]
use cortex_m_port as port;
#[cfg(target_os = "none")]
mod cortex_m_port;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {}
}

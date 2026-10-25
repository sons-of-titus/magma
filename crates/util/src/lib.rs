//! Shared primitives for Magma: reusable ID allocation and child-process
//! hygiene.

pub mod id_pool;
pub mod proctree;

pub use id_pool::IdPool;

#[cfg(test)]
mod tests;

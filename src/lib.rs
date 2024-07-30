//! Utility functions for the UCS-2 character encoding.

#![no_std]
#![deny(missing_docs)]
#![deny(clippy::all)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod encoding;
pub mod types;

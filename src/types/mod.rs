//! Rusty-types to work with UCS-2 strings and for convenient interoperability
//! with Rust string literals (`&str`) and Rust strings (`String`).

mod chars;
mod cstr16;
mod cstr8;
#[cfg(feature = "alloc")]
mod cstring16;
mod macros;
mod unaligned_slice;

pub use crate::cstr16;
pub use crate::cstr8;
pub use chars::*;
use core::fmt;
use core::fmt::{Display, Formatter};
pub use cstr16::*;
pub use cstr8::*;
#[cfg(feature = "alloc")]
pub use cstring16::*;
pub use unaligned_slice::*;

/// Errors which can occur during checked `[uN]` -> `CStrN` conversions
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FromSliceWithNulError {
    /// An invalid character was encountered before the end of the slice
    InvalidChar(usize),

    /// A null character was encountered before the end of the slice
    InteriorNul(usize),

    /// The slice was not null-terminated
    NotNulTerminated,
}

impl Display for FromSliceWithNulError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidChar(usize) => write!(f, "invalid character at index {}", usize),
            Self::InteriorNul(usize) => write!(f, "interior null character at index {}", usize),
            Self::NotNulTerminated => write!(f, "not null-terminated"),
        }
    }
}

#[cfg(feature = "unstable")]
impl core::error::Error for FromSliceWithNulError {}

/// Error returned by [`CStr16::from_str_with_buf`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FromStrWithBufError {
    /// An invalid character was encountered before the end of the string
    InvalidChar(usize),

    /// A null character was encountered in the string
    InteriorNul(usize),

    /// The buffer is not big enough to hold the entire string and
    /// trailing null character
    BufferTooSmall,
}

impl Display for FromStrWithBufError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidChar(usize) => write!(f, "invalid character at index {}", usize),
            Self::InteriorNul(usize) => write!(f, "interior null character at index {}", usize),
            Self::BufferTooSmall => write!(f, "buffer too small"),
        }
    }
}

#[cfg(feature = "unstable")]
impl core::error::Error for FromStrWithBufError {}

/// The EqStrUntilNul trait helps to compare Rust strings against UEFI string types (UCS-2 strings).
/// The given generic implementation of this trait enables us that we only have to
/// implement one direction (`left.eq_str_until_nul(&right)`) for each UEFI string type and we
/// get the other direction (`right.eq_str_until_nul(&left)`) for free. Hence, the relation is
/// reflexive.
pub trait EqStrUntilNul<StrType: ?Sized> {
    /// Checks if the provided Rust string `StrType` is equal to [Self] until the first null character
    /// is found. An exception is the terminating null character of [Self] which is ignored.
    ///
    /// As soon as the first null character in either `&self` or `other` is found, this method returns.
    /// Note that Rust strings are allowed to contain null bytes that do not terminate the string.
    /// Although this is rather unusual, you can compare `"foo\0bar"` with an instance of [Self].
    /// In that case, only `foo"` is compared against [Self] (if [Self] is long enough).
    fn eq_str_until_nul(&self, other: &StrType) -> bool;
}

// magic implementation which transforms an existing `left.eq_str_until_nul(&right)` implementation
// into an additional working `right.eq_str_until_nul(&left)` implementation.
impl<StrType, C16StrType> EqStrUntilNul<C16StrType> for StrType
where
    StrType: AsRef<str>,
    C16StrType: EqStrUntilNul<StrType> + ?Sized,
{
    fn eq_str_until_nul(&self, other: &C16StrType) -> bool {
        // reuse the existing implementation
        other.eq_str_until_nul(self)
    }
}

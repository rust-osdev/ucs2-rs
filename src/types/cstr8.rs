//! Rusty-types to work with UCS-2 strings and for convenient interoperability
//! with Rust string literals (`&str`) and Rust strings (`String`).

use crate::types::chars::{Char8, NUL_8};
use crate::types::{EqStrUntilNul, FromSliceWithNulError};
use core::borrow::Borrow;
use core::ffi::CStr;
use core::{fmt, slice};

/// A null-terminated Latin-1 string.
///
/// This type is largely inspired by [`core::ffi::CStr`] with the exception that all characters are
/// guaranteed to be 8 bit long.
///
/// A [`CStr8`] can be constructed from a [`core::ffi::CStr`] via a `try_from` call:
/// ```ignore
/// let cstr8: &CStr8 = TryFrom::try_from(cstr).unwrap();
/// ```
///
/// For convenience, a [`CStr8`] is comparable with [`core::str`] and
/// `alloc::string::String` from the standard library through the trait [`EqStrUntilNul`].
#[repr(transparent)]
#[derive(Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CStr8([Char8]);

impl CStr8 {
    /// Takes a raw pointer to a null-terminated Latin-1 string and wraps it in a CStr8 reference.
    ///
    /// # Safety
    ///
    /// The function will start accessing memory from `ptr` until the first
    /// null byte. It's the callers responsibility to ensure `ptr` points to
    /// a valid null-terminated string in accessible memory.
    #[must_use]
    pub unsafe fn from_ptr<'ptr>(ptr: *const Char8) -> &'ptr Self {
        let mut len = 0;
        while *ptr.add(len) != NUL_8 {
            len += 1
        }
        let ptr = ptr.cast::<u8>();
        Self::from_bytes_with_nul_unchecked(slice::from_raw_parts(ptr, len + 1))
    }

    /// Creates a CStr8 reference from bytes.
    pub fn from_bytes_with_nul(chars: &[u8]) -> Result<&Self, FromSliceWithNulError> {
        let nul_pos = chars.iter().position(|&c| c == 0);
        if let Some(nul_pos) = nul_pos {
            if nul_pos + 1 != chars.len() {
                return Err(FromSliceWithNulError::InteriorNul(nul_pos));
            }
            Ok(unsafe { Self::from_bytes_with_nul_unchecked(chars) })
        } else {
            Err(FromSliceWithNulError::NotNulTerminated)
        }
    }

    /// Unsafely creates a CStr8 reference from bytes.
    ///
    /// # Safety
    ///
    /// It's the callers responsibility to ensure chars is a valid Latin-1
    /// null-terminated string, with no interior null bytes.
    #[must_use]
    pub const unsafe fn from_bytes_with_nul_unchecked(chars: &[u8]) -> &Self {
        &*(chars as *const [u8] as *const Self)
    }

    /// Returns the inner pointer to this CStr8.
    #[must_use]
    pub const fn as_ptr(&self) -> *const Char8 {
        self.0.as_ptr()
    }

    /// Returns the underlying bytes as slice including the terminating null
    /// character.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        unsafe { &*(&self.0 as *const [Char8] as *const [u8]) }
    }
}

impl fmt::Debug for CStr8 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CStr8({:?})", &self.0)
    }
}

impl fmt::Display for CStr8 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for c in self.0.iter() {
            <Char8 as fmt::Display>::fmt(c, f)?;
        }
        Ok(())
    }
}

impl AsRef<[u8]> for CStr8 {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl Borrow<[u8]> for CStr8 {
    fn borrow(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<StrType: AsRef<str> + ?Sized> EqStrUntilNul<StrType> for CStr8 {
    fn eq_str_until_nul(&self, other: &StrType) -> bool {
        let other = other.as_ref();

        // TODO: CStr16 has .iter() implemented, CStr8 not yet
        let any_not_equal = self
            .0
            .iter()
            .copied()
            .map(char::from)
            .zip(other.chars())
            // This only works as CStr8 is guaranteed to have a fixed character length
            // (unlike UTF-8).
            .take_while(|(l, r)| *l != '\0' && *r != '\0')
            .any(|(l, r)| l != r);

        !any_not_equal
    }
}

impl<'a> TryFrom<&'a CStr> for &'a CStr8 {
    type Error = FromSliceWithNulError;

    fn try_from(cstr: &'a CStr) -> Result<Self, Self::Error> {
        CStr8::from_bytes_with_nul(cstr.to_bytes_with_nul())
    }
}

/// Get a Latin-1 character from a UTF-8 byte slice at the given offset.
///
/// Returns a pair containing the Latin-1 character and the number of bytes in
/// the UTF-8 encoding of that character.
///
/// Panics if the string cannot be encoded in Latin-1.
///
/// # Safety
///
/// The input `bytes` must be valid UTF-8.
const unsafe fn latin1_from_utf8_at_offset(bytes: &[u8], offset: usize) -> (u8, usize) {
    if bytes[offset] & 0b1000_0000 == 0b0000_0000 {
        (bytes[offset], 1)
    } else if bytes[offset] & 0b1110_0000 == 0b1100_0000 {
        let a = (bytes[offset] & 0b0001_1111) as u16;
        let b = (bytes[offset + 1] & 0b0011_1111) as u16;
        let ch = a << 6 | b;
        if ch > 0xff {
            panic!("input string cannot be encoded as Latin-1");
        }
        (ch as u8, 2)
    } else {
        // Latin-1 code points only go up to 0xff, so if the input contains any
        // UTF-8 characters larger than two bytes it cannot be converted to
        // Latin-1.
        panic!("input string cannot be encoded as Latin-1");
    }
}

/// Count the number of Latin-1 characters in a string.
///
/// Panics if the string cannot be encoded in Latin-1.
///
/// This is public but hidden; it is used in the `cstr8` macro.
#[must_use]
pub const fn str_num_latin1_chars(s: &str) -> usize {
    let bytes = s.as_bytes();
    let len = bytes.len();

    let mut offset = 0;
    let mut num_latin1_chars = 0;

    while offset < len {
        // SAFETY: `bytes` is valid UTF-8.
        let (_, num_utf8_bytes) = unsafe { latin1_from_utf8_at_offset(bytes, offset) };
        offset += num_utf8_bytes;
        num_latin1_chars += 1;
    }

    num_latin1_chars
}

/// Convert a `str` into a null-terminated Latin-1 character array.
///
/// Panics if the string cannot be encoded in Latin-1.
///
/// This is public but hidden; it is used in the `cstr8` macro.
#[must_use]
pub const fn str_to_latin1<const N: usize>(s: &str) -> [u8; N] {
    let bytes = s.as_bytes();
    let len = bytes.len();

    let mut output = [0; N];

    let mut output_offset = 0;
    let mut input_offset = 0;
    while input_offset < len {
        // SAFETY: `bytes` is valid UTF-8.
        let (ch, num_utf8_bytes) = unsafe { latin1_from_utf8_at_offset(bytes, input_offset) };
        if ch == 0 {
            panic!("interior null character");
        } else {
            output[output_offset] = ch;
            output_offset += 1;
            input_offset += num_utf8_bytes;
        }
    }

    // The output array must be one bigger than the converted string,
    // to leave room for the trailing null character.
    if output_offset + 1 != N {
        panic!("incorrect array length");
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cstr8;

    // Tests if our CStr8 type can be constructed from a valid core::ffi::CStr
    #[test]
    fn test_cstr8_from_cstr() {
        let msg = "hello world\0";
        let cstr = unsafe { CStr::from_ptr(msg.as_ptr().cast()) };
        let cstr8: &CStr8 = TryFrom::try_from(cstr).unwrap();
        assert!(cstr8.eq_str_until_nul(msg));
        assert!(msg.eq_str_until_nul(cstr8));
    }

    #[test]
    fn test_cstr8_as_bytes() {
        let string: &CStr8 = cstr8!("a");
        assert_eq!(string.as_bytes(), &[b'a', 0]);
        assert_eq!(<CStr8 as AsRef<[u8]>>::as_ref(string), &[b'a', 0]);
        assert_eq!(<CStr8 as Borrow<[u8]>>::borrow(string), &[b'a', 0]);
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests_with_alloc {
    use super::*;
    use crate::cstr8;
    use alloc::string::String;

    // Code generation helper for the compare tests of our CStrX types against "str" and "String"
    // from the standard library.
    #[allow(non_snake_case)]
    macro_rules! test_compare_cstrX {
        ($input:ident) => {
            assert!($input.eq_str_until_nul(&"test"));
            assert!($input.eq_str_until_nul(&String::from("test")));

            // now other direction
            assert!(String::from("test").eq_str_until_nul($input));
            assert!("test".eq_str_until_nul($input));

            // some more tests
            // this is fine: compare until the first null
            assert!($input.eq_str_until_nul(&"te\0st"));
            // this is fine
            assert!($input.eq_str_until_nul(&"test\0"));
            assert!(!$input.eq_str_until_nul(&"hello"));
        };
    }

    /// Tests the trait implementation of trait [`EqStrUntilNul]` for [`CStr8`].
    ///
    /// This tests that `String` and `str` from the standard library can be
    /// checked for equality against a [`CStr8`]. It checks both directions,
    /// i.e., the equality is reflexive.
    #[test]
    fn test_cstr8_eq_std_str() {
        let input: &CStr8 = cstr8!("test");

        // test various comparisons with different order (left, right)
        assert!(input.eq_str_until_nul("test")); // requires ?Sized constraint
        assert!(input.eq_str_until_nul(&"test"));
        assert!(input.eq_str_until_nul(&String::from("test")));

        // now other direction
        assert!(String::from("test").eq_str_until_nul(input));
        assert!("test".eq_str_until_nul(input));
    }

    #[test]
    fn test_compare_cstr8() {
        // test various comparisons with different order (left, right)
        let input: &CStr8 = cstr8!("test");
        test_compare_cstrX!(input);
    }
}

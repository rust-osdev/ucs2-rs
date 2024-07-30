use crate::polyfill::maybe_uninit_slice_assume_init_ref;
use crate::types::chars::{Char16, NUL_16};
use crate::types::unaligned_slice::UnalignedSlice;
use crate::types::{EqStrUntilNul, FromSliceWithNulError, FromStrWithBufError};
use core::borrow::Borrow;
use core::fmt::{Display, Formatter};
use core::mem::MaybeUninit;
use core::{fmt, slice};

/// Error returned by [`CStr16::from_unaligned_slice`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnalignedCStr16Error {
    /// An invalid character was encountered.
    InvalidChar(usize),

    /// A null character was encountered before the end of the data.
    InteriorNul(usize),

    /// The data was not null-terminated.
    NotNulTerminated,

    /// The buffer is not big enough to hold the entire string and
    /// trailing null character.
    BufferTooSmall,
}

impl Display for UnalignedCStr16Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidChar(usize) => write!(f, "invalid character at index {}", usize),
            Self::InteriorNul(usize) => write!(f, "interior null character at index {}", usize),
            Self::NotNulTerminated => write!(f, "not null-terminated"),
            Self::BufferTooSmall => write!(f, "buffer too small"),
        }
    }
}

#[cfg(feature = "unstable")]
impl core::error::Error for UnalignedCStr16Error {}

/// An UCS-2 null-terminated string slice.
///
/// This type is largely inspired by [`core::ffi::CStr`] with the exception that all characters are
/// guaranteed to be 16 bit long.
///
/// For convenience, a [`CStr16`] is comparable with [`core::str`] and
/// `alloc::string::String` from the standard library through the trait [`EqStrUntilNul`].
#[derive(Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
pub struct CStr16([Char16]);

impl CStr16 {
    /// Wraps a raw UEFI string with a safe C string wrapper
    ///
    /// # Safety
    ///
    /// The function will start accessing memory from `ptr` until the first
    /// null character. It's the callers responsibility to ensure `ptr` points to
    /// a valid string, in accessible memory.
    #[must_use]
    pub unsafe fn from_ptr<'ptr>(ptr: *const Char16) -> &'ptr Self {
        let mut len = 0;
        while *ptr.add(len) != NUL_16 {
            len += 1
        }
        let ptr = ptr.cast::<u16>();
        Self::from_u16_with_nul_unchecked(slice::from_raw_parts(ptr, len + 1))
    }

    /// Creates a `&CStr16` from a u16 slice, if the slice contains exactly
    /// one terminating null-byte and all chars are valid UCS-2 chars.
    pub fn from_u16_with_nul(codes: &[u16]) -> Result<&Self, FromSliceWithNulError> {
        for (pos, &code) in codes.iter().enumerate() {
            match code.try_into() {
                Ok(NUL_16) => {
                    if pos != codes.len() - 1 {
                        return Err(FromSliceWithNulError::InteriorNul(pos));
                    } else {
                        return Ok(unsafe { Self::from_u16_with_nul_unchecked(codes) });
                    }
                }
                Err(_) => {
                    return Err(FromSliceWithNulError::InvalidChar(pos));
                }
                _ => {}
            }
        }
        Err(FromSliceWithNulError::NotNulTerminated)
    }

    /// Unsafely creates a `&CStr16` from a u16 slice.
    ///
    /// # Safety
    ///
    /// It's the callers responsibility to ensure chars is a valid UCS-2
    /// null-terminated string, with no interior null characters.
    #[must_use]
    pub const unsafe fn from_u16_with_nul_unchecked(codes: &[u16]) -> &Self {
        &*(codes as *const [u16] as *const Self)
    }

    /// Creates a `&CStr16` from a [`Char16`] slice, if the slice is
    /// null-terminated and has no interior null characters.
    pub fn from_char16_with_nul(chars: &[Char16]) -> Result<&Self, FromSliceWithNulError> {
        // Fail early if the input is empty.
        if chars.is_empty() {
            return Err(FromSliceWithNulError::NotNulTerminated);
        }

        // Find the index of the first null char.
        if let Some(null_index) = chars.iter().position(|c| *c == NUL_16) {
            // Verify the null character is at the end.
            if null_index == chars.len() - 1 {
                // Safety: the input is null-terminated and has no interior nulls.
                Ok(unsafe { Self::from_char16_with_nul_unchecked(chars) })
            } else {
                Err(FromSliceWithNulError::InteriorNul(null_index))
            }
        } else {
            Err(FromSliceWithNulError::NotNulTerminated)
        }
    }

    /// Unsafely creates a `&CStr16` from a `Char16` slice.
    ///
    /// # Safety
    ///
    /// It's the callers responsibility to ensure chars is null-terminated and
    /// has no interior null characters.
    #[must_use]
    pub const unsafe fn from_char16_with_nul_unchecked(chars: &[Char16]) -> &Self {
        let ptr: *const [Char16] = chars;
        &*(ptr as *const Self)
    }

    /// Convert a [`&str`] to a `&CStr16`, backed by a buffer.
    ///
    /// The input string must contain only characters representable with
    /// UCS-2, and must not contain any null characters (even at the end of
    /// the input).
    ///
    /// The backing buffer must be big enough to hold the converted string as
    /// well as a trailing null character.
    ///
    /// # Examples
    ///
    /// Convert the UTF-8 string "ABC" to a `&CStr16`:
    ///
    /// ```
    /// use ucs2::types::CStr16;
    ///
    /// let mut buf = [0; 4];
    /// CStr16::from_str_with_buf("ABC", &mut buf).unwrap();
    /// ```
    pub fn from_str_with_buf<'a>(
        input: &str,
        buf: &'a mut [u16],
    ) -> Result<&'a Self, FromStrWithBufError> {
        let mut index = 0;

        // Convert to UTF-16.
        for c in input.encode_utf16() {
            *buf.get_mut(index)
                .ok_or(FromStrWithBufError::BufferTooSmall)? = c;
            index += 1;
        }

        // Add trailing null character.
        *buf.get_mut(index)
            .ok_or(FromStrWithBufError::BufferTooSmall)? = 0;

        // Convert from u16 to Char16. This checks for invalid UCS-2 chars and
        // interior nulls. The NotNulTerminated case is unreachable because we
        // just added a trailing null character.
        Self::from_u16_with_nul(&buf[..index + 1]).map_err(|err| match err {
            FromSliceWithNulError::InvalidChar(p) => FromStrWithBufError::InvalidChar(p),
            FromSliceWithNulError::InteriorNul(p) => FromStrWithBufError::InteriorNul(p),
            FromSliceWithNulError::NotNulTerminated => {
                unreachable!()
            }
        })
    }

    /// Create a `&CStr16` from an [`UnalignedSlice`] using an aligned
    /// buffer for storage. The lifetime of the output is tied to `buf`,
    /// not `src`.
    pub fn from_unaligned_slice<'buf>(
        src: &UnalignedSlice<'_, u16>,
        buf: &'buf mut [MaybeUninit<u16>],
    ) -> Result<&'buf Self, UnalignedCStr16Error> {
        // The input `buf` might be longer than needed, so get a
        // subslice of the required length.
        let buf = buf
            .get_mut(..src.len())
            .ok_or(UnalignedCStr16Error::BufferTooSmall)?;

        src.copy_to_maybe_uninit(buf);
        let buf = unsafe {
            // Safety: `copy_buf` fully initializes the slice.
            maybe_uninit_slice_assume_init_ref(buf)
        };
        Self::from_u16_with_nul(buf).map_err(|e| match e {
            FromSliceWithNulError::InvalidChar(v) => UnalignedCStr16Error::InvalidChar(v),
            FromSliceWithNulError::InteriorNul(v) => UnalignedCStr16Error::InteriorNul(v),
            FromSliceWithNulError::NotNulTerminated => UnalignedCStr16Error::NotNulTerminated,
        })
    }

    /// Returns the inner pointer to this C16 string.
    #[must_use]
    pub const fn as_ptr(&self) -> *const Char16 {
        self.0.as_ptr()
    }

    /// Get the underlying [`Char16`]s as slice without the trailing null.
    #[must_use]
    pub fn as_slice(&self) -> &[Char16] {
        &self.0[..self.num_chars()]
    }

    /// Get the underlying [`Char16`]s as slice including the trailing null.
    #[must_use]
    pub const fn as_slice_with_nul(&self) -> &[Char16] {
        &self.0
    }

    /// Converts this C string to a u16 slice without the trailing null.
    #[must_use]
    pub fn to_u16_slice(&self) -> &[u16] {
        let chars = self.to_u16_slice_with_nul();
        &chars[..chars.len() - 1]
    }

    /// Converts this C string to a u16 slice containing the trailing null.
    #[must_use]
    pub const fn to_u16_slice_with_nul(&self) -> &[u16] {
        unsafe { &*(&self.0 as *const [Char16] as *const [u16]) }
    }

    /// Returns an iterator over this C string
    #[must_use]
    pub const fn iter(&self) -> CStr16Iter {
        CStr16Iter {
            inner: self,
            pos: 0,
        }
    }

    /// Returns the number of characters without the trailing null. character
    #[must_use]
    pub const fn num_chars(&self) -> usize {
        self.0.len() - 1
    }

    /// Returns if the string is empty. This ignores the null character.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.num_chars() == 0
    }

    /// Get the number of bytes in the string (including the trailing null).
    #[must_use]
    pub const fn num_bytes(&self) -> usize {
        self.0.len() * 2
    }

    /// Checks if all characters in this string are within the ASCII range.
    #[must_use]
    pub fn is_ascii(&self) -> bool {
        self.0.iter().all(|c| c.is_ascii())
    }

    /// Writes each [`Char16`] as a [`char`] (4 bytes long in Rust language) into the buffer.
    /// It is up to the implementer of [`core::fmt::Write`] to convert the char to a string
    /// with proper encoding/charset. For example, in the case of [`alloc::string::String`]
    /// all Rust chars (UTF-32) get converted to UTF-8.
    ///
    /// ## Example
    ///
    /// ```ignore
    /// let firmware_vendor_c16_str: CStr16 = ...;
    /// // crate "arrayvec" uses stack-allocated arrays for Strings => no heap allocations
    /// let mut buf = arrayvec::ArrayString::<128>::new();
    /// firmware_vendor_c16_str.as_str_in_buf(&mut buf);
    /// log::info!("as rust str: {}", buf.as_str());
    /// ```
    ///
    /// [`alloc::string::String`]: https://doc.rust-lang.org/nightly/alloc/string/struct.String.html
    pub fn as_str_in_buf(&self, buf: &mut dyn core::fmt::Write) -> core::fmt::Result {
        for c16 in self.iter() {
            buf.write_char(char::from(*c16))?;
        }
        Ok(())
    }

    /// Returns the underlying bytes as slice including the terminating null
    /// character.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.0.as_ptr().cast(), self.num_bytes()) }
    }
}

impl AsRef<[u8]> for CStr16 {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl Borrow<[u8]> for CStr16 {
    fn borrow(&self) -> &[u8] {
        self.as_bytes()
    }
}

#[cfg(feature = "alloc")]
impl From<&CStr16> for alloc::string::String {
    fn from(value: &CStr16) -> Self {
        value
            .as_slice()
            .iter()
            .copied()
            .map(u16::from)
            .map(u32::from)
            .map(|int| char::from_u32(int).expect("Should be encodable as UTF-8"))
            .collect::<Self>()
    }
}

impl<StrType: AsRef<str> + ?Sized> EqStrUntilNul<StrType> for CStr16 {
    fn eq_str_until_nul(&self, other: &StrType) -> bool {
        let other = other.as_ref();

        let any_not_equal = self
            .iter()
            .copied()
            .map(char::from)
            .zip(other.chars())
            // This only works as CStr16 is guaranteed to have a fixed character length
            // (unlike UTF-8 or UTF-16).
            .take_while(|(l, r)| *l != '\0' && *r != '\0')
            .any(|(l, r)| l != r);

        !any_not_equal
    }
}

impl AsRef<Self> for CStr16 {
    fn as_ref(&self) -> &Self {
        self
    }
}

/// An iterator over the [`Char16`]s in a [`CStr16`].
#[derive(Debug)]
pub struct CStr16Iter<'a> {
    inner: &'a CStr16,
    pos: usize,
}

impl<'a> Iterator for CStr16Iter<'a> {
    type Item = &'a Char16;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.inner.0.len() - 1 {
            None
        } else {
            self.pos += 1;
            self.inner.0.get(self.pos - 1)
        }
    }
}

impl fmt::Debug for CStr16 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CStr16({:?})", &self.0)
    }
}

impl fmt::Display for CStr16 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for c in self.iter() {
            <Char16 as fmt::Display>::fmt(c, f)?;
        }
        Ok(())
    }
}

#[cfg(feature = "alloc")]
impl PartialEq<CString16> for &CStr16 {
    fn eq(&self, other: &CString16) -> bool {
        PartialEq::eq(*self, other.as_ref())
    }
}

impl<'a> UnalignedSlice<'a, u16> {
    /// Create a [`CStr16`] from an [`UnalignedSlice`] using an aligned
    /// buffer for storage. The lifetime of the output is tied to `buf`,
    /// not `self`.
    pub fn to_cstr16<'buf>(
        &self,
        buf: &'buf mut [MaybeUninit<u16>],
    ) -> Result<&'buf CStr16, UnalignedCStr16Error> {
        CStr16::from_unaligned_slice(self, buf)
    }
}

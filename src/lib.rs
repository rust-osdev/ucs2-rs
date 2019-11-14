//! Utility functions for the UCS-2 character encoding.

#![no_std]

#[deny(missing_docs)]
#[deny(clippy::all)]

/// Possible errors returned by the API.
#[derive(Debug, Copy, Clone)]
pub enum Error {
    /// Not enough space left in the output buffer.
    BufferOverflow,
    /// Input contained a character which cannot be represented in UCS-2.
    MultiByte,
}

type Result<T> = core::result::Result<T, Error>;

/// Encodes an input UTF-8 string into a UCS-2 string.
///
/// The returned `usize` represents the length of the returned buffer,
/// measured in 2-byte characters.
pub fn encode(input: &str, buffer: &mut [u16]) -> Result<usize> {
    let buffer_size = buffer.len();
    let mut i = 0;

    encode_with(input, |ch| {
        if i >= buffer_size {
            Err(Error::BufferOverflow)
        } else {
            buffer[i] = ch;
            i += 1;
            Ok(())
        }
    })?;

    Ok(i)
}

/// Encode UTF-8 string to UCS-2 with a custom callback function.
///
/// `output` is a function which receives every encoded character.
pub fn encode_with<F>(input: &str, mut output: F) -> Result<()>
where
    F: FnMut(u16) -> Result<()>,
{
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        let ch;

        if bytes[i] & 0b1000_0000 == 0b0000_0000 {
            ch = u16::from(bytes[i]);
            i += 1;
        } else if bytes[i] & 0b1110_0000 == 0b1100_0000 {
            // 2 byte codepoint
            if i + 1 >= len {
                // safe: len is the length of bytes,
                // and bytes is a direct view into the
                // buffer of input, which in order to be a valid
                // utf-8 string _must_ contain `i + 1`.
                unsafe { core::hint::unreachable_unchecked() }
            }

            let a = u16::from(bytes[i] & 0b0001_1111);
            let b = u16::from(bytes[i + 1] & 0b0011_1111);
            ch = a << 6 | b;
            i += 2;
        } else if bytes[i] & 0b1111_0000 == 0b1110_0000 {
            // 3 byte codepoint
            if i + 2 >= len || i + 1 >= len {
                // safe: impossible utf-8 string.
                unsafe { core::hint::unreachable_unchecked() }
            }

            let a = u16::from(bytes[i] & 0b0000_1111);
            let b = u16::from(bytes[i + 1] & 0b0011_1111);
            let c = u16::from(bytes[i + 2] & 0b0011_1111);
            ch = a << 12 | b << 6 | c;
            i += 3;
        } else if bytes[i] & 0b1111_0000 == 0b1111_0000 {
            return Err(Error::MultiByte); // UTF-16
        } else {
            // safe: impossible utf-8 string.
            unsafe { core::hint::unreachable_unchecked() }
        }
        output(ch)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding() {
        let input = "őэ╋";
        let mut buffer = [0u16; 3];

        let result = encode(input, &mut buffer);
        assert_eq!(result.unwrap(), 3);

        assert_eq!(buffer[..], [0x0151, 0x044D, 0x254B]);
    }
}

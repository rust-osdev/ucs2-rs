//! Utility functions for the UCS-2 character encoding.

#![no_std]
#![deny(missing_docs)]
#![deny(clippy::all)]

use bit_field::BitField;

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

/// Decode UCS-2 string to UTF-8 with a custom callback function.
///
/// `output` is a function which receives every decoded character.
pub fn decode_with<F>(input: &[u16], mut output: F) -> Result<usize>
where
    F: FnMut(&[u8]) -> Result<()>,
{
    let mut written = 0;

    for ch in input.iter() {
        /*
         * We need to find how many bytes of UTF-8 this UCS-2 code-point needs. Because UCS-2 can only encode
         * the Basic Multilingual Plane, a maximum of three bytes are needed.
         */
        if (0x000..0x0080).contains(ch) {
            output(&[*ch as u8])?;

            written += 1;
        } else if (0x0080..0x0800).contains(ch) {
            let first = 0b1100_0000 + ch.get_bits(6..11) as u8;
            let last = 0b1000_0000 + ch.get_bits(0..6) as u8;

            output(&[first, last])?;

            written += 2;
        } else {
            let first = 0b1110_0000 + ch.get_bits(12..16) as u8;
            let mid = 0b1000_0000 + ch.get_bits(6..12) as u8;
            let last = 0b1000_0000 + ch.get_bits(0..6) as u8;

            output(&[first, mid, last])?;

            written += 3;
        }
    }

    Ok(written)
}

/// Decode an input UCS-2 string into a UTF-8 string.
///
/// The returned `usize` represents the length of the returned buffer,
/// in bytes.
pub fn decode(input: &[u16], output: &mut [u8]) -> Result<usize> {
    let buffer_size = output.len();
    let mut i = 0;

    decode_with(input, |bytes| {
        if bytes.len() == 1 {
            // Can be encoded in a single byte
            if i >= buffer_size {
                return Err(Error::BufferOverflow);
            }

            output[i] = bytes[0];

            i += 1;
        } else if bytes.len() == 2 {
            // Can be encoded two bytes
            if i + 1 >= buffer_size {
                return Err(Error::BufferOverflow);
            }

            output[i] = bytes[0];
            output[i + 1] = bytes[1];

            i += 2;
        } else if bytes.len() == 3 {
            // Can be encoded three bytes
            if i + 2 >= buffer_size {
                return Err(Error::BufferOverflow);
            }

            output[i] = bytes[0];
            output[i + 1] = bytes[1];
            output[i + 2] = bytes[2];

            i += 3;
        } else {
            unreachable!("More than three bytes per UCS-2 character.");
        }

        Ok(())
    })
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

    #[test]
    fn decoding() {
        let input = "$¢ह한";
        let mut u16_buffer = [0u16; 4];
        let result = encode(input, &mut u16_buffer);
        assert_eq!(result.unwrap(), 4);

        let mut u8_buffer = [0u8; 9];
        let result = decode(&u16_buffer, &mut u8_buffer);
        assert_eq!(result.unwrap(), 9);
        assert_eq!(core::str::from_utf8(&u8_buffer[0..9]), Ok("$¢ह한"));
    }

    #[test]
    fn decoding_with() {
        let input = "$¢ह한";

        let mut u16_buffer = [0u16; 4];
        let result = encode(input, &mut u16_buffer);
        assert_eq!(result.unwrap(), 4);

        let mut u8_buffer = [0u8; 9];
        let mut pos = 0;

        let result = decode_with(&u16_buffer, |bytes| {
            for byte in bytes.into_iter() {
                u8_buffer[pos] = *byte;
                pos += 1;
            }

            Ok(())
        });

        assert_eq!(result.unwrap(), 9);
        assert_eq!(core::str::from_utf8(&u8_buffer[0..9]), Ok("$¢ह한"));
    }
}

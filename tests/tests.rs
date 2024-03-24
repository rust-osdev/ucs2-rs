use ucs2::{decode, decode_with, encode, Error};

#[test]
fn encoding() {
    let input = "őэ╋";
    let mut buffer = [0u16; 3];

    assert_eq!(encode(input, &mut buffer), Ok(3));
    assert_eq!(buffer[..], [0x0151, 0x044D, 0x254B]);

    let mut buffer = [0u16; 2];
    assert_eq!(encode(input, &mut buffer), Err(Error::BufferOverflow));

    let input = "😎";
    assert_eq!(encode(input, &mut buffer), Err(Error::MultiByte));
}

#[test]
fn decoding() {
    let input = "$¢ह한";
    let mut u16_buffer = [0u16; 4];
    assert_eq!(encode(input, &mut u16_buffer), Ok(4));

    let mut u8_buffer = [0u8; 9];
    assert_eq!(decode(&u16_buffer, &mut u8_buffer), Ok(9));
    assert_eq!(core::str::from_utf8(&u8_buffer[0..9]), Ok("$¢ह한"));

    // `decode` has three branches that can return `BufferOverflow`,
    // check each of them.
    assert_eq!(
        decode(&u16_buffer, &mut u8_buffer[..0]),
        Err(Error::BufferOverflow)
    );
    assert_eq!(
        decode(&u16_buffer, &mut u8_buffer[..1]),
        Err(Error::BufferOverflow)
    );
    assert_eq!(
        decode(&u16_buffer, &mut u8_buffer[..3]),
        Err(Error::BufferOverflow)
    );
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

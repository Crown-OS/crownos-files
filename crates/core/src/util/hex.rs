const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

pub fn to_lower_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .flat_map(|&byte| {
            [
                HEX_DIGITS[usize::from(byte >> 4)],
                HEX_DIGITS[usize::from(byte & 0xF)],
            ]
        })
        .map(char::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_become_lowercase_pairs() {
        assert_eq!(to_lower_hex(&[0x00, 0xAB, 0x5F]), "00ab5f");
    }
}

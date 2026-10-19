//! On-prem access code ("Zugangscode"): random 128 bit, hex encoded.

/// Fresh access code: 16 random bytes from the OS RNG as 32 lowercase hex
/// characters (URL-safe, no encoding needed in the QR link).
pub fn generate_token() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|e| format!("Zufallszahlen nicht verfügbar: {e}"))?;
    Ok(hex(&bytes))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[usize::from(b >> 4)] as char);
        out.push(DIGITS[usize::from(b & 0x0f)] as char);
    }
    out
}

/// Shape check of a stored code (32 lowercase hex chars).
pub fn is_valid_token(token: &str) -> bool {
    token.len() == 32
        && token
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn token_is_128_bit_hex() {
        let t = generate_token().unwrap();
        assert_eq!(t.len(), 32);
        assert!(is_valid_token(&t));
    }

    #[test]
    fn tokens_are_unique() {
        let set: HashSet<String> = (0..200).map(|_| generate_token().unwrap()).collect();
        assert_eq!(set.len(), 200);
    }

    #[test]
    fn hex_encoding() {
        assert_eq!(hex(&[0x00, 0x0f, 0xa5, 0xff]), "000fa5ff");
    }

    #[test]
    fn validity() {
        assert!(!is_valid_token(""));
        assert!(!is_valid_token("0123456789ABCDEF0123456789abcdef"));
        assert!(!is_valid_token("0123456789abcdef0123456789abcde"));
        assert!(!is_valid_token("0123456789abcdef0123456789abcdeg"));
        assert!(is_valid_token("0123456789abcdef0123456789abcdef"));
    }
}

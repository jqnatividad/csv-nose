//! UTF-8 encoding detection using simdutf8.

use simdutf8::basic::from_utf8;

/// Check if the given bytes are valid UTF-8.
///
/// Uses SIMD-accelerated validation for performance.
pub fn is_utf8(data: &[u8]) -> bool {
    from_utf8(data).is_ok()
}

/// Check if the data starts with a UTF-8 BOM (Byte Order Mark).
///
/// The UTF-8 BOM is the byte sequence: EF BB BF
pub fn has_utf8_bom(data: &[u8]) -> bool {
    data.len() >= 3 && data[0] == 0xEF && data[1] == 0xBB && data[2] == 0xBF
}

/// Skip the UTF-8 BOM if present and return the remaining data.
pub fn skip_bom(data: &[u8]) -> &[u8] {
    if has_utf8_bom(data) {
        &data[3..]
    } else {
        data
    }
}

/// Detect the encoding of the data.
///
/// Currently only supports UTF-8 detection. Returns true if valid UTF-8.
pub fn detect_encoding(data: &[u8]) -> EncodingInfo {
    let has_bom = has_utf8_bom(data);
    let data_without_bom = skip_bom(data);
    let valid_utf8 = is_utf8(data_without_bom);

    EncodingInfo {
        is_utf8: valid_utf8,
        has_bom,
    }
}

/// Information about the detected encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodingInfo {
    /// Whether the data is valid UTF-8.
    pub is_utf8: bool,
    /// Whether a UTF-8 BOM was present.
    pub has_bom: bool,
}

impl EncodingInfo {
    /// Create a new EncodingInfo.
    pub fn new(is_utf8: bool, has_bom: bool) -> Self {
        Self { is_utf8, has_bom }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_utf8() {
        assert!(is_utf8(b"Hello, World!"));
        assert!(is_utf8("こんにちは".as_bytes()));
        assert!(is_utf8(b""));
    }

    #[test]
    fn test_invalid_utf8() {
        // Invalid UTF-8 sequence
        assert!(!is_utf8(&[0xFF, 0xFE]));
        assert!(!is_utf8(&[0x80, 0x81, 0x82]));
    }

    #[test]
    fn test_utf8_bom() {
        let with_bom = [0xEF, 0xBB, 0xBF, b'a', b'b', b'c'];
        let without_bom = b"abc";

        assert!(has_utf8_bom(&with_bom));
        assert!(!has_utf8_bom(without_bom));

        assert_eq!(skip_bom(&with_bom), b"abc");
        assert_eq!(skip_bom(without_bom), b"abc");
    }

    #[test]
    fn test_detect_encoding() {
        let info = detect_encoding(b"Hello");
        assert!(info.is_utf8);
        assert!(!info.has_bom);

        let with_bom = [0xEF, 0xBB, 0xBF, b'H', b'i'];
        let info = detect_encoding(&with_bom);
        assert!(info.is_utf8);
        assert!(info.has_bom);
    }
}

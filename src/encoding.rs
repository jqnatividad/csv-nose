//! Encoding detection and transcoding using chardetng and `encoding_rs`.

use chardetng::EncodingDetector;
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
    if has_utf8_bom(data) { &data[3..] } else { data }
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
    /// Create a new `EncodingInfo`.
    pub const fn new(is_utf8: bool, has_bom: bool) -> Self {
        Self { is_utf8, has_bom }
    }
}

/// Detect the encoding of data and transcode to UTF-8 if necessary.
///
/// Uses chardetng for robust encoding detection supporting:
/// - Windows-1251 (Cyrillic)
/// - Windows-1250 (Central European)
/// - ISO-8859 variants
/// - GB2312/GBK (Chinese)
/// - UTF-16 LE/BE
/// - And many more
///
/// Returns (`transcoded_data`, `was_transcoded`). If `was_transcoded` is false,
/// the original data is returned as-is (it was already valid UTF-8).
pub fn detect_and_transcode(data: &[u8]) -> (std::borrow::Cow<'_, [u8]>, bool) {
    // Check for UTF-16 BOM first (chardetng doesn't handle these well)
    if data.len() >= 2 {
        // UTF-16 LE BOM: FF FE
        if data[0] == 0xFF && data[1] == 0xFE {
            let (decoded, _, _) = encoding_rs::UTF_16LE.decode(data);
            return (
                std::borrow::Cow::Owned(decoded.into_owned().into_bytes()),
                true,
            );
        }
        // UTF-16 BE BOM: FE FF
        if data[0] == 0xFE && data[1] == 0xFF {
            let (decoded, _, _) = encoding_rs::UTF_16BE.decode(data);
            return (
                std::borrow::Cow::Owned(decoded.into_owned().into_bytes()),
                true,
            );
        }
    }

    // Check if already valid UTF-8
    if is_utf8(data) {
        return (std::borrow::Cow::Borrowed(data), false);
    }

    // Use chardetng to detect encoding
    let mut detector = EncodingDetector::new();
    detector.feed(data, true);
    let encoding = detector.guess(None, true);

    // If detected as UTF-8, return as-is (might have some invalid bytes)
    if encoding == encoding_rs::UTF_8 {
        return (std::borrow::Cow::Borrowed(data), false);
    }

    // Transcode to UTF-8
    let (decoded, _, _) = encoding.decode(data);
    (
        std::borrow::Cow::Owned(decoded.into_owned().into_bytes()),
        true,
    )
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

    #[test]
    fn test_detect_and_transcode_utf8() {
        // Valid UTF-8 should not be transcoded
        let data = b"Hello, World!";
        let (result, was_transcoded) = detect_and_transcode(data);
        assert!(!was_transcoded);
        assert_eq!(&result[..], data);
    }

    #[test]
    fn test_detect_and_transcode_utf16_le() {
        // UTF-16 LE with BOM: "Hi"
        let data: &[u8] = &[0xFF, 0xFE, b'H', 0x00, b'i', 0x00];
        let (result, was_transcoded) = detect_and_transcode(data);
        assert!(was_transcoded);
        // Result should be UTF-8 (without BOM marker in content)
        assert!(is_utf8(&result));
    }

    #[test]
    fn test_detect_and_transcode_windows1251() {
        // Windows-1251 encoded Cyrillic text: "Привет" (Hello in Russian)
        // П=0xCF, р=0xF0, и=0xE8, в=0xE2, е=0xE5, т=0xF2
        let data: &[u8] = &[0xCF, 0xF0, 0xE8, 0xE2, 0xE5, 0xF2];
        let (result, was_transcoded) = detect_and_transcode(data);
        // Should be transcoded since it's not valid UTF-8
        assert!(was_transcoded);
        // Result should be valid UTF-8
        assert!(is_utf8(&result));
    }
}

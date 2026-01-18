//! csv-nose: CSV dialect sniffer using the Table Uniformity Method
//!
//! A drop-in replacement for qsv-sniffer with improved dialect detection accuracy
//! using the Table Uniformity Method from the CSVsniffer paper.
//!
//! # Quick Start
//!
//! ```no_run
//! use csv_nose::{Sniffer, SampleSize};
//!
//! // Create a sniffer with default settings
//! let mut sniffer = Sniffer::new();
//!
//! // Optionally configure sampling
//! sniffer.sample_size(SampleSize::Records(100));
//!
//! // Sniff a file
//! let metadata = sniffer.sniff_path("data.csv").unwrap();
//!
//! println!("Delimiter: {}", metadata.dialect.delimiter as char);
//! println!("Has header: {}", metadata.dialect.header.has_header_row);
//! println!("Fields: {:?}", metadata.fields);
//! println!("Types: {:?}", metadata.types);
//! ```
//!
//! # API Compatibility
//!
//! This crate provides API compatibility with qsv-sniffer, making it easy to
//! switch between implementations:
//!
//! ```no_run
//! use csv_nose::{Sniffer, Metadata, Dialect, Header, Quote, Type, SampleSize, DatePreference};
//!
//! let mut sniffer = Sniffer::new();
//! sniffer
//!     .sample_size(SampleSize::Records(50))
//!     .date_preference(DatePreference::MdyFormat)
//!     .delimiter(b',')
//!     .quote(Quote::Some(b'"'));
//! ```
//!
//! # The Table Uniformity Method
//!
//! This library implements the Table Uniformity Method from:
//! "Wrangling Messy CSV Files by Detecting Row and Type Patterns"
//! by van den Burg, Nazábal, and Sutton (2019).
//!
//! The algorithm achieves ~93% accuracy on real-world messy CSV files by:
//! 1. Testing multiple potential dialects (delimiter, quote, line terminator combinations)
//! 2. Scoring each dialect based on table uniformity (consistent field counts)
//! 3. Scoring based on type detection (consistent data types within columns)
//! 4. Selecting the dialect with the highest combined score

pub mod benchmark;
mod encoding;
mod error;
mod field_type;
mod metadata;
mod sample;
mod sniffer;
mod tum;

// Re-export public API (qsv-sniffer compatible)
pub use error::{Result, SnifferError};
pub use field_type::Type;
pub use metadata::{Dialect, Header, Metadata, Quote};
pub use sample::{DatePreference, SampleSize};
pub use sniffer::Sniffer;

// Re-export for advanced usage
pub use encoding::{detect_encoding, is_utf8, EncodingInfo};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_public_api() {
        // Verify all public types are accessible
        let _sniffer = Sniffer::new();
        let _sample = SampleSize::Records(100);
        let _date_pref = DatePreference::MdyFormat;
        let _quote = Quote::Some(b'"');
        let _type = Type::Text;
    }

    #[test]
    fn test_sniff_simple_csv() {
        let data = b"a,b,c\n1,2,3\n4,5,6\n";
        let sniffer = Sniffer::new();

        let metadata = sniffer.sniff_bytes(data).unwrap();

        assert_eq!(metadata.dialect.delimiter, b',');
        assert_eq!(metadata.num_fields, 3);
    }

    #[test]
    fn test_builder_pattern() {
        let mut sniffer = Sniffer::new();
        sniffer
            .sample_size(SampleSize::Bytes(4096))
            .date_preference(DatePreference::DmyFormat)
            .delimiter(b';')
            .quote(Quote::None);

        // Verify builder returns &mut Self for chaining
    }
}

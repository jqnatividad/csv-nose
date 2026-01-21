//! Integration tests for csv-nose

use csv_nose::{DatePreference, Quote, SampleSize, Sniffer, Type};
use std::io::Cursor;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_sniff_comma_delimited() {
    let data = b"name,age,city\nAlice,30,New York\nBob,25,Los Angeles\nCharlie,35,Chicago\n";
    let sniffer = Sniffer::new();

    let metadata = sniffer.sniff_bytes(data).unwrap();

    assert_eq!(metadata.dialect.delimiter, b',');
    assert!(metadata.dialect.header.has_header_row);
    assert_eq!(metadata.num_fields, 3);
    assert_eq!(metadata.fields, vec!["name", "age", "city"]);
}

#[test]
fn test_sniff_tab_delimited() {
    let data = b"name\tage\tcity\nAlice\t30\tNew York\nBob\t25\tLos Angeles\n";
    let sniffer = Sniffer::new();

    let metadata = sniffer.sniff_bytes(data).unwrap();

    assert_eq!(metadata.dialect.delimiter, b'\t');
    assert!(metadata.dialect.header.has_header_row);
    assert_eq!(metadata.num_fields, 3);
}

#[test]
fn test_sniff_semicolon_delimited() {
    let data = b"name;age;city\nAlice;30;New York\nBob;25;Los Angeles\n";
    let sniffer = Sniffer::new();

    let metadata = sniffer.sniff_bytes(data).unwrap();

    assert_eq!(metadata.dialect.delimiter, b';');
}

#[test]
fn test_sniff_pipe_delimited() {
    let data = b"name|age|city\nAlice|30|New York\nBob|25|Los Angeles\n";
    let sniffer = Sniffer::new();

    let metadata = sniffer.sniff_bytes(data).unwrap();

    assert_eq!(metadata.dialect.delimiter, b'|');
}

#[test]
fn test_sniff_quoted_fields() {
    let data = b"\"name\",\"value\"\n\"hello, world\",\"123\"\n\"test\",\"456\"\n";
    let sniffer = Sniffer::new();

    let metadata = sniffer.sniff_bytes(data).unwrap();

    assert_eq!(metadata.dialect.delimiter, b',');
    assert_eq!(metadata.dialect.quote, Quote::Some(b'"'));
}

#[test]
fn test_sniff_single_quoted() {
    let data = b"'name','value'\n'hello, world','123'\n'test','456'\n";
    let sniffer = Sniffer::new();

    let metadata = sniffer.sniff_bytes(data).unwrap();

    assert_eq!(metadata.dialect.delimiter, b',');
    assert_eq!(metadata.dialect.quote, Quote::Some(b'\''));
}

#[test]
fn test_sniff_no_header() {
    let data = b"1,2,3\n4,5,6\n7,8,9\n10,11,12\n";
    let sniffer = Sniffer::new();

    let metadata = sniffer.sniff_bytes(data).unwrap();

    assert_eq!(metadata.dialect.delimiter, b',');
    assert!(!metadata.dialect.header.has_header_row);
    assert_eq!(metadata.num_fields, 3);
    // Should have generated field names
    assert_eq!(metadata.fields, vec!["field_1", "field_2", "field_3"]);
}

#[test]
fn test_sniff_type_detection() {
    let data =
        b"id,name,score,active,date\n1,Alice,95.5,true,2023-01-15\n2,Bob,87.2,false,2023-02-20\n";
    let sniffer = Sniffer::new();

    let metadata = sniffer.sniff_bytes(data).unwrap();

    assert_eq!(metadata.types.len(), 5);
    assert_eq!(metadata.types[0], Type::Unsigned); // id
    assert_eq!(metadata.types[1], Type::Text); // name
    assert_eq!(metadata.types[2], Type::Float); // score
    assert_eq!(metadata.types[3], Type::Boolean); // active
    assert_eq!(metadata.types[4], Type::Date); // date
}

#[test]
fn test_sniff_windows_line_endings() {
    let data = b"name,age\r\nAlice,30\r\nBob,25\r\n";
    let sniffer = Sniffer::new();

    let metadata = sniffer.sniff_bytes(data).unwrap();

    assert_eq!(metadata.dialect.delimiter, b',');
    assert_eq!(metadata.num_fields, 2);
}

#[test]
fn test_sniff_from_reader() {
    let data = b"a,b,c\n1,2,3\n4,5,6\n";
    let cursor = Cursor::new(data.to_vec());

    let mut sniffer = Sniffer::new();
    let metadata = sniffer.sniff_reader(cursor).unwrap();

    assert_eq!(metadata.dialect.delimiter, b',');
    assert_eq!(metadata.num_fields, 3);
}

#[test]
fn test_sniff_from_file() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(temp_file, "name,age,city").unwrap();
    writeln!(temp_file, "Alice,30,NYC").unwrap();
    writeln!(temp_file, "Bob,25,LA").unwrap();
    temp_file.flush().unwrap();

    let mut sniffer = Sniffer::new();
    let metadata = sniffer.sniff_path(temp_file.path()).unwrap();

    assert_eq!(metadata.dialect.delimiter, b',');
    assert_eq!(metadata.num_fields, 3);
    assert!(metadata.dialect.header.has_header_row);
}

#[test]
fn test_forced_delimiter() {
    // Data that could be interpreted as comma or semicolon
    let data = b"a;b;c\n1;2;3\n";

    let mut sniffer = Sniffer::new();
    sniffer.delimiter(b';');

    let metadata = sniffer.sniff_bytes(data).unwrap();

    assert_eq!(metadata.dialect.delimiter, b';');
}

#[test]
fn test_sample_size_records() {
    let data = b"a,b\n1,2\n3,4\n5,6\n7,8\n9,10\n";

    let mut sniffer = Sniffer::new();
    sniffer.sample_size(SampleSize::Records(3));

    let metadata = sniffer.sniff_bytes(data).unwrap();

    assert_eq!(metadata.dialect.delimiter, b',');
}

#[test]
fn test_sample_size_bytes() {
    let data = b"name,age\nAlice,30\nBob,25\nCharlie,35\n";

    let mut sniffer = Sniffer::new();
    sniffer.sample_size(SampleSize::Bytes(50));

    let metadata = sniffer.sniff_bytes(data).unwrap();

    assert_eq!(metadata.dialect.delimiter, b',');
}

#[test]
fn test_utf8_detection() {
    let data = "name,city\nAlice,东京\nBob,Москва\n".as_bytes();
    let sniffer = Sniffer::new();

    let metadata = sniffer.sniff_bytes(data).unwrap();

    assert!(metadata.dialect.is_utf8);
}

#[test]
fn test_utf8_bom() {
    let mut data = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
    data.extend_from_slice(b"a,b,c\n1,2,3\n");

    let sniffer = Sniffer::new();
    let metadata = sniffer.sniff_bytes(&data).unwrap();

    assert_eq!(metadata.dialect.delimiter, b',');
    assert!(metadata.dialect.is_utf8);
}

#[test]
fn test_empty_file_error() {
    let data = b"";
    let sniffer = Sniffer::new();

    let result = sniffer.sniff_bytes(data);

    assert!(result.is_err());
}

#[test]
fn test_flexible_field_counts() {
    let data = b"a,b,c\n1,2\n3,4,5,6\n7,8,9\n";
    let sniffer = Sniffer::new();

    let metadata = sniffer.sniff_bytes(data).unwrap();

    // Should detect as flexible due to varying field counts
    assert!(metadata.dialect.flexible);
}

#[test]
fn test_date_types() {
    let data = b"iso_date,us_date,euro_date\n2023-12-31,12/31/2023,31.12.2023\n2024-01-15,01/15/2024,15.01.2024\n";
    let sniffer = Sniffer::new();

    let metadata = sniffer.sniff_bytes(data).unwrap();

    // All three columns should be detected as Date
    for typ in &metadata.types {
        assert_eq!(*typ, Type::Date);
    }
}

#[test]
fn test_datetime_types() {
    let data = b"timestamp\n2023-12-31T12:30:45\n2024-01-15T08:00:00Z\n";
    let sniffer = Sniffer::new();

    let metadata = sniffer.sniff_bytes(data).unwrap();

    assert_eq!(metadata.types[0], Type::DateTime);
}

#[test]
fn test_null_values() {
    let data = b"id,value\n1,100\n2,\n3,NULL\n4,N/A\n";
    let sniffer = Sniffer::new();

    let metadata = sniffer.sniff_bytes(data).unwrap();

    // First column should still be detected as Unsigned despite some null values
    assert_eq!(metadata.types[0], Type::Unsigned);
}

#[test]
fn test_builder_chaining() {
    let mut sniffer = Sniffer::new();
    let sniffer_ref = sniffer
        .sample_size(SampleSize::Records(50))
        .date_preference(DatePreference::DmyFormat)
        .delimiter(b',')
        .quote(Quote::Some(b'"'));

    // Verify chaining works
    let data = b"a,b\n1,2\n";
    let _ = sniffer_ref.sniff_bytes(data);
}

#[test]
fn test_many_columns() {
    // Generate CSV with many columns
    let header: Vec<String> = (0..50).map(|i| format!("col{}", i)).collect();
    let row: Vec<String> = (0..50).map(|i| format!("{}", i)).collect();

    let mut data = header.join(",");
    data.push('\n');
    data.push_str(&row.join(","));
    data.push('\n');

    let sniffer = Sniffer::new();
    let metadata = sniffer.sniff_bytes(data.as_bytes()).unwrap();

    assert_eq!(metadata.num_fields, 50);
    assert_eq!(metadata.dialect.delimiter, b',');
}

#[test]
fn test_single_column() {
    let data = b"value\n100\n200\n300\n";
    let sniffer = Sniffer::new();

    let metadata = sniffer.sniff_bytes(data).unwrap();

    assert_eq!(metadata.num_fields, 1);
}

#[test]
fn test_mixed_types_column() {
    // Column with mixed types should become Text
    let data = b"value\n100\nhello\n300\n";
    let sniffer = Sniffer::new();

    let metadata = sniffer.sniff_bytes(data).unwrap();

    assert_eq!(metadata.types[0], Type::Text);
}

/// Regression test for avg_record_len calculation with SampleSize::Records.
///
/// Previously, when using SampleSize::Records(n), the avg_record_len was always
/// ~1024 bytes because the buffer size estimate (n * 1024) was divided by the
/// parsed row count (n), regardless of actual record size.
///
/// This test uses a real-world CSV sample (NYC 311 data) to verify that
/// avg_record_len reflects the actual average record size (~530 bytes),
/// not the buffer estimate constant (1024 bytes).
#[test]
fn test_avg_record_len_regression_nyc_311() {
    let fixture_path = std::path::Path::new("tests/data/fixtures/nyc_311_sample_200.csv");

    // Skip test if fixture file doesn't exist
    if !fixture_path.exists() {
        eprintln!("Skipping test: fixture file not found at {:?}", fixture_path);
        return;
    }

    let mut sniffer = Sniffer::new();
    // Use default SampleSize::Records(100) which triggered the bug
    sniffer.sample_size(SampleSize::Records(100));

    let metadata = sniffer.sniff_path(fixture_path).unwrap();

    // Verify basic detection
    assert_eq!(metadata.dialect.delimiter, b',');
    assert_eq!(metadata.dialect.quote, Quote::Some(b'"'));
    assert!(metadata.dialect.header.has_header_row);
    assert_eq!(metadata.num_fields, 41);

    // THE KEY REGRESSION TEST:
    // avg_record_len should be approximately 530 bytes (actual record size),
    // NOT 1024 bytes (the old buggy value from buffer_size / row_count)
    assert!(
        metadata.avg_record_len < 700,
        "avg_record_len should be ~530 bytes, not 1024. Got: {} bytes",
        metadata.avg_record_len
    );
    assert!(
        metadata.avg_record_len > 400,
        "avg_record_len should be ~530 bytes, not too small. Got: {} bytes",
        metadata.avg_record_len
    );
}

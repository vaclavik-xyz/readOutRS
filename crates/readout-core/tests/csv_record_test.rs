use readout_core::csv_record::{find_mode_changes, parse_csv_file};
use std::io::Cursor;

fn parse(csv: &str) -> Vec<readout_core::csv_record::CsvRecord> {
    parse_csv_file(Cursor::new(csv.as_bytes())).unwrap()
}

#[test]
fn parse_single_row() {
    let records = parse(
        "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n\
2026-03-29T10:30:00.123,Multimeter,12.345,V DC,DCV,false,false,false\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].timestamp, "2026-03-29T10:30:00.123");
    assert_eq!(records[0].device, "Multimeter");
    assert_eq!(records[0].value, Some(12.345));
    assert_eq!(records[0].unit, "V DC");
    assert_eq!(records[0].mode, "DCV");
    assert!(!records[0].is_overload);
    assert!(!records[0].is_open);
    assert!(!records[0].is_short);
}

#[test]
fn parse_multiple_rows() {
    let records = parse(
        "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n\
2026-03-29T10:30:00,Multimeter,12.345,V DC,DCV,false,false,false\n\
2026-03-29T10:30:01,Multimeter,12.346,V DC,DCV,false,false,false\n",
    );

    assert_eq!(records.len(), 2);
    assert_eq!(records[0].value, Some(12.345));
    assert_eq!(records[1].value, Some(12.346));
}

#[test]
fn parse_handles_mode_change() {
    let records = parse(
        "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n\
2026-03-29T10:30:00,Multimeter,12.345,V DC,DCV,false,false,false\n\
2026-03-29T10:30:01,Multimeter,0.001,A DC,DCA,false,false,false\n",
    );

    assert_eq!(records[0].mode, "DCV");
    assert_eq!(records[1].mode, "DCA");
}

#[test]
fn parse_skips_malformed_rows() {
    let records = parse(
        "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n\
bad_row\n\
2026-03-29T10:30:00,Multimeter,12.345,V DC,DCV,false,false,false\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].mode, "DCV");
}

#[test]
fn parse_empty_file_returns_empty() {
    let records = parse("timestamp,device,value,unit,mode,is_overload,is_open,is_short\n");

    assert!(records.is_empty());
}

#[test]
fn mode_change_indices_detected() {
    let records = parse(
        "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n\
2026-03-29T10:30:00,Multimeter,12.345,V DC,DCV,false,false,false\n\
2026-03-29T10:30:01,Multimeter,12.346,V DC,DCV,false,false,false\n\
2026-03-29T10:30:02,Multimeter,0.001,A DC,DCA,false,false,false\n",
    );

    assert_eq!(find_mode_changes(&records), vec![2]);
}

#[test]
fn parse_preserves_overload_row() {
    let records = parse(
        "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n\
2026-03-29T10:30:00.123,Multimeter,OL,V DC,DCV,true,false,false\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].timestamp, "2026-03-29T10:30:00.123");
    assert_eq!(records[0].device, "Multimeter");
    assert_eq!(records[0].value, None);
    assert_eq!(records[0].unit, "V DC");
    assert_eq!(records[0].mode, "DCV");
    assert!(records[0].is_overload);
    assert!(!records[0].is_open);
    assert!(!records[0].is_short);
}

#[test]
fn parse_skips_invalid_numeric_token() {
    let records = parse(
        "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n\
2026-03-29T10:30:00.123,Multimeter,abc,V DC,DCV,false,false,false\n\
2026-03-29T10:30:01.123,Multimeter,12.345,V DC,DCV,false,false,false\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].timestamp, "2026-03-29T10:30:01.123");
    assert_eq!(records[0].value, Some(12.345));
}

#[test]
fn parse_skips_invalid_bool_token() {
    let records = parse(
        "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n\
2026-03-29T10:30:00.123,Multimeter,12.345,V DC,DCV,maybe,false,false\n\
2026-03-29T10:30:01.123,Multimeter,12.345,V DC,DCV,false,false,false\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].timestamp, "2026-03-29T10:30:01.123");
    assert_eq!(records[0].is_overload, false);
    assert_eq!(records[0].value, Some(12.345));
}

#[test]
fn parse_rejects_non_exact_ol_token() {
    let records = parse(
        "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n\
2026-03-29T10:30:00.123,Multimeter,ol,V DC,DCV,true,false,false\n\
2026-03-29T10:30:00.223,Multimeter,Ol,V DC,DCV,true,false,false\n\
2026-03-29T10:30:00.323,Multimeter,oL,V DC,DCV,true,false,false\n\
2026-03-29T10:30:01.123,Multimeter,OL,V DC,DCV,true,false,false\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].timestamp, "2026-03-29T10:30:01.123");
    assert_eq!(records[0].value, None);
    assert!(records[0].is_overload);
}

#[test]
fn parse_rejects_whitespace_padded_ol_token() {
    let records = parse(
        "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n\
2026-03-29T10:30:00.123,Multimeter, OL ,V DC,DCV,true,false,false\n\
2026-03-29T10:30:01.123,Multimeter,OL\t,V DC,DCV,true,false,false\n\
2026-03-29T10:30:02.123,Multimeter,OL,V DC,DCV,true,false,false\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].timestamp, "2026-03-29T10:30:02.123");
    assert_eq!(records[0].value, None);
    assert!(records[0].is_overload);
}

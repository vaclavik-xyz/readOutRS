use chrono::DateTime;
use std::io::BufRead;

#[derive(Debug, Clone, PartialEq)]
pub struct CsvRecord {
    pub timestamp: String,
    pub parsed_time: Option<f64>,
    pub device: String,
    pub value: Option<f64>,
    pub unit: String,
    pub mode: String,
    pub is_overload: bool,
    pub is_open: bool,
    pub is_short: bool,
}

pub fn parse_csv_file(reader: impl BufRead) -> Result<Vec<CsvRecord>, std::io::Error> {
    let mut records = Vec::new();
    let mut lines = reader.lines();

    if lines.next().is_none() {
        return Ok(records);
    }

    for line in lines {
        let line = line?;
        if let Some(record) = parse_row(&line) {
            records.push(record);
        }
    }

    Ok(records)
}

pub fn parse_row(line: &str) -> Option<CsvRecord> {
    let fields: Vec<&str> = line.splitn(8, ',').collect();
    if fields.len() != 8 {
        return None;
    }

    let value = parse_value(fields[2])?;
    let is_overload = parse_bool(fields[5])?;
    let is_open = parse_bool(fields[6])?;
    let is_short = parse_bool(fields[7])?;

    Some(CsvRecord {
        timestamp: fields[0].to_string(),
        parsed_time: parse_timestamp(fields[0]),
        device: fields[1].to_string(),
        value,
        unit: fields[3].to_string(),
        mode: fields[4].to_string(),
        is_overload,
        is_open,
        is_short,
    })
}

pub fn find_mode_changes(records: &[CsvRecord]) -> Vec<usize> {
    let mut changes = Vec::new();
    for i in 1..records.len() {
        if records[i].mode != records[i - 1].mode {
            changes.push(i);
        }
    }
    changes
}

fn parse_value(raw: &str) -> Option<Option<f64>> {
    if raw == "OL" {
        return Some(None);
    }
    let trimmed = raw.trim();
    trimmed.parse().ok().map(Some)
}

fn parse_bool(raw: &str) -> Option<bool> {
    raw.trim().parse().ok()
}

fn parse_timestamp(raw: &str) -> Option<f64> {
    let timestamp = DateTime::parse_from_rfc3339(raw).ok()?;
    Some(timestamp.timestamp() as f64 + f64::from(timestamp.timestamp_subsec_nanos()) / 1e9)
}

use readout_core::csv_record::{find_mode_changes, parse_csv_file, parse_row, CsvRecord};
use readout_core::downsampling::{min_max_downsample, DataPoint};
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::Duration;

pub type FileId = usize;

const COLORS: &[[u8; 3]] = &[
    [74, 158, 255],
    [78, 205, 196],
    [255, 107, 107],
    [255, 217, 61],
    [168, 120, 255],
];

pub struct LoadedFile {
    pub path: PathBuf,
    pub records: Vec<CsvRecord>,
    pub mode_changes: Vec<usize>,
    pub visible: bool,
    pub color: egui::Color32,
    pub modes: Vec<String>,
    pub visible_modes: HashSet<String>,
    pub last_read_pos: u64,
    pub is_live: bool,
}

pub struct CsvDataStore {
    files: Vec<LoadedFile>,
    next_color_idx: usize,
}

impl CsvDataStore {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            next_color_idx: 0,
        }
    }

    pub fn load_file(&mut self, path: PathBuf, is_live: bool) -> Result<FileId, std::io::Error> {
        let content = std::fs::read(&path)?;
        let (records, last_read_pos) = parse_initial_records(&content)?;
        let color = next_color(self.next_color_idx);
        self.next_color_idx += 1;

        let loaded_file = LoadedFile {
            path,
            records,
            mode_changes: Vec::new(),
            visible: true,
            color,
            modes: Vec::new(),
            visible_modes: HashSet::new(),
            last_read_pos,
            is_live,
        };

        self.files.push(loaded_file);
        let id = self.files.len() - 1;
        refresh_file_metadata(&mut self.files[id]);

        Ok(id)
    }

    pub fn poll_live_files(&mut self) {
        for file in &mut self.files {
            if !file.is_live {
                continue;
            }

            let Ok(mut handle) = std::fs::File::open(&file.path) else {
                continue;
            };
            let Ok(metadata) = handle.metadata() else {
                continue;
            };
            let file_len = metadata.len();

            if file_len < file.last_read_pos {
                file.last_read_pos = 0;
                file.records.clear();
                file.mode_changes.clear();
                file.modes.clear();
                file.visible_modes.clear();
            }

            if file_len <= file.last_read_pos {
                continue;
            }

            if handle.seek(SeekFrom::Start(file.last_read_pos)).is_err() {
                continue;
            }

            let mut unread = Vec::new();
            if handle.read_to_end(&mut unread).is_err() {
                continue;
            }

            let consumed_len = last_complete_line_offset(&unread);
            if consumed_len == 0 {
                continue;
            }

            let old_len = file.records.len();
            let reader = BufReader::new(&unread[..consumed_len]);
            for line in reader.lines().map_while(Result::ok) {
                if let Some(record) = parse_row(&line) {
                    file.records.push(record);
                }
            }

            if file.records.len() != old_len {
                refresh_file_metadata(file);
            }

            file.last_read_pos += consumed_len as u64;
        }
    }

    pub fn sync_live_paths(&mut self, live_paths: &[PathBuf]) {
        for file in &mut self.files {
            file.is_live = live_paths.iter().any(|live_path| live_path == &file.path);
        }
    }

    pub fn query_points(&self, file_id: FileId, target_points: usize) -> Vec<DataPoint> {
        let Some(file) = self.files.get(file_id) else {
            return Vec::new();
        };

        let points: Vec<DataPoint> = file
            .records
            .iter()
            .enumerate()
            .filter(|(_, record)| file.visible_modes.contains(&record.mode))
            .filter_map(|(idx, record)| {
                record
                    .value
                    .map(|value| (Duration::from_secs(idx as u64), value))
            })
            .collect();

        min_max_downsample(&points, target_points)
    }

    pub fn files(&self) -> &[LoadedFile] {
        &self.files
    }

    #[allow(dead_code)]
    pub fn files_mut(&mut self) -> &mut Vec<LoadedFile> {
        &mut self.files
    }

    #[allow(dead_code)]
    pub fn remove_file(&mut self, file_id: FileId) {
        if file_id < self.files.len() {
            self.files.remove(file_id);
        }
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

fn next_color(idx: usize) -> egui::Color32 {
    let rgb = COLORS[idx % COLORS.len()];
    egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

fn parse_initial_records(content: &[u8]) -> Result<(Vec<CsvRecord>, u64), std::io::Error> {
    let consumed_len = last_complete_line_offset(content);
    if consumed_len == 0 {
        return Ok((Vec::new(), 0));
    }

    let records = parse_csv_file(BufReader::new(&content[..consumed_len]))?;
    Ok((records, consumed_len as u64))
}

fn last_complete_line_offset(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map(|idx| idx + 1)
        .unwrap_or(0)
}

fn refresh_file_metadata(file: &mut LoadedFile) {
    file.mode_changes = find_mode_changes(&file.records);

    let mut modes: Vec<String> = file
        .records
        .iter()
        .map(|record| record.mode.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    modes.sort();
    file.modes = modes.clone();
    let available_modes: HashSet<String> = modes.iter().cloned().collect();

    let selected_modes: HashSet<String> = if file.visible_modes.is_empty() {
        modes.iter().cloned().collect()
    } else {
        file.visible_modes
            .intersection(&available_modes)
            .cloned()
            .collect()
    };

    file.visible_modes = if selected_modes.is_empty() {
        modes.into_iter().collect()
    } else {
        selected_modes
    };
}

#[cfg(test)]
mod tests {
    use super::{CsvDataStore, LoadedFile};
    use readout_core::csv_record::CsvRecord;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn query_points_skips_non_numeric_rows() {
        let mut store = CsvDataStore::new();
        store.files.push(LoadedFile {
            path: PathBuf::from("sample.csv"),
            records: vec![
                CsvRecord {
                    timestamp: "2026-03-29T10:00:00Z".to_string(),
                    device: "Multimeter".to_string(),
                    value: Some(1.25),
                    unit: "V".to_string(),
                    mode: "DCV".to_string(),
                    is_overload: false,
                    is_open: false,
                    is_short: false,
                },
                CsvRecord {
                    timestamp: "2026-03-29T10:00:01Z".to_string(),
                    device: "Multimeter".to_string(),
                    value: None,
                    unit: "V".to_string(),
                    mode: "DCV".to_string(),
                    is_overload: true,
                    is_open: false,
                    is_short: false,
                },
                CsvRecord {
                    timestamp: "2026-03-29T10:00:02Z".to_string(),
                    device: "Multimeter".to_string(),
                    value: Some(2.5),
                    unit: "V".to_string(),
                    mode: "DCV".to_string(),
                    is_overload: false,
                    is_open: false,
                    is_short: false,
                },
            ],
            mode_changes: Vec::new(),
            visible: true,
            color: egui::Color32::WHITE,
            modes: vec!["DCV".to_string()],
            visible_modes: std::iter::once("DCV".to_string()).collect(),
            last_read_pos: 0,
            is_live: false,
        });

        let points = store.query_points(0, 32);

        assert_eq!(
            points,
            vec![
                (Duration::from_secs(0), 1.25),
                (Duration::from_secs(2), 2.5),
            ]
        );
    }

    #[test]
    fn poll_live_files_keeps_incomplete_trailing_row_until_completed() {
        let path = std::env::temp_dir().join(format!(
            "readout_csv_viewer_partial_row_{}_{}.csv",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before epoch")
                .as_nanos()
        ));

        fs::write(
            &path,
            concat!(
                "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n",
                "2026-03-29T10:00:00Z,Multimeter,1.25,V,DCV,false,false,false\n",
                "2026-03-29T10:00:01Z,Multimeter,2.50"
            ),
        )
        .expect("write initial csv");

        let mut store = CsvDataStore::new();
        store
            .load_file(path.clone(), true)
            .expect("load live csv file");

        assert_eq!(store.files()[0].records.len(), 1);
        assert_eq!(store.files()[0].records[0].value, Some(1.25));

        {
            let mut file = OpenOptions::new()
                .append(true)
                .open(&path)
                .expect("open csv for append");
            writeln!(file, ",V,DCV,false,false,false").expect("append row remainder");
        }

        store.poll_live_files();

        assert_eq!(store.files()[0].records.len(), 2);
        assert_eq!(store.files()[0].records[1].value, Some(2.5));

        fs::remove_file(&path).expect("remove temp csv");
    }
}

use super::source_model::{
    SourceStatus, ViewerSample, ViewerSourceId, ViewerSourceKind, XDomain,
};
use super::render_sampling::downsample_visible_points;
use chrono::{TimeZone, Utc};
use readout_core::csv_record::{CsvRecord, find_mode_changes, parse_csv_file, parse_row};
use readout_core::downsampling::DataPoint;
use readout_core::types::{ConnectionState, DeviceId, DeviceMeasurement, RuntimeEvent};
use std::collections::HashSet;
use std::error::Error as StdError;
use std::fmt;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub type FileId = usize;

const COLORS: &[[u8; 3]] = &[
    [74, 158, 255],
    [78, 205, 196],
    [255, 107, 107],
    [255, 217, 61],
    [168, 120, 255],
];

#[derive(Debug, Clone, PartialEq)]
pub struct HoveredRecord {
    pub series: String,
    pub x: f64,
    pub timestamp: String,
    pub value: f64,
    pub unit: String,
    pub mode: String,
}

pub struct RuntimeAnchor {
    pub first_monotonic: std::time::Instant,
    pub first_wall_clock_epoch: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportRow {
    pub timestamp: String,
    pub device: String,
    pub value: String,
    pub unit: String,
    pub mode: String,
    pub is_overload: bool,
    pub is_open: bool,
    pub is_short: bool,
}

#[derive(Debug)]
pub enum CsvViewerError {
    Io(std::io::Error),
    Message(String),
}

impl fmt::Display for CsvViewerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::Message(message) => f.write_str(message),
        }
    }
}

impl StdError for CsvViewerError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Message(_) => None,
        }
    }
}

impl From<std::io::Error> for CsvViewerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

pub struct ViewerSource {
    pub id: ViewerSourceId,
    pub kind: ViewerSourceKind,
    pub path: PathBuf,
    pub x_domain: XDomain,
    pub label: String,
    pub visible: bool,
    pub color: egui::Color32,
    pub status: SourceStatus,
    pub records: Vec<CsvRecord>,
    pub samples: Vec<ViewerSample>,
    pub mode_changes: Vec<usize>,
    pub modes: Vec<String>,
    pub visible_modes: HashSet<String>,
    mode_filter_initialized: bool,
    pub runtime_anchor: Option<RuntimeAnchor>,
    pub last_read_pos: u64,
    pub last_modified: Option<SystemTime>,
    pub is_live: bool,
}

pub struct CsvDataStore {
    sources: Vec<ViewerSource>,
    next_source_id: ViewerSourceId,
    next_color_idx: usize,
    active_domain: Option<XDomain>,
}

impl CsvDataStore {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
            next_source_id: 0,
            next_color_idx: 0,
            active_domain: None,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn load_file(
        &mut self,
        path: PathBuf,
        is_live: bool,
    ) -> Result<ViewerSourceId, CsvViewerError> {
        self.attach_path_source(path, is_live, false)
    }

    pub fn load_csv_file(
        &mut self,
        path: PathBuf,
        replace_existing: bool,
    ) -> Result<ViewerSourceId, CsvViewerError> {
        self.attach_path_source(path, false, replace_existing)
    }

    pub fn attach_runtime_device(
        &mut self,
        device: DeviceId,
    ) -> Result<ViewerSourceId, CsvViewerError> {
        if let Some(existing) = self.sources.iter().find(|source| {
            matches!(
                source.kind,
                ViewerSourceKind::RuntimeDevice { device: existing } if existing == device
            )
        }) {
            return Ok(existing.id);
        }

        self.ensure_compatible_domain(XDomain::WallClock)?;
        self.active_domain.get_or_insert(XDomain::WallClock);

        let id = self.allocate_source_id();
        let color = self.allocate_color();
        self.sources.push(ViewerSource {
            id,
            kind: ViewerSourceKind::RuntimeDevice { device },
            path: PathBuf::new(),
            x_domain: XDomain::WallClock,
            label: runtime_source_label(device),
            visible: true,
            color,
            status: SourceStatus::Waiting("Waiting for samples".to_owned()),
            records: Vec::new(),
            samples: Vec::new(),
            mode_changes: Vec::new(),
            modes: Vec::new(),
            visible_modes: HashSet::new(),
            mode_filter_initialized: false,
            runtime_anchor: None,
            last_read_pos: 0,
            last_modified: None,
            is_live: true,
        });

        Ok(id)
    }

    pub fn attach_live_csv(
        &mut self,
        device: DeviceId,
        path: PathBuf,
    ) -> Result<ViewerSourceId, CsvViewerError> {
        if let Some(existing) = self.sources.iter().find(|source| {
            matches!(
                &source.kind,
                ViewerSourceKind::LiveCsvTail {
                    device: existing_device,
                    path: existing_path,
                } if *existing_device == device && existing_path == &path
            )
        }) {
            return Ok(existing.id);
        }

        let content = std::fs::read(&path)?;
        let (records, last_read_pos) = parse_initial_records(&content)?;
        let x_domain = infer_x_domain(&records);
        self.ensure_compatible_domain(x_domain)?;
        self.active_domain.get_or_insert(x_domain);
        let last_modified = std::fs::metadata(&path)
            .ok()
            .and_then(|metadata| metadata.modified().ok());

        let mut source = ViewerSource {
            id: self.allocate_source_id(),
            kind: ViewerSourceKind::LiveCsvTail {
                device,
                path: path.clone(),
            },
            path,
            x_domain,
            label: source_label(
                &ViewerSourceKind::LiveCsvTail {
                    device,
                    path: PathBuf::new(),
                },
                Path::new(""),
            ),
            visible: true,
            color: self.allocate_color(),
            status: SourceStatus::Ready,
            records,
            samples: Vec::new(),
            mode_changes: Vec::new(),
            modes: Vec::new(),
            visible_modes: HashSet::new(),
            mode_filter_initialized: false,
            runtime_anchor: None,
            last_read_pos,
            last_modified,
            is_live: true,
        };
        refresh_source_metadata(&mut source);
        let id = source.id;
        self.sources.push(source);

        Ok(id)
    }

    pub fn poll_live_sources(&mut self) {
        for source in &mut self.sources {
            if !matches!(source.kind, ViewerSourceKind::LiveCsvTail { .. }) {
                continue;
            }

            poll_path_source(source);
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn poll_live_files(&mut self) {
        for source in &mut self.sources {
            if !source.is_live
                || source.path.as_os_str().is_empty()
                || matches!(source.kind, ViewerSourceKind::RuntimeDevice { .. })
            {
                continue;
            }

            poll_path_source(source);
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn query_points(&self, source_id: ViewerSourceId, target_points: usize) -> Vec<DataPoint> {
        self.query_points_in_view(source_id, None, target_points)
    }

    pub fn query_points_in_view(
        &self,
        source_id: ViewerSourceId,
        x_range: Option<(f64, f64)>,
        target_points: usize,
    ) -> Vec<DataPoint> {
        let Some(source) = self.source_by_id(source_id) else {
            return Vec::new();
        };

        let normalized_x_range = x_range.map(|(start, end)| (start.min(end), start.max(end)));
        let points: Vec<DataPoint> = source
            .samples
            .iter()
            .filter(|sample| source.visible_modes.contains(&sample.mode))
            .filter(|sample| {
                normalized_x_range
                    .map(|(start, end)| sample.x >= start && sample.x <= end)
                    .unwrap_or(true)
            })
            .filter_map(|sample| {
                sample
                    .value
                    .map(|value| (Duration::from_secs_f64(sample.x.max(0.0)), value))
            })
            .collect();

        if points.len() <= target_points {
            points
        } else {
            downsample_visible_points(&points, target_points)
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn sources(&self) -> &[ViewerSource] {
        &self.sources
    }

    pub fn active_domain(&self) -> Option<XDomain> {
        self.active_domain
    }

    pub fn files(&self) -> &[ViewerSource] {
        &self.sources
    }

    pub fn all_modes(&self) -> Vec<String> {
        let mut modes: Vec<String> = self
            .sources
            .iter()
            .flat_map(|source| source.modes.iter().cloned())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        modes.sort();
        modes
    }

    pub fn is_mode_visible(&self, mode: &str) -> bool {
        let mut matched = false;

        for source in &self.sources {
            if source.modes.iter().any(|source_mode| source_mode == mode) {
                matched = true;
                if !source.visible_modes.contains(mode) {
                    return false;
                }
            }
        }

        matched
    }

    pub fn set_mode_visible(&mut self, mode: &str, visible: bool) {
        for source in &mut self.sources {
            if !source.modes.iter().any(|source_mode| source_mode == mode) {
                continue;
            }

            source.mode_filter_initialized = true;

            if visible {
                source.visible_modes.insert(mode.to_owned());
            } else {
                source.visible_modes.remove(mode);
            }
        }
    }

    pub fn visible_values_in_range(&self, x_min: f64, x_max: f64) -> Vec<f64> {
        let x_start = x_min.min(x_max);
        let x_end = x_min.max(x_max);

        self.sources
            .iter()
            .filter(|source| source.visible)
            .flat_map(|source| {
                source
                    .samples
                    .iter()
                    .filter(move |sample| {
                        sample.x >= x_start
                            && sample.x <= x_end
                            && source.visible_modes.contains(&sample.mode)
                    })
                    .filter_map(|sample| sample.value)
            })
            .collect()
    }

    pub fn latest_live_x(&self) -> Option<f64> {
        self.sources
            .iter()
            .filter(|source| source.visible && source.is_live)
            .flat_map(|source| {
                source
                    .samples
                    .iter()
                    .rev()
                    .filter(|sample| source.visible_modes.contains(&sample.mode))
                    .filter_map(|sample| sample.value.map(|_| sample.x))
                    .next()
            })
            .max_by(|left, right| left.total_cmp(right))
    }

    pub fn handle_runtime_event(&mut self, event: &RuntimeEvent) {
        match event {
            RuntimeEvent::Measurement { device, value } => {
                for source in &mut self.sources {
                    if matches!(
                        source.kind,
                        ViewerSourceKind::RuntimeDevice {
                            device: source_device
                        } if source_device == *device
                    ) {
                        push_runtime_sample(source, value);
                    }
                }
            }
            RuntimeEvent::ConnectionChanged { device, state } => {
                for source in &mut self.sources {
                    if matches!(
                        source.kind,
                        ViewerSourceKind::RuntimeDevice {
                            device: source_device
                        } if source_device == *device
                    ) {
                        source.status = runtime_status_for_connection(state, !source.samples.is_empty());
                    }
                }
            }
            RuntimeEvent::Error { device, message } => {
                for source in &mut self.sources {
                    if matches!(
                        source.kind,
                        ViewerSourceKind::RuntimeDevice {
                            device: source_device
                        } if source_device == *device
                    ) {
                        source.status = SourceStatus::Error(message.clone());
                    }
                }
            }
            _ => {}
        }
    }

    pub fn export_rows(&self, selection: Option<(f64, f64)>) -> Vec<ExportRow> {
        let selection = selection.map(|(start, end)| (start.min(end), start.max(end)));
        let mut rows = Vec::new();

        for source in &self.sources {
            if !source.visible {
                continue;
            }

            match source.kind {
                ViewerSourceKind::RuntimeDevice { .. } => {
                    for sample in &source.samples {
                        if !source.visible_modes.contains(&sample.mode) {
                            continue;
                        }
                        if let Some((x_min, x_max)) = selection
                            && (sample.x < x_min || sample.x > x_max)
                        {
                            continue;
                        }

                        rows.push(ExportRow {
                            timestamp: sample.x_label.clone(),
                            device: sample.device.clone(),
                            value: format_export_value(sample.value),
                            unit: sample.unit.clone(),
                            mode: sample.mode.clone(),
                            is_overload: sample.is_overload,
                            is_open: sample.is_open,
                            is_short: sample.is_short,
                        });
                    }
                }
                _ => {
                    for (idx, record) in source.records.iter().enumerate() {
                        if !source.visible_modes.contains(&record.mode) {
                            continue;
                        }

                        let record_x = source.samples.get(idx).map_or_else(
                            || source_x(record, idx, source.x_domain),
                            |sample| sample.x,
                        );
                        if let Some((x_min, x_max)) = selection
                            && (record_x < x_min || record_x > x_max)
                        {
                            continue;
                        }

                        rows.push(ExportRow {
                            timestamp: record.timestamp.clone(),
                            device: record.device.clone(),
                            value: format_export_value(record.value),
                            unit: record.unit.clone(),
                            mode: record.mode.clone(),
                            is_overload: record.is_overload,
                            is_open: record.is_open,
                            is_short: record.is_short,
                        });
                    }
                }
            }
        }

        rows
    }

    pub fn nearest_visible_sample(&self, x: f64, y: f64) -> Option<HoveredRecord> {
        // Two-pass: find closest (source_idx, sample_idx) first, then construct record
        let mut best: Option<(f64, f64, usize, usize)> = None; // (x_dist, y_dist, si, ri)
        for (si, source) in self.sources.iter().enumerate() {
            if !source.visible {
                continue;
            }
            for (ri, sample) in source.samples.iter().enumerate() {
                let Some(value) = sample.value else { continue };
                if !source.visible_modes.contains(&sample.mode) {
                    continue;
                }
                let x_dist = (sample.x - x).abs();
                let y_dist = (value - y).abs();
                if best.is_none_or(|(bx, by, _, _)| {
                    x_dist < bx || (x_dist == bx && y_dist < by)
                }) {
                    best = Some((x_dist, y_dist, si, ri));
                }
            }
        }
        let (_, _, si, ri) = best?;
        let source = &self.sources[si];
        let sample = &source.samples[ri];
        Some(HoveredRecord {
            series: source.label.clone(),
            x: sample.x,
            timestamp: sample.x_label.clone(),
            value: sample.value?,
            unit: sample.unit.clone(),
            mode: sample.mode.clone(),
        })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn nearest_visible_record(&self, x: f64) -> Option<HoveredRecord> {
        // Two-pass: first find closest (source_idx, sample_idx), then construct record
        let mut best: Option<(f64, usize, usize)> = None;
        for (si, source) in self.sources.iter().enumerate() {
            if !source.visible {
                continue;
            }
            for (ri, sample) in source.samples.iter().enumerate() {
                if sample.value.is_none() || !source.visible_modes.contains(&sample.mode) {
                    continue;
                }
                let dist = (sample.x - x).abs();
                if best.is_none_or(|(d, _, _)| dist < d) {
                    best = Some((dist, si, ri));
                }
            }
        }
        let (_, si, ri) = best?;
        let source = &self.sources[si];
        let sample = &source.samples[ri];
        Some(HoveredRecord {
            series: source.label.clone(),
            x: sample.x,
            timestamp: sample.x_label.clone(),
            value: sample.value?,
            unit: sample.unit.clone(),
            mode: sample.mode.clone(),
        })
    }

    #[allow(dead_code)]
    pub fn files_mut(&mut self) -> &mut Vec<ViewerSource> {
        &mut self.sources
    }

    pub fn remove_source(&mut self, source_id: ViewerSourceId) {
        if let Some(index) = self.source_index(source_id) {
            self.sources.remove(index);
            self.recompute_active_domain();
        }
    }

    #[allow(dead_code)]
    pub fn remove_file(&mut self, file_id: FileId) {
        if let Some(source_id) = self.sources.get(file_id).map(|source| source.id) {
            self.remove_source(source_id);
        }
    }

    pub fn file_count(&self) -> usize {
        self.sources.len()
    }

    fn attach_path_source(
        &mut self,
        path: PathBuf,
        is_live: bool,
        replace_existing: bool,
    ) -> Result<ViewerSourceId, CsvViewerError> {
        let content = std::fs::read(&path)?;
        let (records, last_read_pos) = parse_initial_records(&content)?;
        let x_domain = infer_x_domain(&records);
        let last_modified = std::fs::metadata(&path)
            .ok()
            .and_then(|metadata| metadata.modified().ok());

        if !replace_existing {
            self.ensure_compatible_domain(x_domain)?;
        }

        if replace_existing {
            self.sources.clear();
            self.active_domain = None;
            self.next_color_idx = 0;
        }

        self.active_domain.get_or_insert(x_domain);

        let kind = source_kind_for_path(&path, &records, is_live);
        let label = source_label(&kind, &path);
        let mut source = ViewerSource {
            id: self.allocate_source_id(),
            kind,
            path,
            x_domain,
            label,
            visible: true,
            color: self.allocate_color(),
            status: SourceStatus::Ready,
            records,
            samples: Vec::new(),
            mode_changes: Vec::new(),
            modes: Vec::new(),
            visible_modes: HashSet::new(),
            mode_filter_initialized: false,
            runtime_anchor: None,
            last_read_pos,
            last_modified,
            is_live,
        };
        refresh_source_metadata(&mut source);
        let id = source.id;
        self.sources.push(source);

        Ok(id)
    }

    fn allocate_source_id(&mut self) -> ViewerSourceId {
        let id = self.next_source_id;
        self.next_source_id += 1;
        id
    }

    fn allocate_color(&mut self) -> egui::Color32 {
        let color = next_color(self.next_color_idx);
        self.next_color_idx += 1;
        color
    }

    fn ensure_compatible_domain(&self, candidate: XDomain) -> Result<(), CsvViewerError> {
        match self.active_domain {
            Some(active) if active != candidate => Err(CsvViewerError::Message(format!(
                "Cannot attach source with incompatible time axis: {candidate:?} vs {active:?}"
            ))),
            _ => Ok(()),
        }
    }

    fn recompute_active_domain(&mut self) {
        self.active_domain = self.sources.first().map(|source| source.x_domain);
    }

    fn source_by_id(&self, source_id: ViewerSourceId) -> Option<&ViewerSource> {
        self.sources.iter().find(|source| source.id == source_id)
    }

    #[cfg(test)]
    fn source_by_id_mut(&mut self, source_id: ViewerSourceId) -> Option<&mut ViewerSource> {
        self.sources
            .iter_mut()
            .find(|source| source.id == source_id)
    }

    fn source_index(&self, source_id: ViewerSourceId) -> Option<usize> {
        self.sources.iter().position(|source| source.id == source_id)
    }

    #[cfg(test)]
    pub fn push_test_sample(
        &mut self,
        source_id: ViewerSourceId,
        x: f64,
        value: Option<f64>,
        unit: &str,
        mode: &str,
    ) {
        let Some(source) = self.source_by_id_mut(source_id) else {
            return;
        };

        let x_label = match source.x_domain {
            XDomain::WallClock => format_epoch_rfc3339(x),
            XDomain::SequenceIndex => format!("#{}", x.round() as i64),
        };
        let device = match source.kind {
            ViewerSourceKind::RuntimeDevice { device }
            | ViewerSourceKind::LiveCsvTail { device, .. } => runtime_device_name(device),
            ViewerSourceKind::CsvFile { .. } => "CSV".to_owned(),
        };

        source.samples.push(ViewerSample {
            x,
            x_label,
            value,
            device,
            unit: unit.to_owned(),
            mode: mode.to_owned(),
            is_overload: value.is_none(),
            is_open: false,
            is_short: false,
        });
        refresh_source_metadata(source);
    }
}

fn next_color(idx: usize) -> egui::Color32 {
    let rgb = COLORS[idx % COLORS.len()];
    egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

fn poll_path_source(source: &mut ViewerSource) {
    let Ok(mut handle) = std::fs::File::open(&source.path) else {
        return;
    };
    let Ok(metadata) = handle.metadata() else {
        return;
    };
    let file_len = metadata.len();
    let modified = metadata.modified().ok();

    let replaced_in_place = matches!(
        (&source.last_modified, &modified),
        (Some(previous), Some(current)) if current > previous && file_len <= source.last_read_pos
    );

    if file_len < source.last_read_pos || replaced_in_place {
        source.last_read_pos = 0;
        source.records.clear();
        source.mode_filter_initialized = false;
        source.visible_modes.clear();
        refresh_source_metadata(source);
    }

    if file_len <= source.last_read_pos {
        return;
    }

    if handle.seek(SeekFrom::Start(source.last_read_pos)).is_err() {
        return;
    }

    let mut unread = Vec::new();
    if handle.read_to_end(&mut unread).is_err() {
        return;
    }

    let consumed_len = last_complete_line_offset(&unread);
    if consumed_len == 0 {
        return;
    }

    let old_len = source.records.len();
    let reader = BufReader::new(&unread[..consumed_len]);
    for line in reader.lines().map_while(Result::ok) {
        if let Some(record) = parse_row(&line) {
            source.records.push(record);
        }
    }

    if source.records.len() != old_len {
        refresh_source_metadata(source);
    }

    source.last_read_pos += consumed_len as u64;
    source.last_modified = modified;
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

fn refresh_source_metadata(source: &mut ViewerSource) {
    if !matches!(source.kind, ViewerSourceKind::RuntimeDevice { .. }) {
        source.samples = normalize_samples(&source.records, source.x_domain);
    }
    source.mode_changes = if source.records.is_empty() {
        find_mode_changes_in_samples(&source.samples)
    } else {
        find_mode_changes(&source.records)
    };

    let mut modes: Vec<String> = source
        .samples
        .iter()
        .map(|sample| sample.mode.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    modes.sort();
    source.modes = modes.clone();
    let available_modes: HashSet<String> = modes.iter().cloned().collect();

    let selected_modes: HashSet<String> = if !source.mode_filter_initialized {
        modes.iter().cloned().collect()
    } else {
        source
            .visible_modes
            .intersection(&available_modes)
            .cloned()
            .collect()
    };

    source.visible_modes = selected_modes;
}

fn push_runtime_sample(source: &mut ViewerSource, measurement: &DeviceMeasurement) {
    let anchor = source
        .runtime_anchor
        .get_or_insert_with(|| RuntimeAnchor {
            first_monotonic: measurement.timestamp,
            first_wall_clock_epoch: utc_now_epoch(),
        });
    let x = anchor.first_wall_clock_epoch
        + measurement
            .timestamp
            .saturating_duration_since(anchor.first_monotonic)
            .as_secs_f64();

    source.samples.push(ViewerSample {
        x,
        x_label: format_epoch_rfc3339(x),
        value: measurement.primary_value,
        device: runtime_device_name(measurement.device),
        unit: measurement.primary_unit.clone(),
        mode: measurement.mode_string.clone(),
        is_overload: measurement.is_overload,
        is_open: measurement.is_open,
        is_short: measurement.is_short,
    });
    source.status = SourceStatus::Ready;

    // Incremental metadata update — only check the new sample for mode changes
    let len = source.samples.len();
    if len >= 2 {
        let prev = &source.samples[len - 2];
        let curr = &source.samples[len - 1];
        if prev.device == curr.device && prev.mode != curr.mode {
            source.mode_changes.push(len - 1);
        }
    }
    // Add new mode if not yet seen
    let new_mode = &source.samples[len - 1].mode;
    if !source.modes.contains(new_mode) {
        source.modes.push(new_mode.clone());
        source.modes.sort();
        if !source.mode_filter_initialized {
            source.visible_modes.insert(new_mode.clone());
        }
    }
    if !source.mode_filter_initialized {
        source.mode_filter_initialized = true;
    }
}

fn normalize_samples(records: &[CsvRecord], x_domain: XDomain) -> Vec<ViewerSample> {
    records
        .iter()
        .enumerate()
        .map(|(idx, record)| ViewerSample {
            x: source_x(record, idx, x_domain),
            x_label: source_x_label(record, idx, x_domain),
            value: record.value,
            device: record.device.clone(),
            unit: record.unit.clone(),
            mode: record.mode.clone(),
            is_overload: record.is_overload,
            is_open: record.is_open,
            is_short: record.is_short,
        })
        .collect()
}

fn source_x(record: &CsvRecord, idx: usize, x_domain: XDomain) -> f64 {
    match x_domain {
        XDomain::WallClock => record.parsed_time.unwrap_or(idx as f64),
        XDomain::SequenceIndex => idx as f64,
    }
}

fn source_x_label(record: &CsvRecord, idx: usize, x_domain: XDomain) -> String {
    match x_domain {
        XDomain::WallClock => record.timestamp.clone(),
        XDomain::SequenceIndex => format!("#{idx}"),
    }
}

fn infer_x_domain(records: &[CsvRecord]) -> XDomain {
    if records.iter().all(|record| record.parsed_time.is_some()) {
        XDomain::WallClock
    } else {
        XDomain::SequenceIndex
    }
}

fn find_mode_changes_in_samples(samples: &[ViewerSample]) -> Vec<usize> {
    let mut changes = Vec::new();
    for idx in 1..samples.len() {
        if samples[idx].mode != samples[idx - 1].mode {
            changes.push(idx);
        }
    }
    changes
}

fn source_kind_for_path(path: &Path, records: &[CsvRecord], is_live: bool) -> ViewerSourceKind {
    if is_live
        && let Some(device) = guess_device_id(records, path)
    {
        return ViewerSourceKind::LiveCsvTail {
            device,
            path: path.to_path_buf(),
        };
    }

    ViewerSourceKind::CsvFile {
        path: path.to_path_buf(),
    }
}

fn guess_device_id(records: &[CsvRecord], path: &Path) -> Option<DeviceId> {
    if let Some(record) = records.first() {
        if record.device.eq_ignore_ascii_case("multimeter") {
            return Some(DeviceId::Multimeter);
        }
        if record.device.eq_ignore_ascii_case("usb-c") || record.device.eq_ignore_ascii_case("usbc")
        {
            return Some(DeviceId::UsbC);
        }
    }

    let path_string = path.to_string_lossy().to_ascii_lowercase();
    if path_string.contains("multimeter") || path_string.contains("meter") || path_string.contains("mm")
    {
        return Some(DeviceId::Multimeter);
    }
    if path_string.contains("usb-c") || path_string.contains("usbc") {
        return Some(DeviceId::UsbC);
    }

    None
}

fn source_label(kind: &ViewerSourceKind, path: &Path) -> String {
    match kind {
        ViewerSourceKind::CsvFile { .. } => path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("CSV")
            .to_string(),
        ViewerSourceKind::LiveCsvTail { device, .. } => match device {
            DeviceId::Multimeter => "MM Tail".to_owned(),
            DeviceId::UsbC => "USB-C Tail".to_owned(),
        },
        ViewerSourceKind::RuntimeDevice { device } => runtime_source_label(*device),
    }
}

fn runtime_source_label(device: DeviceId) -> String {
    match device {
        DeviceId::Multimeter => "MM Live".to_owned(),
        DeviceId::UsbC => "USB-C Live".to_owned(),
    }
}

fn runtime_device_name(device: DeviceId) -> String {
    match device {
        DeviceId::Multimeter => "Multimeter".to_owned(),
        DeviceId::UsbC => "UsbC".to_owned(),
    }
}

fn runtime_status_for_connection(state: &ConnectionState, has_samples: bool) -> SourceStatus {
    match state {
        ConnectionState::Connected => {
            if has_samples {
                SourceStatus::Ready
            } else {
                SourceStatus::Waiting("Waiting for samples".to_owned())
            }
        }
        ConnectionState::Connecting => SourceStatus::Waiting("Connecting".to_owned()),
        ConnectionState::Reconnecting => SourceStatus::Waiting("Reconnecting".to_owned()),
        ConnectionState::Disconnected => SourceStatus::Waiting("Disconnected".to_owned()),
        ConnectionState::Error(message) => SourceStatus::Error(message.clone()),
    }
}

fn format_export_value(value: Option<f64>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => "OL".to_owned(),
    }
}

fn utc_now_epoch() -> f64 {
    let now = Utc::now();
    now.timestamp() as f64 + f64::from(now.timestamp_subsec_nanos()) / 1e9
}

fn format_epoch_rfc3339(epoch: f64) -> String {
    let total_nanos = (epoch * 1_000_000_000.0).round() as i128;
    let seconds = (total_nanos / 1_000_000_000) as i64;
    let nanos = (total_nanos % 1_000_000_000) as u32;

    Utc.timestamp_opt(seconds, nanos)
        .single()
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| format!("{epoch:.3}"))
}

pub(super) fn record_x(record: &CsvRecord, idx: usize) -> f64 {
    record.parsed_time.unwrap_or(idx as f64)
}

#[cfg(test)]
mod tests {
    use super::{CsvDataStore, ViewerSource, infer_x_domain, refresh_source_metadata};
    use crate::widgets::graph_viewer::source_model::{SourceStatus, ViewerSourceKind, XDomain};
    use chrono::DateTime;
    use readout_core::csv_record::CsvRecord;
    use readout_core::measurement_mode::MeasurementMode;
    use readout_core::types::{
        AlarmState, ConnectionState, DeviceId, DeviceMeasurement, RuntimeEvent,
    };
    use std::collections::HashSet;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::Duration;

    fn write_temp_csv(contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "readout_csv_viewer_{}_{}.csv",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before epoch")
                .as_nanos()
        ));

        fs::write(&path, contents).expect("write temp csv");
        path
    }

    fn csv_with_value(timestamp: &str, value: f64) -> String {
        format!(
            "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n\
             {timestamp},Multimeter,{value},V,DCV,false,false,false\n"
        )
    }

    fn build_dense_live_csv() -> String {
        let mut csv =
            "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n".to_owned();

        for second in 0..=95 {
            csv.push_str(&format!(
                "1970-01-01T00:01:{second:02}Z,Multimeter,{second},V,DCV,false,false,false\n"
            ));
        }

        for (second, value) in [
            (96, 0.0),
            (97, 1.5),
            (98, 0.5),
            (99, 2.0),
            (100, 1.0),
        ] {
            csv.push_str(&format!(
                "1970-01-01T00:01:{second:02}Z,Multimeter,{value},V,DCV,false,false,false\n"
            ));
        }

        csv
    }

    fn fake_measurement(device: DeviceId, value: f64) -> DeviceMeasurement {
        DeviceMeasurement {
            timestamp: std::time::Instant::now(),
            device,
            primary_value: Some(value),
            primary_unit: "V".to_owned(),
            secondary_value: None,
            secondary_unit: None,
            power_watts: None,
            energy_mwh: None,
            energy_mah: None,
            mode: MeasurementMode::DcVoltage,
            mode_string: "DCV".to_owned(),
            is_overload: false,
            is_open: false,
            is_short: false,
            alarm_state: AlarmState::None,
        }
    }

    fn test_source(
        path: &str,
        records: Vec<CsvRecord>,
        visible_modes: Option<HashSet<String>>,
    ) -> ViewerSource {
        let x_domain = infer_x_domain(&records);
        let visible_modes = visible_modes.unwrap_or_default();
        let mut source = ViewerSource {
            id: 0,
            kind: ViewerSourceKind::CsvFile {
                path: PathBuf::from(path),
            },
            path: PathBuf::from(path),
            x_domain,
            label: path.to_owned(),
            visible: true,
            color: egui::Color32::WHITE,
            status: SourceStatus::Ready,
            records,
            samples: Vec::new(),
            mode_changes: Vec::new(),
            modes: Vec::new(),
            visible_modes,
            mode_filter_initialized: true,
            runtime_anchor: None,
            last_read_pos: 0,
            last_modified: None,
            is_live: false,
        };
        refresh_source_metadata(&mut source);
        source
    }

    #[test]
    fn query_points_skips_non_numeric_rows() {
        let mut store = CsvDataStore::new();
        store.sources.push(test_source(
            "sample.csv",
            vec![
                CsvRecord {
                    timestamp: "2026-03-29T10:00:00Z".to_string(),
                    parsed_time: Some(10.0),
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
                    parsed_time: Some(20.0),
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
                    parsed_time: Some(30.0),
                    device: "Multimeter".to_string(),
                    value: Some(2.5),
                    unit: "V".to_string(),
                    mode: "DCV".to_string(),
                    is_overload: false,
                    is_open: false,
                    is_short: false,
                },
            ],
            Some(std::iter::once("DCV".to_string()).collect()),
        ));

        let points = store.query_points(0, 32);

        assert_eq!(
            points,
            vec![
                (Duration::from_secs(10), 1.25),
                (Duration::from_secs(30), 2.5),
            ]
        );
    }

    #[test]
    fn query_points_in_view_returns_only_visible_samples() {
        let path = write_temp_csv(concat!(
            "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n",
            "1970-01-01T00:00:00Z,Multimeter,1.0,V,DCV,false,false,false\n",
            "1970-01-01T00:00:01Z,Multimeter,2.0,V,DCV,false,false,false\n",
            "1970-01-01T00:00:02Z,Multimeter,3.0,V,DCV,false,false,false\n",
        ));

        let mut store = CsvDataStore::new();
        let source_id = store.load_csv_file(path.clone(), false).unwrap();
        let points = store.query_points_in_view(source_id, Some((1.0, 2.0)), 64);

        assert_eq!(points.len(), 2);
        assert_eq!(points[0].0.as_secs_f64(), 1.0);
        assert_eq!(points[0].1, 2.0);
        assert_eq!(points[1].0.as_secs_f64(), 2.0);
        assert_eq!(points[1].1, 3.0);

        fs::remove_file(path).expect("remove temp csv");
    }

    #[test]
    fn query_points_in_view_keeps_newest_local_live_shape() {
        let path = write_temp_csv(&build_dense_live_csv());

        let mut store = CsvDataStore::new();
        let source_id = store
            .attach_live_csv(DeviceId::Multimeter, path.clone())
            .expect("attach live csv");
        let points = store.query_points_in_view(source_id, Some((96.0, 100.0)), 256);

        let xs: Vec<f64> = points.iter().map(|(x, _)| x.as_secs_f64()).collect();
        let ys: Vec<f64> = points.iter().map(|(_, y)| *y).collect();
        assert_eq!(xs, vec![96.0, 97.0, 98.0, 99.0, 100.0]);
        assert_eq!(ys, vec![0.0, 1.5, 0.5, 2.0, 1.0]);

        fs::remove_file(path).expect("remove temp csv");
    }

    #[test]
    fn nearest_visible_record_uses_time_axis_and_mode_filter() {
        let mut store = CsvDataStore::new();
        store.sources.push(test_source(
            "sample.csv",
            vec![
                CsvRecord {
                    timestamp: "2026-03-29T10:00:00Z".to_string(),
                    parsed_time: Some(100.0),
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
                    parsed_time: Some(101.0),
                    device: "Multimeter".to_string(),
                    value: Some(2.5),
                    unit: "V".to_string(),
                    mode: "ACV".to_string(),
                    is_overload: false,
                    is_open: false,
                    is_short: false,
                },
            ],
            Some(std::iter::once("DCV".to_string()).collect()),
        ));

        let hovered = store
            .nearest_visible_record(100.8)
            .expect("nearest visible record");

        assert_eq!(hovered.series, "sample.csv");
        assert_eq!(hovered.timestamp, "2026-03-29T10:00:00Z");
        assert_eq!(hovered.value, 1.25);
        assert_eq!(hovered.mode, "DCV");
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
        let first_time = store.files()[0].records[0]
            .parsed_time
            .expect("first parsed timestamp");

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
        let second_time = store.files()[0].records[1]
            .parsed_time
            .expect("second parsed timestamp");
        assert!((second_time - first_time - 1.0).abs() < 1e-9);

        fs::remove_file(&path).expect("remove temp csv");
    }

    #[test]
    fn attach_runtime_source_is_deduplicated_per_device() {
        let mut store = CsvDataStore::new();

        let first = store.attach_runtime_device(DeviceId::Multimeter).unwrap();
        let second = store.attach_runtime_device(DeviceId::Multimeter).unwrap();

        assert_eq!(first, second);
        assert_eq!(store.sources().len(), 1);
    }

    #[test]
    fn attach_rejects_incompatible_x_domain() {
        let path = write_temp_csv(
            "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n\
             not-a-time,Multimeter,1.0,V,DCV,false,false,false\n",
        );

        let mut store = CsvDataStore::new();
        store.load_csv_file(path.clone(), false).unwrap();

        let err = store.attach_runtime_device(DeviceId::Multimeter).unwrap_err();
        assert!(err.to_string().contains("incompatible time axis"));
        assert_eq!(store.sources()[0].x_domain, XDomain::SequenceIndex);

        fs::remove_file(path).expect("remove temp csv");
    }

    #[test]
    fn remove_source_uses_stable_id_instead_of_vec_position() {
        let a = write_temp_csv(&csv_with_value("2026-03-29T10:00:00Z", 1.0));
        let b = write_temp_csv(&csv_with_value("2026-03-29T10:00:01Z", 2.0));

        let mut store = CsvDataStore::new();
        let source_a = store.load_csv_file(a.clone(), false).unwrap();
        let source_b = store.load_csv_file(b.clone(), false).unwrap();

        store.remove_source(source_a);

        let points = store.query_points(source_b, 32);
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].1, 2.0);

        fs::remove_file(a).expect("remove temp csv a");
        fs::remove_file(b).expect("remove temp csv b");
    }

    #[test]
    fn poll_live_csv_sources_keeps_incomplete_trailing_row_until_completed() {
        let path = write_temp_csv(concat!(
            "timestamp,device,value,unit,mode,is_overload,is_open,is_short\n",
            "2026-03-29T10:00:00Z,Multimeter,1.25,V,DCV,false,false,false\n",
            "2026-03-29T10:00:01Z,Multimeter,2.50"
        ));

        let mut store = CsvDataStore::new();
        let source_id = store
            .attach_live_csv(DeviceId::Multimeter, path.clone())
            .expect("attach live csv");

        assert_eq!(store.query_points(source_id, 32).len(), 1);

        {
            let mut file = OpenOptions::new()
                .append(true)
                .open(&path)
                .expect("open csv for append");
            writeln!(file, ",V,DCV,false,false,false").expect("append row remainder");
        }

        store.poll_live_sources();

        let points = store.query_points(source_id, 32);
        assert_eq!(points.len(), 2);
        assert_eq!(points[1].1, 2.5);

        fs::remove_file(path).expect("remove temp csv");
    }

    #[test]
    fn live_tail_reload_after_truncation_restarts_from_file_start() {
        let path = write_temp_csv(&csv_with_value("2026-03-29T10:00:00Z", 1.0));

        let mut store = CsvDataStore::new();
        let source_id = store
            .attach_live_csv(DeviceId::Multimeter, path.clone())
            .expect("attach live csv");

        fs::write(&path, csv_with_value("2026-03-29T10:00:10Z", 9.0)).expect("truncate and rewrite csv");

        store.poll_live_sources();

        let points = store.query_points(source_id, 32);
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].1, 9.0);

        fs::remove_file(path).expect("remove temp csv");
    }

    #[test]
    fn runtime_measurements_buffer_only_after_attach() {
        let mut store = CsvDataStore::new();
        let measurement = fake_measurement(DeviceId::Multimeter, 12.0);

        store.handle_runtime_event(&RuntimeEvent::Measurement {
            device: DeviceId::Multimeter,
            value: measurement.clone(),
        });
        assert!(store.latest_live_x().is_none());

        let source_id = store.attach_runtime_device(DeviceId::Multimeter).unwrap();
        store.handle_runtime_event(&RuntimeEvent::Measurement {
            device: DeviceId::Multimeter,
            value: measurement,
        });

        assert_eq!(store.query_points(source_id, 32).len(), 1);
    }

    #[test]
    fn runtime_export_uses_derived_wall_clock_timestamps() {
        let mut store = CsvDataStore::new();
        let source_id = store.attach_runtime_device(DeviceId::Multimeter).unwrap();

        let first = fake_measurement(DeviceId::Multimeter, 1.0);
        store.handle_runtime_event(&RuntimeEvent::Measurement {
            device: DeviceId::Multimeter,
            value: first,
        });

        std::thread::sleep(Duration::from_millis(5));

        let second = fake_measurement(DeviceId::Multimeter, 2.0);
        store.handle_runtime_event(&RuntimeEvent::Measurement {
            device: DeviceId::Multimeter,
            value: second,
        });

        let rows = store.export_rows(Some(selection_for_last_runtime_point(
            &store,
            source_id,
        )));

        assert_eq!(rows.len(), 1);
        assert!(!rows[0].timestamp.is_empty());
        DateTime::parse_from_rfc3339(&rows[0].timestamp).expect("valid runtime timestamp");
        let all_rows = store.export_rows(None);
        assert!(all_rows[0].timestamp < all_rows[1].timestamp);
    }

    fn selection_for_last_runtime_point(
        store: &CsvDataStore,
        source_id: u64,
    ) -> (f64, f64) {
        let points = store.query_points(source_id, 32);
        let last_x = points.last().expect("runtime point").0.as_secs_f64();
        (last_x - 0.001, last_x + 0.001)
    }

    #[test]
    fn runtime_connection_status_updates_waiting_message() {
        let mut store = CsvDataStore::new();
        store.attach_runtime_device(DeviceId::Multimeter).unwrap();

        store.handle_runtime_event(&RuntimeEvent::ConnectionChanged {
            device: DeviceId::Multimeter,
            state: ConnectionState::Disconnected,
        });

        match &store.sources()[0].status {
            SourceStatus::Waiting(message) => assert!(message.contains("Disconnected")),
            other => panic!("expected waiting status, got {other:?}"),
        }
    }

    #[test]
    fn nearest_visible_sample_prefers_matching_y_when_x_values_overlap() {
        let mut store = CsvDataStore::new();
        let low = store.attach_runtime_device(DeviceId::Multimeter).unwrap();
        let high = store.attach_runtime_device(DeviceId::UsbC).unwrap();

        store.push_test_sample(low, 100.0, Some(1.0), "V", "DCV");
        store.push_test_sample(high, 100.0, Some(9.0), "V", "DCV");

        let hovered = store.nearest_visible_sample(100.0, 8.8).unwrap();
        assert_eq!(hovered.series, "USB-C Live");
        assert_eq!(hovered.value, 9.0);
    }
}

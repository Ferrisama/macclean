use anyhow::{bail, Result};
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static SESSION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize)]
pub struct HistoryRecord {
    pub session_id: String,
    pub timestamp: u64,
    pub cleaner: String,
    pub label: String,
    pub original_path: PathBuf,
    pub trash_path: Option<PathBuf>,
    pub size_bytes: u64,
    pub method: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum RestoreState {
    Available,
    OriginalExists,
    TrashItemMissing,
    NotTrashBacked,
}

impl RestoreState {
    pub fn label(&self) -> &'static str {
        match self {
            RestoreState::Available => "available",
            RestoreState::OriginalExists => "original exists",
            RestoreState::TrashItemMissing => "missing from Trash",
            RestoreState::NotTrashBacked => "not restorable",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub timestamp: u64,
    pub cleaner: String,
    pub item_count: usize,
    pub total_bytes: u64,
    pub method: String,
    pub restorable_count: usize,
}

#[derive(Debug, Clone)]
pub struct RestoreOutcome {
    pub record: HistoryRecord,
    pub restored: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReceiptItem {
    pub label: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub method: String,
    pub status: String,
    pub restore: String,
    pub error: Option<String>,
    pub trash_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
struct CleanupReceipt {
    session_id: String,
    timestamp: u64,
    command: String,
    cleaner: String,
    method: String,
    items: Vec<ReceiptItem>,
}

impl HistoryRecord {
    pub fn new(
        session_id: impl Into<String>,
        cleaner: impl Into<String>,
        label: impl Into<String>,
        original_path: PathBuf,
        trash_path: Option<PathBuf>,
        size_bytes: u64,
        method: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            timestamp: now_secs(),
            cleaner: cleaner.into(),
            label: label.into(),
            original_path,
            trash_path,
            size_bytes,
            method: method.into(),
        }
    }

    pub fn restore_state(&self) -> RestoreState {
        if self.method != "trash" {
            return RestoreState::NotTrashBacked;
        }
        if self.original_path.exists() {
            return RestoreState::OriginalExists;
        }
        let Some(trash_path) = self.trash_path.as_ref() else {
            return RestoreState::NotTrashBacked;
        };
        if trash_path.exists() {
            RestoreState::Available
        } else {
            RestoreState::TrashItemMissing
        }
    }
}

pub fn new_session_id(cleaner: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let sequence = SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{}-{:x}-{:x}", cleaner, nanos, sequence)
}

pub fn append(record: &HistoryRecord) -> Result<()> {
    let path = history_path()?;
    append_to_path(&path, record)
}

fn append_to_path(path: &Path, record: &HistoryRecord) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(path)?;
    let length = file.metadata()?.len();
    if length > 0 {
        file.seek(SeekFrom::End(-1))?;
        let mut last = [0_u8; 1];
        file.read_exact(&mut last)?;
        if last[0] != b'\n' {
            // A killed writer may leave an incomplete final record. Start the
            // next valid record on a fresh line so recovery can ignore only
            // the damaged line instead of losing all later history.
            file.write_all(b"\n")?;
        }
    }
    writeln!(
        file,
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        record.timestamp,
        escape(&record.session_id),
        escape(&record.cleaner),
        escape(&record.label),
        escape(&record.original_path.display().to_string()),
        escape(
            &record
                .trash_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
        ),
        record.size_bytes,
        escape(&record.method)
    )?;
    file.sync_data()?;
    Ok(())
}

pub fn write_receipt(
    session_id: &str,
    cleaner: &str,
    method: &str,
    items: Vec<ReceiptItem>,
) -> Result<PathBuf> {
    let path = receipt_path(session_id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let receipt = CleanupReceipt {
        session_id: session_id.into(),
        timestamp: now_secs(),
        command: std::env::args().collect::<Vec<_>>().join(" "),
        cleaner: cleaner.into(),
        method: method.into(),
        items,
    };
    atomic_write(&path, serde_json::to_string_pretty(&receipt)?.as_bytes())?;
    Ok(path)
}

fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Output path has no parent: {}", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("macclean-record");
    let sequence = SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(
        ".{}.tmp-{}-{}",
        file_name,
        std::process::id(),
        sequence
    ));

    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(data)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub fn read_all() -> Result<Vec<HistoryRecord>> {
    let path = history_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = fs::File::open(path)?;
    let mut records = Vec::new();
    for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
        if let Some(record) = parse_line(&line) {
            records.push(record);
        }
    }
    Ok(records)
}

pub fn latest_session() -> Result<String> {
    read_all()?
        .into_iter()
        .max_by_key(|record| record.timestamp)
        .map(|record| record.session_id)
        .ok_or_else(|| anyhow::anyhow!("No macclean history found."))
}

pub fn session_summaries() -> Result<Vec<SessionSummary>> {
    let mut summaries = Vec::<SessionSummary>::new();
    for record in read_all()? {
        if let Some(summary) = summaries
            .iter_mut()
            .find(|summary| summary.session_id == record.session_id)
        {
            summary.timestamp = summary.timestamp.max(record.timestamp);
            summary.item_count += 1;
            summary.total_bytes = summary.total_bytes.saturating_add(record.size_bytes);
            if record.restore_state() == RestoreState::Available {
                summary.restorable_count += 1;
            }
        } else {
            let restorable_count = usize::from(record.restore_state() == RestoreState::Available);
            summaries.push(SessionSummary {
                session_id: record.session_id.clone(),
                timestamp: record.timestamp,
                cleaner: record.cleaner.clone(),
                item_count: 1,
                total_bytes: record.size_bytes,
                method: record.method.clone(),
                restorable_count,
            });
        }
    }

    summaries.sort_by_key(|summary| std::cmp::Reverse(summary.timestamp));
    Ok(summaries)
}

pub fn restore_session_detailed(session_id: Option<&str>) -> Result<Vec<RestoreOutcome>> {
    let session = match session_id {
        Some(id) => id.to_string(),
        None => latest_session()?,
    };

    let records: Vec<_> = read_all()?
        .into_iter()
        .filter(|record| record.session_id == session && record.method == "trash")
        .collect();

    if records.is_empty() {
        bail!(
            "No trash-backed history records found for session '{}'.",
            session
        );
    }

    Ok(restore_records(records))
}

fn restore_records(records: Vec<HistoryRecord>) -> Vec<RestoreOutcome> {
    let mut outcomes = Vec::new();
    for record in records {
        match restore_record(&record) {
            Ok(()) => outcomes.push(RestoreOutcome {
                record,
                restored: true,
                error: None,
            }),
            Err(e) => outcomes.push(RestoreOutcome {
                record,
                restored: false,
                error: Some(e.to_string()),
            }),
        }
    }
    outcomes
}

fn restore_record(record: &HistoryRecord) -> Result<()> {
    if record.original_path.exists() {
        bail!(
            "Original path already exists: {}",
            record.original_path.display()
        );
    }

    let Some(trashed) = record.trash_path.as_ref() else {
        bail!(
            "This cleanup record has no verified Trash destination and cannot be restored automatically."
        );
    };
    if !trashed.exists() {
        bail!("Trash item not found: {}", trashed.display());
    }
    if let Some(parent) = record.original_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(trashed, &record.original_path)?;
    Ok(())
}

fn history_path() -> Result<PathBuf> {
    Ok(crate::core::state_dir()?.join("history.tsv"))
}

pub fn receipt_path(session_id: &str) -> Result<PathBuf> {
    Ok(crate::core::state_dir()?
        .join("receipts")
        .join(format!("{}.json", session_id)))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn parse_line(line: &str) -> Option<HistoryRecord> {
    let parts: Vec<_> = line.split('\t').collect();
    if parts.len() != 7 && parts.len() != 8 {
        return None;
    }
    Some(HistoryRecord {
        timestamp: parts[0].parse().ok()?,
        session_id: unescape(parts[1]),
        cleaner: unescape(parts[2]),
        label: unescape(parts[3]),
        original_path: Path::new(&unescape(parts[4])).to_path_buf(),
        trash_path: (parts.len() == 8 && !parts[5].is_empty())
            .then(|| PathBuf::from(unescape(parts[5]))),
        size_bytes: parts[if parts.len() == 8 { 6 } else { 5 }].parse().ok()?,
        method: unescape(parts[if parts.len() == 8 { 7 } else { 6 }]),
    })
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

fn unescape(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_record(original_path: PathBuf, trash_path: Option<PathBuf>) -> HistoryRecord {
        HistoryRecord::new(
            "session",
            "test",
            "item",
            original_path,
            trash_path,
            4,
            "trash",
        )
    }

    #[test]
    fn escapes_round_trip() {
        let value = "a\tb\nc\\d";
        assert_eq!(unescape(&escape(value)), value);
    }

    #[test]
    fn session_ids_are_unique_within_the_same_second() {
        let first = new_session_id("app-review");
        let second = new_session_id("app-review");
        assert_ne!(first, second);
    }

    #[test]
    fn history_with_a_recorded_trash_path_round_trips() {
        let line = "1\tsession\tcleaner\tlabel\t/original\t/.Trash/item\t42\ttrash";
        let record = parse_line(line).unwrap();
        assert_eq!(record.trash_path, Some(PathBuf::from("/.Trash/item")));
        assert_eq!(record.size_bytes, 42);
    }

    #[test]
    fn append_recovers_after_an_interrupted_final_record() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.tsv");
        fs::write(&path, "1\ttruncated-without-newline").unwrap();
        let record = test_record(
            dir.path().join("original"),
            Some(dir.path().join("trashed")),
        );

        append_to_path(&path, &record).unwrap();

        let contents = fs::read_to_string(path).unwrap();
        let parsed: Vec<_> = contents.lines().filter_map(parse_line).collect();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].session_id, "session");
    }

    #[test]
    fn atomic_write_leaves_only_the_completed_destination() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("receipt.json");
        atomic_write(&path, br#"{"status":"complete"}"#).unwrap();

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            r#"{"status":"complete"}"#
        );
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn legacy_history_is_not_promised_as_restorable() {
        let record = parse_line("1\tsession\tcleaner\tlabel\t/original\t42\ttrash").unwrap();
        assert_eq!(record.trash_path, None);
        assert_eq!(record.restore_state(), RestoreState::NotTrashBacked);
    }

    #[test]
    fn restore_refuses_to_overwrite_an_existing_original() {
        let dir = tempdir().unwrap();
        let original = dir.path().join("original");
        let trashed = dir.path().join("trashed");
        fs::write(&original, "keep").unwrap();
        fs::write(&trashed, "trash").unwrap();

        let error = restore_record(&test_record(original.clone(), Some(trashed.clone())))
            .unwrap_err()
            .to_string();
        assert!(error.contains("already exists"));
        assert_eq!(fs::read_to_string(original).unwrap(), "keep");
        assert!(trashed.exists());
    }

    #[test]
    fn restore_uses_the_recorded_collision_destination() {
        let dir = tempdir().unwrap();
        let original = dir.path().join("item");
        let collision_destination = dir.path().join("item 2");
        fs::write(&collision_destination, "data").unwrap();

        restore_record(&test_record(
            original.clone(),
            Some(collision_destination.clone()),
        ))
        .unwrap();
        assert_eq!(fs::read_to_string(original).unwrap(), "data");
        assert!(!collision_destination.exists());
    }

    #[test]
    fn restore_reports_a_missing_trash_item() {
        let dir = tempdir().unwrap();
        let original = dir.path().join("original");
        let missing = dir.path().join("missing-trash-item");
        let error = restore_record(&test_record(original, Some(missing)))
            .unwrap_err()
            .to_string();
        assert!(error.contains("Trash item not found"));
    }

    #[test]
    fn partial_restore_reports_each_item_independently() {
        let dir = tempdir().unwrap();
        let restored_original = dir.path().join("restored");
        let restored_trash = dir.path().join("restored-trash");
        fs::write(&restored_trash, "ok").unwrap();
        let occupied_original = dir.path().join("occupied");
        let occupied_trash = dir.path().join("occupied-trash");
        fs::write(&occupied_original, "keep").unwrap();
        fs::write(&occupied_trash, "blocked").unwrap();

        let outcomes = restore_records(vec![
            test_record(restored_original.clone(), Some(restored_trash)),
            test_record(occupied_original, Some(occupied_trash.clone())),
        ]);
        assert!(outcomes[0].restored);
        assert!(!outcomes[1].restored);
        assert!(restored_original.exists());
        assert!(occupied_trash.exists());
    }
}

use anyhow::{bail, Result};
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, Write};
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
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
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
    fs::write(&path, serde_json::to_string_pretty(&receipt)?)?;
    Ok(path)
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
    Ok(outcomes)
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
    let Some(home) = dirs::home_dir() else {
        bail!("Could not find home directory.");
    };
    Ok(home.join("Library/Application Support/macclean/history.tsv"))
}

pub fn receipt_path(session_id: &str) -> Result<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        bail!("Could not find home directory.");
    };
    Ok(home
        .join("Library/Application Support/macclean/receipts")
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
    fn legacy_history_is_not_promised_as_restorable() {
        let record = parse_line("1\tsession\tcleaner\tlabel\t/original\t42\ttrash").unwrap();
        assert_eq!(record.trash_path, None);
        assert_eq!(record.restore_state(), RestoreState::NotTrashBacked);
    }
}

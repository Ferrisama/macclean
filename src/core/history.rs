use anyhow::{bail, Result};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct HistoryRecord {
    pub session_id: String,
    pub timestamp: u64,
    pub cleaner: String,
    pub label: String,
    pub original_path: PathBuf,
    pub size_bytes: u64,
    pub method: String,
}

impl HistoryRecord {
    pub fn new(
        session_id: impl Into<String>,
        cleaner: impl Into<String>,
        label: impl Into<String>,
        original_path: PathBuf,
        size_bytes: u64,
        method: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            timestamp: now_secs(),
            cleaner: cleaner.into(),
            label: label.into(),
            original_path,
            size_bytes,
            method: method.into(),
        }
    }
}

pub fn new_session_id(cleaner: &str) -> String {
    format!("{}-{}", cleaner, now_secs())
}

pub fn append(record: &HistoryRecord) -> Result<()> {
    let path = history_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(
        file,
        "{}\t{}\t{}\t{}\t{}\t{}\t{}",
        record.timestamp,
        escape(&record.session_id),
        escape(&record.cleaner),
        escape(&record.label),
        escape(&record.original_path.display().to_string()),
        record.size_bytes,
        escape(&record.method)
    )?;
    Ok(())
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

pub fn restore_session(session_id: Option<&str>) -> Result<(usize, usize)> {
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

    let mut restored = 0usize;
    let mut failed = 0usize;
    for record in records {
        match restore_record(&record) {
            Ok(()) => restored += 1,
            Err(_) => failed += 1,
        }
    }
    Ok((restored, failed))
}

fn restore_record(record: &HistoryRecord) -> Result<()> {
    if record.original_path.exists() {
        bail!(
            "Original path already exists: {}",
            record.original_path.display()
        );
    }

    let Some(name) = record.original_path.file_name() else {
        bail!(
            "Original path has no file name: {}",
            record.original_path.display()
        );
    };
    let Some(home) = dirs::home_dir() else {
        bail!("Could not find home directory.");
    };
    let trashed = home.join(".Trash").join(name);
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

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn parse_line(line: &str) -> Option<HistoryRecord> {
    let parts: Vec<_> = line.split('\t').collect();
    if parts.len() != 7 {
        return None;
    }
    Some(HistoryRecord {
        timestamp: parts[0].parse().ok()?,
        session_id: unescape(parts[1]),
        cleaner: unescape(parts[2]),
        label: unescape(parts[3]),
        original_path: Path::new(&unescape(parts[4])).to_path_buf(),
        size_bytes: parts[5].parse().ok()?,
        method: unescape(parts[6]),
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
}

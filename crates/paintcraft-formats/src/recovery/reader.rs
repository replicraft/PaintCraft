//! Validate checkpoint anchors and replay only a backed contiguous journal tail.
use super::*;

pub(super) fn scan_report(dir: &Path) -> ScanReport {
    let mut report = ScanReport::default();
    let Ok(entries) = fs::read_dir(dir) else {
        return report;
    };
    for (count, entry) in entries.enumerate() {
        if count >= 100_000 {
            report.issues.push(
                "Recovery directory scan limit reached; remaining entries were retained".into(),
            );
            break;
        }
        let Ok(entry) = entry else {
            report
                .issues
                .push("Cannot inspect a recovery directory entry".into());
            continue;
        };
        let path = entry.path();
        let Some(session) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".lock"))
            .and_then(|id| Uuid::parse_str(id).ok())
            .filter(|id| {
                path.file_name().and_then(|n| n.to_str()) == Some(format!("{id}.lock").as_str())
            })
        else {
            continue;
        };
        if fs::symlink_metadata(&path).is_ok_and(|m| !m.is_file()) {
            report
                .issues
                .push("Invalid recovery lock file was retained".into());
            continue;
        }
        let lock = match OpenOptions::new().read(true).write(true).open(&path) {
            Ok(lock) => lock,
            Err(error) => {
                report.issues.push(format!(
                    "Cannot open recovery lock for {session}: {}; data retained",
                    error.kind()
                ));
                continue;
            }
        };
        match lock.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => continue,
            Err(error) => {
                report.issues.push(format!(
                    "Cannot acquire recovery lock for {session}: {error}; data retained"
                ));
                continue;
            }
        }
        let marker = tombstone(dir, session);
        if marker.exists() {
            if read_bounded(&marker, RECORD_MAX).is_ok_and(|b| b == cleared_bytes(session)) {
                let cleaned = clear_owned(dir, session)
                    .and_then(|()| remove_if_present(&marker))
                    .and_then(|()| remove_if_present(&path))
                    .and_then(|()| sync_directory(dir));
                if let Err(error) = cleaned {
                    report.issues.push(error.to_string());
                }
                continue;
            }
            report.issues.push(format!(
                "Recovery discard marker for {session} is invalid; data retained"
            ));
        }
        match scan_session(dir, session, &mut report.issues) {
            Ok(Some(found)) => report.found.push(found),
            Ok(None) => {
                if owned_paths(dir, session).is_ok_and(|paths| {
                    paths.iter().all(|p| {
                        p == &path
                            || p.extension().is_some_and(|e| e == "json")
                                && p.file_name().and_then(|n| n.to_str())
                                    == Some(format!("{session}.json").as_str())
                    })
                }) {
                    let _ = remove_if_present(&dir.join(format!("{session}.json")));
                    let _ = remove_if_present(&path);
                } else {
                    report.issues.push(format!("Recovery files for {session} have no valid checkpoint; retained for inspection"));
                }
            }
            Err(error) => report.issues.push(format!("Recovery {session}: {error}")),
        }
    }
    report.found.sort_by_key(|f| f.modified);
    report
}

pub(super) fn load_commit(dir: &Path, session: Uuid, sequence: u64) -> Result<(Commit, String)> {
    let bytes = read_bounded(&commit_path(dir, session, sequence), RECORD_MAX)?;
    let commit = Commit::decode(&bytes)?;
    if commit.sequence != sequence {
        return Err(journal::invalid("recovery sequence/path mismatch"));
    }
    Ok((commit, hash(&bytes)))
}
fn found_commit(dir: &Path, session: Uuid, commit: &Commit) -> Result<Found> {
    commit.validate_payload(dir, session)?;
    let checkpoint = payload_path(dir, session, commit.payload);
    let modified = fs::metadata(&checkpoint)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    Ok(Found {
        session,
        checkpoint,
        original: commit.original.clone(),
        modified,
        sequence: commit.sequence,
    })
}
fn scan_session(dir: &Path, session: Uuid, issues: &mut Vec<String>) -> Result<Option<Found>> {
    let mut anchors = Vec::new();
    for slot in 0..2 {
        let path = head_path(dir, session, slot);
        if fs::symlink_metadata(&path).is_err() {
            continue;
        }
        let anchor = (|| {
            let head = Head::decode(&read_bounded(&path, RECORD_MAX)?)?;
            let (commit, digest) = load_commit(dir, session, head.sequence)?;
            if digest != head.commit_sha256 {
                return Err(journal::invalid("recovery head/commit checksum mismatch"));
            }
            let found = found_commit(dir, session, &commit)?;
            Ok((found, digest))
        })();
        match anchor {
            Ok(anchor) => anchors.push(anchor),
            Err(error) => issues.push(format!("Recovery {session} head {slot}: {error}")),
        }
    }
    anchors.sort_by_key(|(found, _)| found.sequence);
    let (mut found, mut previous, mut sequence) = if let Some((found, digest)) = anchors.pop() {
        let next = found.sequence.checked_add(1);
        (Some(found), Some(digest), next)
    } else {
        (None, None, Some(1))
    };
    for _ in 0..TAIL_RECORDS_MAX {
        let Some(next) = sequence else {
            break;
        };
        if fs::symlink_metadata(commit_path(dir, session, next)).is_err() {
            break;
        }
        let advanced = (|| {
            let (commit, digest) = load_commit(dir, session, next)?;
            if commit.previous != previous {
                return Err(journal::invalid("recovery journal chain is discontinuous"));
            }
            Ok((found_commit(dir, session, &commit)?, digest))
        })();
        match advanced {
            Ok((new_found, digest)) => {
                found = Some(new_found);
                previous = Some(digest);
                sequence = next.checked_add(1);
            }
            Err(error) => {
                issues.push(format!(
                    "Recovery {session} stopped at journal {next}: {error}"
                ));
                break;
            }
        }
    }
    if found.is_some() {
        return Ok(found);
    }
    // Preserve recovery from builds predating the journal. Legacy bytes are still
    // admitted and semantically validated, rather than trusted because of a suffix.
    let legacy = dir.join(format!("{session}.pcnd"));
    if fs::symlink_metadata(&legacy).is_ok() {
        crate::pcnd::read(&read_bounded(&legacy, SNAPSHOT_BYTES_MAX)?)?;
        let original = read_bounded(&dir.join(format!("{session}.json")), RECORD_MAX)
            .ok()
            .and_then(|b| json::parse(&b).ok())
            .and_then(|v| v.get("original").and_then(Value::as_str).map(str::to_owned));
        return Ok(Some(Found {
            session,
            checkpoint: legacy.clone(),
            original,
            modified: fs::metadata(legacy)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH),
            sequence: 0,
        }));
    }
    Ok(None)
}

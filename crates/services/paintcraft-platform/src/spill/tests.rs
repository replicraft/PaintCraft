use super::*;
fn store(quota: u64, target: u64) -> (PathBuf, NativeSpillStore) {
    let parent = std::env::temp_dir().join(format!("paintcraft-spill-test-{}", Uuid::new_v4()));
    let store = NativeSpillStore::new(&parent, quota, target).unwrap();
    (parent, store)
}
#[test]
fn append_extents_are_checked_and_live_leases_prevent_gc_and_abandoned_cleanup() {
    let (parent, store) = store(1 << 20, 1024);
    let a = store.write(&[1; 100]).unwrap();
    let b = store.write(&[2; 100]).unwrap();
    store.flush().unwrap();
    assert_eq!(store.usage().segments, 1);
    assert_eq!(store.collect_garbage().unwrap().reserved_bytes, 296);
    assert_eq!(remove_abandoned(&parent).unwrap(), 0);
    drop(a);
    let path = store.directory().to_owned();
    drop(store);
    assert!(path.exists());
    let mut out = [0; 100];
    b.read_into(&mut out).unwrap();
    assert_eq!(out, [2; 100]);
    assert_eq!(remove_abandoned(&parent).unwrap(), 0);
    drop(b);
    assert!(!path.exists());
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn quota_admission_preserves_old_extents_and_releases_dead_segments() {
    let (parent, store) = store(200, 100);
    let a = store.write(&[3; 100]).unwrap();
    assert_eq!(
        store.write(&[4; 100]).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let mut out = [0; 100];
    a.read_into(&mut out).unwrap();
    assert_eq!(out, [3; 100]);
    drop(a);
    assert_eq!(store.collect_garbage().unwrap().reserved_bytes, 0);
    store.write(&[5; 100]).unwrap();
    assert_eq!(store.collect_garbage().unwrap().reserved_bytes, 0);
    drop(store);
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn corrupt_short_and_missing_extents_fail_without_changing_other_segments() {
    for mode in 0..3 {
        let (parent, store) = store(1 << 20, 100);
        let a = store.write(&[1; 100]).unwrap();
        let b = store.write(&[2; 100]).unwrap();
        store.collect_garbage().unwrap();
        let mut paths: Vec<_> = fs::read_dir(store.directory())
            .unwrap()
            .map(|p| p.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "segment"))
            .collect();
        paths.sort();
        // Identify the extent by reading the first payload byte.
        let path = paths
            .into_iter()
            .find(|p| fs::read(p).unwrap()[48] == 1)
            .unwrap();
        match mode {
            0 => {
                let mut f = File::options().write(true).open(&path).unwrap();
                f.seek(SeekFrom::Start(48)).unwrap();
                f.write_all(&[9]).unwrap();
            }
            1 => {
                File::options()
                    .write(true)
                    .open(&path)
                    .unwrap()
                    .set_len(60)
                    .unwrap();
            }
            _ => {
                fs::remove_file(path).unwrap();
            }
        }
        assert!(a.read_into(&mut [0; 100]).is_err());
        let mut out = [0; 100];
        b.read_into(&mut out).unwrap();
        assert_eq!(out, [2; 100]);
        drop((a, b, store));
        fs::remove_dir_all(parent).unwrap();
    }
}
#[test]
fn marked_unlocked_orphans_are_removed_but_unmarked_directories_remain() {
    let parent = std::env::temp_dir().join(format!("paintcraft-spill-orphans-{}", Uuid::new_v4()));
    fs::create_dir(&parent).unwrap();
    for marked in [false, true] {
        let path = parent.join(format!("session-{}", Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        fs::write(
            path.join("owner.lock"),
            if marked { MAGIC.as_slice() } else { b"unknown" },
        )
        .unwrap();
    }
    assert_eq!(remove_abandoned(&parent).unwrap(), 1);
    assert_eq!(fs::read_dir(&parent).unwrap().count(), 1);
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn concurrent_readers_and_appends_keep_extents_immutable() {
    let (parent, store) = store(1 << 20, 1024);
    let store = Arc::new(store);
    let a = store.write(&[7; 100]).unwrap();
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let a = a.clone();
            scope.spawn(move || {
                for _ in 0..100 {
                    let mut out = [0; 100];
                    a.read_into(&mut out).unwrap();
                    assert_eq!(out, [7; 100]);
                }
            });
        }
        for i in 0..50 {
            let b = store.write(&[i; 100]).unwrap();
            let mut out = [0; 100];
            b.read_into(&mut out).unwrap();
            assert_eq!(out, [i; 100]);
        }
    });
    drop(a);
    assert_eq!(store.collect_garbage().unwrap().reserved_bytes, 0);
    drop(store);
    fs::remove_dir_all(parent).unwrap();
}

#[cfg(unix)]
#[test]
fn permission_denial_does_not_publish_an_extent_or_poison_retry() {
    use std::os::unix::fs::PermissionsExt;
    let (parent, store) = store(1 << 20, 1024);
    fs::set_permissions(store.directory(), fs::Permissions::from_mode(0o500)).unwrap();
    let result = store.write(&[1; 100]);
    fs::set_permissions(store.directory(), fs::Permissions::from_mode(0o700)).unwrap();
    if let Err(error) = result {
        assert_eq!(error.code, ErrorCode::StorageFailed);
        assert_eq!(store.usage().reserved_bytes, 0);
    } else {
        eprintln!("Host bypasses Unix write permissions; denial was not exercised");
    }
    let blob = store.write(&[2; 100]).unwrap();
    let mut out = [0; 100];
    blob.read_into(&mut out).unwrap();
    assert_eq!(out, [2; 100]);
    drop((blob, store));
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn killed_process_releases_owner_lock_and_only_abandoned_data_is_removed() {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let parent = std::env::temp_dir().join(format!("paintcraft-spill-crash-{}", Uuid::new_v4()));
    fs::create_dir(&parent).unwrap();
    let mut child = Child(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "spill::tests::crash_child", "--nocapture"])
            .env("PAINTCRAFT_SPILL_CHILD", &parent)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let start = Instant::now();
    while !parent.join("ready").exists() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "child did not start"
        );
        assert!(child.0.try_wait().unwrap().is_none(), "child exited early");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(remove_abandoned(&parent).unwrap(), 0);
    child.0.kill().unwrap();
    assert!(!child.0.wait().unwrap().success());
    assert_eq!(remove_abandoned(&parent).unwrap(), 1);
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn crash_child() {
    let Some(parent) = std::env::var_os("PAINTCRAFT_SPILL_CHILD") else {
        return;
    };
    let store = NativeSpillStore::new(Path::new(&parent), 1 << 20, 1024).unwrap();
    let lease = store.write(&[9; 100]).unwrap();
    store.flush().unwrap();
    fs::write(Path::new(&parent).join("ready"), b"ready").unwrap();
    loop {
        std::hint::black_box(&lease);
        std::thread::park();
    }
}

#[test]
fn partial_disk_full_and_failed_rollback_preserve_previously_published_extents() {
    use std::sync::atomic::Ordering;
    for rollback_fails in [false, true] {
        let (parent, store) = store(1 << 20, 1024);
        let a = store.write(&[3; 100]).unwrap();
        store.flush().unwrap();
        store.fail_body.store(true, Ordering::SeqCst);
        store.fail_rollback.store(rollback_fails, Ordering::SeqCst);
        let error = store.write(&[4; 100]).unwrap_err();
        assert_eq!(error.code, ErrorCode::StorageFailed);
        assert_eq!(
            store.usage().reserved_bytes,
            if rollback_fails { 296 } else { 148 }
        );
        let mut out = [0; 100];
        a.read_into(&mut out).unwrap();
        assert_eq!(out, [3; 100]);
        let b = store.write(&[5; 100]).unwrap();
        b.read_into(&mut out).unwrap();
        assert_eq!(out, [5; 100]);
        drop((a, b));
        assert_eq!(store.collect_garbage().unwrap().reserved_bytes, 0);
        drop(store);
        fs::remove_dir_all(parent).unwrap();
    }
}
#[cfg(unix)]
#[test]
fn failed_gc_keeps_quota_reserved_until_the_file_can_be_removed() {
    use std::os::unix::fs::PermissionsExt;
    let (parent, store) = store(1 << 20, 1024);
    let blob = store.write(&[3; 100]).unwrap();
    store.collect_garbage().unwrap();
    fs::set_permissions(store.directory(), fs::Permissions::from_mode(0o500)).unwrap();
    drop(blob);
    let usage = store.usage();
    if usage.reserved_bytes != 0 {
        assert_eq!(usage.reserved_bytes, 148);
        assert!(usage.cleanup_failures > 0);
        assert!(store.collect_garbage().is_err());
    }
    fs::set_permissions(store.directory(), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(store.collect_garbage().unwrap().reserved_bytes, 0);
    drop(store);
    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn synchronization_failure_does_not_invalidate_an_earlier_published_prefix() {
    use std::sync::atomic::Ordering;
    let (parent, store) = store(1 << 20, 1024);
    let a = store.write(&[3; 100]).unwrap();
    store.flush().unwrap();
    let b = store.write(&[4; 100]).unwrap();
    store.fail_flush.store(true, Ordering::SeqCst);
    assert_eq!(store.flush().unwrap_err().code, ErrorCode::StorageFailed);
    let mut out = [0; 100];
    a.read_into(&mut out).unwrap();
    assert_eq!(out, [3; 100]);
    drop(b);
    let c = store.write(&[5; 100]).unwrap();
    store.flush().unwrap();
    c.read_into(&mut out).unwrap();
    assert_eq!(out, [5; 100]);
    drop((a, c));
    assert_eq!(store.collect_garbage().unwrap().reserved_bytes, 0);
    drop(store);
    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn a_closed_provisional_segment_still_requires_the_publication_barrier() {
    use std::sync::atomic::Ordering;
    let (parent, store) = store(1 << 20, 1024);
    let a = store.write(&[3; 100]).unwrap();
    store.collect_garbage().unwrap();
    store.fail_flush.store(true, Ordering::SeqCst);
    assert_eq!(store.flush().unwrap_err().code, ErrorCode::StorageFailed);
    store.flush().unwrap();
    let mut out = [0; 100];
    a.read_into(&mut out).unwrap();
    assert_eq!(out, [3; 100]);
    drop(a);
    assert_eq!(store.collect_garbage().unwrap().reserved_bytes, 0);
    drop(store);
    fs::remove_dir_all(parent).unwrap();
}

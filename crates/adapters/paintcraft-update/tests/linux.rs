//! AppImage swap and the relaunch helper, on temp files.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::time::{Duration, Instant};

use paintcraft_update::apply::linux::swap;
use paintcraft_update::helper::{encoded_command, relaunch_exec};

mod support;
use support::{tempdir, write};

fn mode(path: &std::path::Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o7777
}

#[test]
fn swap_renames_over_the_old_file_keeping_its_mode_plus_owner_execute() {
    let dir = tempdir();
    let target = write(dir.path().join("PaintCraft.AppImage"), b"old");
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o750)).unwrap();
    let payload = write(dir.path().join("download"), b"new");
    std::fs::set_permissions(&payload, std::fs::Permissions::from_mode(0o600)).unwrap();
    swap(&payload, &target).unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"new");
    assert_eq!(mode(&target), 0o750);
    assert!(!payload.exists());
}

#[test]
fn an_old_file_without_execute_gets_owner_execute_and_no_special_bits() {
    let dir = tempdir();
    let target = write(dir.path().join("PaintCraft.AppImage"), b"old");
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o644)).unwrap();
    let payload = write(dir.path().join("download"), b"new");
    std::fs::set_permissions(&payload, std::fs::Permissions::from_mode(0o4777)).unwrap();
    swap(&payload, &target).unwrap();
    assert_eq!(mode(&target), 0o744);
}

#[test]
fn a_missing_target_is_an_error_and_leaves_the_payload() {
    let dir = tempdir();
    let payload = write(dir.path().join("download"), b"new");
    assert!(swap(&payload, &dir.path().join("gone")).is_err());
    assert!(payload.exists());
}

#[test]
fn the_helper_waits_for_the_process_to_exit_then_starts_the_program() {
    let dir = tempdir();
    let marker = dir.path().join("started");
    let script = write(
        dir.path().join("new.AppImage"),
        format!(
            "#!/bin/sh\necho \"${{APPIMAGE-unset}}\" > '{}'\n",
            marker.display()
        )
        .as_bytes(),
    );
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut child = Command::new("sleep").arg("1").spawn().unwrap();

    relaunch_exec(child.id(), &script).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    assert!(!marker.exists(), "must wait while the process lives");
    child.wait().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !marker.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(std::fs::read_to_string(&marker).unwrap().trim(), "unset");
}

#[test]
fn a_path_with_shell_metacharacters_is_just_a_path() {
    let dir = tempdir();
    let nasty = dir.path().join("a b'; touch pwned; '$(touch pwned2)");
    std::fs::create_dir(&nasty).unwrap();
    let script = write(
        nasty.join("run"),
        format!("#!/bin/sh\necho ok > '{}/ok'\n", dir.path().display()).as_bytes(),
    );
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut child = Command::new("true").spawn().unwrap();
    child.wait().unwrap();
    relaunch_exec(child.id(), &script).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !dir.path().join("ok").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(dir.path().join("ok").exists());
    assert!(!dir.path().join("pwned").exists() && !nasty.join("pwned").exists());
    let _ = encoded_command;
}

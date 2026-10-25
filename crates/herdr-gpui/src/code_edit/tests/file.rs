use crate::code_edit::{Error, file};
use std::fs;

fn folder() -> tempfile::TempDir {
    tempfile::Builder::new().prefix("edit").tempdir().unwrap()
}

#[test]
fn crlf_files_edit_with_newlines_and_save_with_crlf() {
    let folder = folder();
    let path = folder.path().join("a.txt");
    fs::write(&path, "one\r\ntwo\r\n").unwrap();
    let loaded = file::load(&path).unwrap();
    assert_eq!(loaded.text, "one\ntwo\n");
    assert!(loaded.crlf);
    let saved = file::save(&path, &loaded, "one\nthree\n", false).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "one\r\nthree\r\n");
    assert_eq!(saved.text, "one\nthree\n");
}

#[test]
fn saving_refuses_a_file_changed_since_it_was_read() {
    let folder = folder();
    let path = folder.path().join("a.txt");
    fs::write(&path, "mine").unwrap();
    let loaded = file::load(&path).unwrap();
    fs::write(&path, "theirs").unwrap();
    let error = file::save(&path, &loaded, "edited", false).unwrap_err();
    assert!(matches!(error, Error::Changed));
    assert_eq!(fs::read_to_string(&path).unwrap(), "theirs");
    file::save(&path, &loaded, "edited", true).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "edited");
}

#[cfg(unix)]
#[test]
fn saving_keeps_the_file_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let folder = folder();
    let path = folder.path().join("run.sh");
    fs::write(&path, "echo").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    let loaded = file::load(&path).unwrap();
    file::save(&path, &loaded, "echo hi", false).unwrap();
    let mode = fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o755);
}

#[test]
fn very_long_lines_are_refused() {
    let folder = folder();
    let path = folder.path().join("min.js");
    fs::write(&path, "x".repeat(20_000)).unwrap();
    assert!(matches!(file::load(&path), Err(Error::LongLines)));
}

#[test]
fn binary_files_are_refused() {
    let folder = folder();
    let path = folder.path().join("a.bin");
    fs::write(&path, b"a\0b").unwrap();
    assert!(matches!(file::load(&path), Err(Error::Unreadable)));
}

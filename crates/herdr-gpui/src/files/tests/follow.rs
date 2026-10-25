use super::*;

fn read(folder: &str, here: Option<&str>) -> Read {
    Read {
        folder: folder.into(),
        listing: Ok((listing(&[2]), here.map(str::to_owned))),
    }
}

#[test]
fn turning_to_a_folder_opens_the_tree_down_to_it() {
    let mut files = Files::default();
    assert!(files.follow(Some("/repo/crates/app/src".into())));
    files.install(
        read("/repo/crates/app/src", Some("crates/app/src")),
        Instant::now(),
    );
    assert!(matches!(files.status, Status::Ready));
    assert_eq!(
        shown(&files),
        [
            "crates",
            "  app",
            "    src",
            "      ui",
            "      main.rs",
            "    Cargo.toml",
            "docs",
            "Cargo.toml",
            "README.md",
        ]
    );
    // A later read of the same folder keeps what the user closed.
    files.toggle("crates");
    files.install(
        read("/repo/crates/app/src", Some("crates/app/src")),
        Instant::now(),
    );
    assert_eq!(shown(&files), ["crates", "docs", "Cargo.toml", "README.md"]);
}

#[test]
fn a_read_for_a_folder_left_behind_is_dropped() {
    let mut files = Files::default();
    files.follow(Some("/repo/docs".into()));
    files.follow(Some("/repo/crates".into()));
    files.install(read("/repo/docs", Some("docs")), Instant::now());
    assert!(files.listing.is_none());
    // The folder followed now is still due at once.
    assert!(files.due(Instant::now()));
}

#[test]
fn pinning_holds_the_tree_while_the_focus_moves() {
    let mut files = Files::default();
    files.follow(Some("/repo".into()));
    files.pinned = true;
    assert!(!files.follow(Some("/elsewhere".into())));
    assert_eq!(files.followed(), Some("/repo"));
    files.pinned = false;
    assert!(files.follow(Some("/elsewhere".into())));
}

#[test]
fn the_changed_files_lead_newest_first_up_to_a_bound() {
    let mut files = Files::default();
    files.follow(Some("/repo".into()));
    files.install(read("/repo", None), Instant::now());
    assert_eq!(files.recent().len(), 1);
    assert_eq!(files.recent()[0].file, 2);
}

#[test]
fn a_failed_read_says_why_and_clears_the_tree() {
    let mut files = Files::default();
    files.follow(Some("/repo".into()));
    files.install(read("/repo", None), Instant::now());
    files.install(
        Read {
            folder: "/repo".into(),
            listing: Err(herdr_vcs::Error::from(herdr_vcs::code_index::Error::Root).into()),
        },
        Instant::now(),
    );
    assert!(matches!(files.status, Status::Failed(_)));
    assert!(files.rows.is_empty());
}

#[test]
fn a_folder_is_placed_relative_to_its_checkout() {
    let dir = tempfile::tempdir().unwrap();
    let inner = dir.path().join("a/b");
    std::fs::create_dir_all(&inner).unwrap();
    assert_eq!(relative(dir.path(), &inner).as_deref(), Some("a/b"));
    assert_eq!(relative(dir.path(), dir.path()), None);
    assert_eq!(relative(&inner, dir.path()), None);
}

#[test]
fn a_checkout_is_read_from_a_folder_inside_it() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
    };
    git(&["init", "-q"]);
    std::fs::create_dir_all(root.join("src/ui")).unwrap();
    std::fs::write(root.join("src/ui/view.rs"), "fn view() {}\n").unwrap();
    std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
    std::fs::create_dir_all(root.join("target")).unwrap();
    std::fs::write(root.join("target/out"), "").unwrap();
    let folder = root.join("src").to_string_lossy().into_owned();
    let (listing, here) = crate::files::read(&folder, &AtomicBool::new(false)).unwrap();
    assert_eq!(here.as_deref(), Some("src"));
    assert_eq!(listing.index.files(), [".gitignore", "src/ui/view.rs"]);
}

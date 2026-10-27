use super::*;

fn ready() -> Files {
    let mut files = Files::default();
    files.follow(Some("/repo".into()));
    files.install(
        Read {
            folder: "/repo".into(),
            listing: Ok((listing(&[]), None)),
        },
        Instant::now(),
    );
    files
}

#[test]
fn a_query_shows_the_matching_files_inside_their_open_folders() {
    let mut files = ready();
    files.set_query("  .RS ");
    assert!(files.filtering());
    assert_eq!(
        shown(&files),
        [
            "crates",
            "  app",
            "    src",
            "      ui",
            "        view.rs",
            "      main.rs",
        ]
    );
    assert!(files.is_open("crates/app"));
}

#[test]
fn a_query_with_a_slash_matches_the_whole_path() {
    let mut files = ready();
    files.set_query("app/cargo");
    assert_eq!(shown(&files), ["crates", "  app", "    Cargo.toml"]);
    // Without one only names count, so the folder `docs` matches nothing.
    files.set_query("docs");
    assert!(files.rows.is_empty());
}

#[test]
fn folders_keep_their_state_while_filtering_and_clearing_reveals_the_opened_file() {
    let mut files = ready();
    files.set_query("guide");
    files.toggle("docs");
    assert_eq!(shown(&files), ["docs", "  guide.md"]);
    files.selected = Some("docs/guide.md".into());
    files.set_query("");
    assert!(!files.filtering());
    assert_eq!(
        shown(&files),
        ["crates", "docs", "  guide.md", "Cargo.toml", "README.md"]
    );
}

#[test]
fn a_new_read_keeps_the_filter() {
    let mut files = ready();
    files.set_query("readme");
    files.install(
        Read {
            folder: "/repo".into(),
            listing: Ok((listing(&[1]), None)),
        },
        Instant::now(),
    );
    assert_eq!(shown(&files), ["README.md"]);
}

use super::*;
use crate::config::preferences::Preference;

#[test]
fn pull_requests_and_issues_are_on_unless_turned_off() -> anyhow::Result<()> {
    for config in [Config::default(), Config::parse(DEFAULT_CONFIG)?] {
        assert!(config.sidebar_style.pull_requests);
        assert!(config.sidebar_style.issues);
    }
    let off = Config::parse("[sidebar]\npull_requests = false\nissues = false")?;
    assert!(!off.sidebar_style.pull_requests);
    assert!(!off.sidebar_style.issues);
    assert!(Config::parse("[sidebar]\npull_requests = \"off\"").is_err());
    assert!(Config::parse("[sidebar]\nissues = \"off\"").is_err());
    Ok(())
}

#[test]
fn turning_pull_requests_or_issues_off_writes_its_own_key() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("local.toml");
    fs::write(&path, "[sidebar]\nactivity_bar = true\n")?;
    for shown in [false, true] {
        Config::save_preference_path(Preference::SidebarPullRequests(shown), &path)?;
        let config = Config::parse(&fs::read_to_string(&path)?)?;
        assert_eq!(config.sidebar_style.pull_requests, shown);
        // Issues still holds what the previous round wrote.
        assert_eq!(config.sidebar_style.issues, !shown);

        Config::save_preference_path(Preference::SidebarIssues(shown), &path)?;
        let text = fs::read_to_string(&path)?;
        let config = Config::parse(&text)?;
        assert_eq!(config.sidebar_style.issues, shown);
        assert_eq!(config.sidebar_style.pull_requests, shown);
        assert!(config.sidebar_style.activity_bar, "{text}");
    }
    Ok(())
}

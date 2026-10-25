use super::*;
use crate::config::preferences::Preference;

#[test]
fn security_is_on_unless_turned_off() -> anyhow::Result<()> {
    for config in [Config::default(), Config::parse(DEFAULT_CONFIG)?] {
        assert!(config.sidebar_style.security);
    }
    let off = Config::parse("[sidebar]\nsecurity = false")?;
    assert!(!off.sidebar_style.security);
    assert!(off.sidebar_style.issues && off.sidebar_style.automations);
    assert!(Config::parse("[sidebar]\nsecurity = \"off\"").is_err());
    Ok(())
}

#[test]
fn turning_security_off_writes_its_own_key() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("local.toml");
    fs::write(&path, "[sidebar]\nissues = false\n")?;
    for shown in [false, true] {
        Config::save_preference_path(Preference::SidebarSecurity(shown), &path)?;
        let text = fs::read_to_string(&path)?;
        let config = Config::parse(&text)?;
        assert_eq!(config.sidebar_style.security, shown, "{text}");
        assert!(!config.sidebar_style.issues, "{text}");
    }
    Ok(())
}

use super::*;
use crate::plus_command::PlusCommand;

#[test]
fn plus_runs_a_command_only_when_configured() -> anyhow::Result<()> {
    for config in [Config::parse("")?, Config::parse(DEFAULT_CONFIG)?] {
        assert_eq!(config.plus_command, None);
    }
    assert_eq!(
        Config::parse("plus_command = \"claude --continue\"")?.plus_command,
        Some(PlusCommand::try_from("claude --continue".to_owned())?)
    );
    assert!(Config::parse("plus_command = \"\"").is_err());
    assert!(Config::parse("plus_command = \"claude\\nexit\"").is_err());
    Ok(())
}

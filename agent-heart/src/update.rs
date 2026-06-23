use agent_body_core::github_release::run_organ_self_update;
use anyhow::Result;

const REPO: &str = "autonomic-ai-dev/agent-heart";
const BINARY: &str = "agent-heart";

pub fn run_update(force: bool) -> Result<bool> {
    if !force && !agent_body_core::should_update_binary(BINARY, false).unwrap_or(true) {
        println!("update disabled in ~/.autonomic/config.toml — use --force to override");
        return Ok(false);
    }
    run_organ_self_update(REPO, BINARY, env!("CARGO_PKG_VERSION"), force)
}

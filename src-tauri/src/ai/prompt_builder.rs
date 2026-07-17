//! Load and assemble agent system prompts from markdown files.

use std::path::PathBuf;
use std::sync::OnceLock;

use super::response_schema::ToolDefinition;

pub const PROMPT_VERSION: &str = "coreside-prompt-v1";

#[derive(Debug, Clone)]
pub struct PromptBundle {
    pub system: String,
    pub tool_builder: String,
    pub tool_editor: String,
    pub response_rules: String,
}

static PROMPTS: OnceLock<PromptBundle> = OnceLock::new();

fn prompts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("prompts")
}

fn read_prompt(name: &str) -> String {
    let path = prompts_dir().join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        tracing::warn!(path = %path.display(), error = %e, "missing prompt file");
        String::new()
    })
}

pub fn load_prompts() -> PromptBundle {
    PROMPTS
        .get_or_init(|| PromptBundle {
            system: read_prompt("system.md"),
            tool_builder: read_prompt("tool_builder.md"),
            tool_editor: read_prompt("tool_editor.md"),
            response_rules: read_prompt("response_rules.md"),
        })
        .clone()
}

/// Assemble the full system prompt for an agent turn.
pub fn build_agent_prompt(
    active_tool: Option<&ToolDefinition>,
    workspace_hint: Option<&str>,
) -> String {
    let prompts = load_prompts();
    let mut parts = Vec::new();

    parts.push(format!("# Prompt version: {PROMPT_VERSION}"));
    parts.push(prompts.system);
    parts.push(prompts.response_rules);

    if let Some(tool) = active_tool {
        parts.push(prompts.tool_editor);
        parts.push("## Active tool (JSON)".to_string());
        parts.push(
            serde_json::to_string_pretty(tool)
                .unwrap_or_else(|_| "{}".to_string()),
        );
    } else {
        parts.push(prompts.tool_builder);
    }

    if let Some(hint) = workspace_hint {
        parts.push(format!("## Workspace context\n{hint}"));
    }

    parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_includes_version() {
        let p = build_agent_prompt(None, None);
        assert!(p.contains(PROMPT_VERSION));
    }
}

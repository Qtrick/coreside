//! Load and assemble agent system prompts from markdown files.

use std::path::PathBuf;
use std::sync::OnceLock;

use super::response_schema::ToolDefinition;
use super::capability_registry::capability_schemas;
use crate::exa::has_exa_key;
use crate::research::research_capability_notice;

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

fn protected_resources_prompt() -> String {
    read_prompt("protected_resources.md")
}

/// Assemble the full system prompt for an agent turn.
pub fn build_agent_prompt(
    active_tool: Option<&ToolDefinition>,
    workspace_hint: Option<&str>,
) -> String {
    build_agent_prompt_with_references(active_tool, &[], workspace_hint, None)
}

/// Optional project-scoped context injected before the model turn.
#[derive(Debug, Clone)]
pub struct ProjectPromptContext {
    pub project_name: String,
    pub instructions: Option<String>,
    pub summary: Option<String>,
    pub retrieval_snippets: Vec<String>,
}

/// Prompt with explicitly `@`-referenced tools (takes precedence for context).
pub fn build_agent_prompt_with_references(
    active_tool: Option<&ToolDefinition>,
    referenced_tools: &[ToolDefinition],
    workspace_hint: Option<&str>,
    project_context: Option<&ProjectPromptContext>,
) -> String {
    let prompts = load_prompts();
    let mut parts = Vec::new();

    parts.push(format!("# Prompt version: {PROMPT_VERSION}"));
    parts.push(prompts.system);
    parts.push(prompts.response_rules);
    parts.push(protected_resources_prompt());

    if !referenced_tools.is_empty() {
        parts.push(prompts.tool_editor);
        parts.push("## Explicitly referenced tools (JSON)".to_string());
        parts.push(
            serde_json::to_string_pretty(referenced_tools)
                .unwrap_or_else(|_| "[]".to_string()),
        );
        if let Some(tool) = active_tool {
            if !referenced_tools.iter().any(|t| t.id == tool.id) {
                parts.push("## Active tool (JSON)".to_string());
                parts.push(
                    serde_json::to_string_pretty(tool)
                        .unwrap_or_else(|_| "{}".to_string()),
                );
            }
        }
    } else if let Some(tool) = active_tool {
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

    if let Some(ctx) = project_context {
        let mut block = format!("## Project context: {}\n", ctx.project_name);
        if let Some(instr) = ctx.instructions.as_ref().filter(|s| !s.trim().is_empty()) {
            block.push_str(&format!("\n### Instructions\n{instr}\n"));
        }
        if let Some(summary) = ctx.summary.as_ref().filter(|s| !s.trim().is_empty()) {
            block.push_str(&format!("\n### Summary\n{summary}\n"));
        }
        if !ctx.retrieval_snippets.is_empty() {
            block.push_str("\n### Relevant prior messages\n");
            for line in &ctx.retrieval_snippets {
                block.push_str(&format!("- {line}\n"));
            }
        }
        parts.push(block);
    }

    parts.push("## Agent tool capabilities (tool_use)".to_string());
    parts.push(
        serde_json::to_string_pretty(&capability_schemas())
            .unwrap_or_else(|_| "[]".to_string()),
    );
    parts.push(format!(
        "## Web research capability\n{}",
        research_capability_notice(has_exa_key())
    ));
    parts.push(
        "When search or project context is needed, respond with responseType \"tool_use\" and toolCalls. \
         After tools run, respond with responseType \"message\" and optional citations."
            .to_string(),
    );

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

    #[test]
    fn prompt_includes_protected_resources() {
        let p = build_agent_prompt(None, None);
        assert!(p.contains("Protected Core Resources") || p.contains("core.branding"));
        assert!(p.contains("core.settings.agent.action_log") || p.contains("actionLogEnabled"));
    }

    #[test]
    fn prompt_with_project_context() {
        let ctx = ProjectPromptContext {
            project_name: "Research".into(),
            instructions: Some("Focus on Rust.".into()),
            summary: Some("Prior work on async.".into()),
            retrieval_snippets: vec!["[Chat A] user: tokio runtime".into()],
        };
        let p = build_agent_prompt_with_references(None, &[], None, Some(&ctx));
        assert!(p.contains("Project context: Research"));
        assert!(p.contains("Focus on Rust"));
        assert!(p.contains("tokio runtime"));
    }

    #[test]
    fn prompt_with_references_lists_tools() {
        use super::super::response_schema::ToolDefinition;
        use serde_json::json;
        let tool = ToolDefinition {
            id: "water-tracker".into(),
            name: "Water Tracker".into(),
            description: "hydration".into(),
            layout: json!({ "type": "single-column" }),
            components: vec![],
        };
        let p = build_agent_prompt_with_references(None, &[tool], None, None);
        assert!(p.contains("Explicitly referenced tools"));
        assert!(p.contains("water-tracker"));
        assert!(p.contains("Water Tracker"));
    }

    #[test]
    fn prompt_includes_web_research_capability() {
        let p = build_agent_prompt(None, None);
        assert!(p.contains("Web research capability"));
    }
}

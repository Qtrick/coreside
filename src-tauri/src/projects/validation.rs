const MIN_NAME_LEN: usize = 1;
const MAX_NAME_LEN: usize = 80;
const MAX_DESCRIPTION_LEN: usize = 2000;
const MAX_INSTRUCTIONS_LEN: usize = 16000;

const RESERVED_NAMES: &[&str] = &[
    "settings",
    "chats",
    "tools",
    "automations",
    "media",
    "coreside",
];

const ICON_KEYS: &[&str] = &[
    "folder", "book", "leaf", "chart", "code", "heart", "star", "travel", "fitness", "pen",
];

pub fn validate_project_name(name: &str) -> Result<(), String> {
    let trimmed = name.trim();
    let len = trimmed.chars().count();
    if len < MIN_NAME_LEN {
        return Err("Project name is required".into());
    }
    if len > MAX_NAME_LEN {
        return Err(format!("Project name must be at most {MAX_NAME_LEN} characters"));
    }
    let lower = trimmed.to_ascii_lowercase();
    if RESERVED_NAMES.iter().any(|r| *r == lower) {
        return Err("That project name is reserved".into());
    }
    if lower.starts_with("core.") {
        return Err("Project names cannot start with core.".into());
    }
    Ok(())
}

pub fn validate_description(description: Option<&str>) -> Result<(), String> {
    if let Some(text) = description {
        if text.chars().count() > MAX_DESCRIPTION_LEN {
            return Err(format!(
                "Description must be at most {MAX_DESCRIPTION_LEN} characters"
            ));
        }
    }
    Ok(())
}

pub fn validate_instructions(instructions: Option<&str>) -> Result<(), String> {
    if let Some(text) = instructions {
        if text.chars().count() > MAX_INSTRUCTIONS_LEN {
            return Err(format!(
                "Instructions must be at most {MAX_INSTRUCTIONS_LEN} characters"
            ));
        }
    }
    Ok(())
}

pub fn validate_icon_key(icon_key: Option<&str>) -> Result<(), String> {
    match icon_key {
        None => Ok(()),
        Some(key) => {
            let trimmed = key.trim();
            if trimmed.is_empty() {
                return Ok(());
            }
            if ICON_KEYS.iter().any(|k| *k == trimmed) {
                Ok(())
            } else {
                Err(format!("Invalid icon key: {trimmed}"))
            }
        }
    }
}

pub fn validate_create_input(input: &super::models::CreateProjectInput) -> Result<(), String> {
    validate_project_name(&input.name)?;
    validate_description(input.description.as_deref())?;
    validate_instructions(input.instructions.as_deref())?;
    validate_icon_key(input.icon_key.as_deref())?;
    Ok(())
}

pub fn validate_update_fields(
    name: Option<&str>,
    description: Option<&str>,
    instructions: Option<&str>,
    icon_key: Option<&str>,
) -> Result<(), String> {
    if let Some(n) = name {
        validate_project_name(n)?;
    }
    validate_description(description)?;
    validate_instructions(instructions)?;
    validate_icon_key(icon_key)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::models::CreateProjectInput;

    #[test]
    fn rejects_reserved_name() {
        assert!(validate_project_name("settings").is_err());
        assert!(validate_project_name("CoreSide").is_err());
        assert!(validate_project_name("core.foo").is_err());
    }

    #[test]
    fn accepts_valid_name_and_icon() {
        assert!(validate_project_name("My Project").is_ok());
        assert!(validate_icon_key(Some("folder")).is_ok());
        assert!(validate_icon_key(Some("invalid")).is_err());
    }

    #[test]
    fn enforces_length_limits() {
        let long = "a".repeat(81);
        assert!(validate_project_name(&long).is_err());
        let desc = "d".repeat(2001);
        assert!(validate_description(Some(&desc)).is_err());
    }

    #[test]
    fn validate_create_input_ok() {
        let input = CreateProjectInput {
            name: "Travel".into(),
            description: Some("Notes".into()),
            icon_key: Some("travel".into()),
            instructions: None,
            pinned: None,
        };
        assert!(validate_create_input(&input).is_ok());
    }
}

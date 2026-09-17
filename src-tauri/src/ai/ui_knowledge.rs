//! Persistent Coreside UI Knowledge & Design System Catalog
//!
//! Provides the in-product agent with structured, retrievable guidance on
//! information architecture, layout patterns, state lifecycles, and component
//! compositions without bloating every prompt.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct UiKnowledgeItem {
    pub id: &'static str,
    pub title: &'static str,
    pub category: &'static str,
    pub keywords: &'static [&'static str],
    pub summary: &'static str,
    pub architecture_guidance: &'static str,
}

pub static UI_KNOWLEDGE_BASE: &[UiKnowledgeItem] = &[
    UiKnowledgeItem {
        id: "pattern-research-dashboard",
        title: "Personal Research & Discovery Dashboard",
        category: "archetype",
        keywords: &["research", "dashboard", "search", "papers", "notes", "queue", "stats", "biology", "science"],
        summary: "Multi-section research workspace comprising a query input, filtered results/saved items list, reading queue, and summary statistics.",
        architecture_guidance: r#"### Information Architecture
1. **Header Section (`sec-header`)**: Application title, status indicator, and quick statistics (e.g. total saved, unread count).
2. **Search & Filter Section (`sec-search`)**: Query input, category filter pills, and sort dropdown. Bound to `state.searchQuery` and `state.filterCategory`.
3. **Saved Items / Library Section (`sec-library`)**: Card list or dense table of items with tags, abstract preview, and action buttons ("Mark as Read", "Remove").
4. **Reading Queue Section (`sec-queue`)**: Compact vertical stack of prioritized items for immediate reading.
5. **Notes & Inspector (`sec-notes`)**: Active item notes editor bound to `state.activeNotes` with auto-save.

### State Contracts
- `searchQuery` (string, initial "")
- `selectedCategory` (string, initial "all")
- `readingQueue` (array of objects)
- `activeItem` (object or id)

### Responsive Behavior
- Desktop: 2-column or 3-column grid (search/library on left, queue & inspector on right).
- Mobile/narrow: Stacks vertically; queue and inspector collapse into tabs."#,
    },
    UiKnowledgeItem {
        id: "pattern-settings-editor",
        title: "Settings & Configuration Editor",
        category: "archetype",
        keywords: &["settings", "config", "preferences", "toggle", "form", "options"],
        summary: "Form-driven configuration interface with categorized sections, clear field validation, and explicit safe persistence.",
        architecture_guidance: r#"### Information Architecture
1. **Section Categories**: Group related settings into semantic cards (`sec-general`, `sec-appearance`, `sec-privacy`).
2. **Control Types**: Use boolean toggles for binary options, select menus for enumerated options, and text inputs with helper text for custom values.
3. **Draft & Persistence**: Bind inputs directly to `state.<settingKey>`. Use `tool_state.set` or local state patches.
4. **Destructive Actions**: Place reset or clear data buttons in a distinct "Danger Zone" card with prominent warning badges."#,
    },
    UiKnowledgeItem {
        id: "pattern-multi-step-workflow",
        title: "Multi-Step Task & Automation Workflow",
        category: "archetype",
        keywords: &["workflow", "wizard", "stepper", "multi-step", "process", "pipeline"],
        summary: "Progressive wizard guiding the user through distinct sequential stages with validation gates.",
        architecture_guidance: r#"### Information Architecture
1. **Progress Bar / Stepper (`sec-stepper`)**: Visual indicator of current step index (`state.currentStep`).
2. **Step Content (`sec-step-content`)**: Dynamically shows only components relevant to the active step.
3. **Navigation Controls (`sec-controls`)**: "Back", "Next", and final "Submit/Apply" buttons with disabled states until step validation passes.
4. **Review Step**: Before final execution, render a read-only summary card of all entered parameters."#,
    },
    UiKnowledgeItem {
        id: "pattern-crud-tracker",
        title: "Data Tracker & Record Manager",
        category: "archetype",
        keywords: &["crud", "tracker", "table", "records", "manager", "inventory", "tasks", "list"],
        summary: "Collection manager with search/filter, table or card presentation, creation form, and editing actions.",
        architecture_guidance: r#"### Information Architecture
1. **Toolbar (`sec-toolbar`)**: Search box, filter toggles, and primary "Add New" button.
2. **Data View (`sec-data`)**: Table component with sortable columns or card grid. Every row/card includes action triggers (`onEdit`, `onDelete`).
3. **Empty State**: When no records match, display a helpful illustration or icon, explanatory text, and an "Add First Item" action.
4. **Form Drawer / Modal (`sec-form`)**: Inline form for item creation/edition bound to `state.draftItem`."#,
    },
    UiKnowledgeItem {
        id: "state-lifecycle-rules",
        title: "State & Lifecycle Contract Rules",
        category: "state",
        keywords: &["state", "lifecycle", "loading", "error", "empty", "draft", "preservation"],
        summary: "Core rules for representing transient, persistent, empty, loading, and error states safely.",
        architecture_guidance: r#"### State Lifecycle Rules
1. **Always Handle Four Visual States**:
   - *Loading State*: Display subtle skeleton loader or spinner while async actions execute.
   - *Empty State*: Show friendly message and primary CTA when collections have 0 items.
   - *Error State*: Render dismissible error banner with actionable recovery advice when an action fails.
   - *Populated State*: Normal functional interface.
2. **Preservation Keys**: When replacing or updating component trees, preserve user inputs by maintaining identical `id` and `valueKey` bindings.
3. **Draft Safety**: User form inputs must survive partial layout updates and background patch applications."#,
    },
    UiKnowledgeItem {
        id: "ui-anti-patterns",
        title: "UI Anti-Patterns & Common Pitfalls to Avoid",
        category: "anti-pattern",
        keywords: &["anti-pattern", "card soup", "dead buttons", "overflow", "bloat", "nesting", "bad ui"],
        summary: "Critical mistakes to avoid: card soup, generic dashboard syndrome, dead buttons, and state-destructive edits.",
        architecture_guidance: r#"### Critical Anti-Patterns to Avoid
1. **Card Soup**: Do NOT wrap every single text snippet or stat in an isolated border-card. Group related metrics into cohesive surfaces.
2. **Generic Dashboard Syndrome**: Avoid rendering 10 disconnected placeholder charts without real user workflows or actionable buttons.
3. **Dead Controls**: Every button must bind to an action (`invokeRegisteredAction`) or state change. Do not emit decorative buttons that do nothing.
4. **Layout Overflow & Mobile Clipping**: Do not hardcode fixed pixel widths (>300px) that cause horizontal scrolling in narrow/chat-adjacent panels (~360px-400px).
5. **State-Destructive Updates**: Never replace an entire form tree when the user only asked to change a label or add a field. Use targeted patches to preserve user input.
6. **Excessive Nested Containers**: Keep DOM depth shallow (< 6 levels); avoid `<card><card><container><card>...`."#,
    },
    UiKnowledgeItem {
        id: "responsive-surface-design",
        title: "Responsive Breakpoints & Narrow Surface Guidelines",
        category: "responsive",
        keywords: &["responsive", "mobile", "breakpoints", "narrow", "chat", "width", "layout"],
        summary: "Guidelines for fluid adaptability across chat-adjacent sidebars (~360-400px), tablets (~600-768px), and wide monitors (1000px+).",
        architecture_guidance: r#"### Responsive Layout Breakpoints
1. **Chat-Adjacent / Narrow Panel (~360px - 400px)**:
   - Single-column vertical stack.
   - Collapse grids into vertical cards or compact lists.
   - Ensure touch targets are at least 36px tall with 8px margins.
2. **Compact Tablet (~600px - 768px)**:
   - 2-column layout max.
   - Toolbar wraps or uses icon buttons with accessible aria labels.
3. **Desktop & Wide Surfaces (~1000px+)**:
   - Multi-column dashboards (e.g. sidebar 280px, main canvas flex, inspector 320px).
   - High data density tables with horizontal scrolling only within the table body, not the whole viewport."#,
    },
    UiKnowledgeItem {
        id: "pattern-dense-table",
        title: "Dense Data Tables & Exploration Surfaces",
        category: "archetype",
        keywords: &["table", "data", "dense", "rows", "columns", "sorting", "filter", "pagination"],
        summary: "High-density data presentation with sorting, column alignment, pagination, and multi-row selection.",
        architecture_guidance: r#"### Information Architecture
1. **Table Header Toolbar**: Quick filter search, column visibility toggle, and export or bulk action trigger.
2. **Column Headers**: Clickable sort indicators, explicit text alignment (left for strings, right for numbers/currency).
3. **Pagination / Infinite Scroll Footer**: Shows total record count, current page range, and page navigation buttons.
4. **Row Actions**: Contextual actions (edit, delete, view detail) placed in the last sticky column."#,
    },
    UiKnowledgeItem {
        id: "pattern-detail-inspector",
        title: "Split-View & Inspector Detail Pattern",
        category: "archetype",
        keywords: &["split", "inspector", "detail", "master-detail", "drawer", "sidebar"],
        summary: "Master-detail surface where selecting an entity in the primary list dynamically updates the contextual inspector.",
        architecture_guidance: r#"### Information Architecture
1. **Master Pane (Left / Primary)**: List or table displaying entity summaries. Selecting an item sets `state.selectedEntityId`.
2. **Detail Inspector (Right / Drawer)**: Detailed property inspector, activity logs, and edit fields for the selected entity.
3. **Empty Inspector State**: When `state.selectedEntityId` is null, render an inviting placeholder: 'Select an item to view details'."#,
    },
    UiKnowledgeItem {
        id: "interaction-visual-hierarchy",
        title: "Visual Hierarchy, Micro-interactions & Action Contracts",
        category: "interaction",
        keywords: &["hierarchy", "actions", "primary", "secondary", "destructive", "contrast", "tokens"],
        summary: "Designing prominent visual paths: one primary action per view, muted secondary actions, and protected destructive triggers.",
        architecture_guidance: r#"### Hierarchy & Interaction Standards
1. **One Primary Action**: Every section or surface should have at most ONE primary action (solid fill/high contrast).
2. **Secondary Actions**: Use outline or ghost styling for secondary actions (cancel, refresh, export).
3. **Destructive Actions**: Always use danger color tokens, place them away from primary buttons, and require two-step confirmation for unrecoverable deletes.
4. **Micro-Feedback**: Provide instant optimistic visual feedback when buttons are clicked (spinner or disabled state during in-flight action)."#,
    },
];

/// Search UI knowledge base by query keywords.
pub fn search_ui_knowledge(query: &str) -> Vec<&'static UiKnowledgeItem> {
    let q = query.to_lowercase();
    let query_terms: Vec<&str> = q.split_whitespace().collect();

    let mut matches = Vec::new();
    for item in UI_KNOWLEDGE_BASE {
        let mut score = 0;
        for term in &query_terms {
            if item.id.contains(term) || item.title.to_lowercase().contains(term) {
                score += 3;
            }
            if item.keywords.iter().any(|kw| kw.contains(term)) {
                score += 2;
            }
            if item.summary.to_lowercase().contains(term) {
                score += 1;
            }
        }
        if score > 0 {
            matches.push((score, item));
        }
    }

    matches.sort_by(|a, b| b.0.cmp(&a.0));
    matches.into_iter().map(|(_, item)| item).collect()
}

/// Retrieve a specific knowledge item by its stable ID.
pub fn get_ui_knowledge_by_id(id: &str) -> Option<&'static UiKnowledgeItem> {
    UI_KNOWLEDGE_BASE.iter().find(|item| item.id == id)
}

/// Format relevant UI knowledge as a concise markdown section for model prompt injection.
pub fn relevant_ui_knowledge_markdown(query: &str) -> String {
    let matches = search_ui_knowledge(query);
    if matches.is_empty() {
        return String::new();
    }

    let mut out = String::from("## Relevant Coreside UI Architecture Patterns\n\n");
    for item in matches.iter().take(2) {
        out.push_str(&format!("### {}\n", item.title));
        out.push_str(&format!("{}\n\n", item.summary));
        out.push_str(&format!("{}\n\n", item.architecture_guidance));
    }
    out
}

/// Compact index of available UI knowledge patterns for prompt injection.
/// Returns only titles and keywords so the model knows what exists,
/// without dumping full architecture guidance into every prompt.
/// Use the `GetUiKnowledge` tool for on-demand detailed retrieval.
pub fn ui_knowledge_catalog_markdown() -> String {
    let mut out = String::from(
        "## Coreside UI Knowledge (compact index)\n\n\
         Available patterns (use GetUiKnowledge tool for full guidance):\n",
    );
    for item in UI_KNOWLEDGE_BASE {
        out.push_str(&format!(
            "- **{}** `{}` — keywords: {}\n",
            item.title,
            item.id,
            item.keywords.join(", "),
        ));
    }
    out.push_str(
        "\nUse GetUiKnowledge(item_id) to retrieve full architecture guidance for a specific pattern.",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_ui_knowledge() {
        let results = search_ui_knowledge("biology research dashboard");
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "pattern-research-dashboard");

        let settings = search_ui_knowledge("preferences and toggles");
        assert!(!settings.is_empty());
        assert_eq!(settings[0].id, "pattern-settings-editor");

        let anti = search_ui_knowledge("card soup and dead buttons");
        assert!(!anti.is_empty());
        assert_eq!(anti[0].id, "ui-anti-patterns");
    }

    #[test]
    fn test_relevant_ui_knowledge_markdown() {
        let md = relevant_ui_knowledge_markdown("research papers");
        assert!(md.contains("Personal Research & Discovery Dashboard"));
        assert!(md.contains("Information Architecture"));
    }

    #[test]
    fn compact_index_is_shorter_than_full_catalog() {
        let compact = ui_knowledge_catalog_markdown();
        // Compact index should be under 2000 chars (vs >15000 for full dump)
        assert!(
            compact.len() < 2000,
            "compact index too large: {} chars",
            compact.len()
        );
        // But should still list all patterns
        assert!(compact.contains("pattern-research-dashboard"));
        assert!(compact.contains("pattern-settings-editor"));
        assert!(compact.contains("pattern-crud-tracker"));
        assert!(compact.contains("pattern-multi-step-workflow"));
        assert!(compact.contains("state-lifecycle-rules"));
        assert!(compact.contains("ui-anti-patterns"));
        assert!(compact.contains("responsive-surface-design"));
    }
}

import type { ToolSummary } from "@/types/tool";

type MentionMenuProps = {
  tools: ToolSummary[];
  query: string;
  activeIndex: number;
  onHover: (index: number) => void;
  onSelect: (tool: ToolSummary) => void;
  emptyLabel?: string;
};

export function MentionMenu({
  tools,
  query,
  activeIndex,
  onHover,
  onSelect,
  emptyLabel = "No tools match",
}: MentionMenuProps) {
  return (
    <div className="mention-menu" role="listbox" aria-label="Tool mentions">
      {tools.length === 0 ? (
        <p className="mention-menu-empty muted">
          {query ? emptyLabel : "No tools yet"}
        </p>
      ) : (
        <ul className="mention-menu-list">
          {tools.map((tool, index) => (
            <li key={tool.id}>
              <button
                type="button"
                role="option"
                aria-selected={index === activeIndex}
                className={`mention-menu-item${index === activeIndex ? " active" : ""}`}
                onMouseEnter={() => onHover(index)}
                onClick={() => onSelect(tool)}
              >
                <span className="mention-menu-icon" aria-hidden>
                  {tool.name.slice(0, 1).toUpperCase()}
                </span>
                <span className="mention-menu-copy">
                  <strong>{tool.name}</strong>
                  {tool.description ? (
                    <span className="muted">{tool.description}</span>
                  ) : null}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

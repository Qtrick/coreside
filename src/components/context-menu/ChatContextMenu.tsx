import type { Project } from "@/types/project";
import { ContextMenu, type ContextMenuItem } from "./ContextMenu";

type ChatContextMenuProps = {
  open: boolean;
  x: number;
  y: number;
  conversationId: string;
  conversationTitle: string;
  projectId?: string | null;
  projects: Project[];
  isCurrent: boolean;
  onClose: () => void;
  onOpen: () => void;
  onRename: () => void;
  onAddToProject: (projectId: string) => void;
  onMoveToProject: (projectId: string) => void;
  onRemoveFromProject: () => void;
  onOpenProject: () => void;
  onDuplicate: () => void;
  onExport: () => void;
  onDelete: () => void;
};

export function ChatContextMenu({
  open,
  x,
  y,
  conversationTitle,
  projectId,
  projects,
  isCurrent,
  onClose,
  onOpen,
  onRename,
  onAddToProject,
  onMoveToProject,
  onRemoveFromProject,
  onOpenProject,
  onDuplicate,
  onExport,
  onDelete,
}: ChatContextMenuProps) {
  const activeProjects = projects.filter((p) => !p.archived);
  const items: ContextMenuItem[] = [
    {
      id: "open",
      label: "Open",
      onSelect: onOpen,
      disabled: isCurrent,
    },
    { id: "rename", label: "Rename", onSelect: onRename, separatorBefore: true },
  ];

  if (projectId) {
    items.push({
      id: "open-project",
      label: "Open project",
      onSelect: onOpenProject,
    });
    items.push({
      id: "remove-project",
      label: "Remove from project",
      onSelect: onRemoveFromProject,
    });
    for (const project of activeProjects.filter((p) => p.id !== projectId)) {
      items.push({
        id: `move-${project.id}`,
        label: `Move to ${project.name}`,
        onSelect: () => onMoveToProject(project.id),
      });
    }
  } else if (activeProjects.length > 0) {
    for (const project of activeProjects) {
      items.push({
        id: `add-${project.id}`,
        label: `Add to ${project.name}`,
        onSelect: () => onAddToProject(project.id),
      });
    }
  }

  items.push(
    {
      id: "duplicate",
      label: "Duplicate",
      onSelect: onDuplicate,
      separatorBefore: true,
    },
    { id: "export", label: "Export", onSelect: onExport },
    {
      id: "delete",
      label: "Delete",
      onSelect: onDelete,
      destructive: true,
      separatorBefore: true,
    },
  );

  return (
    <ContextMenu
      open={open}
      x={x}
      y={y}
      items={items}
      onClose={onClose}
      ariaLabel={`Actions for ${conversationTitle}`}
    />
  );
}

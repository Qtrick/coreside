import type { Project } from "@/types/project";
import { ContextMenu, type ContextMenuItem } from "./ContextMenu";

type ProjectContextMenuProps = {
  open: boolean;
  x: number;
  y: number;
  project: Project;
  onClose: () => void;
  onOpen: () => void;
  onNewChat: () => void;
  onAddChats: () => void;
  onRename: () => void;
  onEditInstructions: () => void;
  onManageContext: () => void;
  onExport: () => void;
  onArchiveToggle: () => void;
  onDelete: () => void;
};

export function ProjectContextMenu({
  open,
  x,
  y,
  project,
  onClose,
  onOpen,
  onNewChat,
  onAddChats,
  onRename,
  onEditInstructions,
  onManageContext,
  onExport,
  onArchiveToggle,
  onDelete,
}: ProjectContextMenuProps) {
  const items: ContextMenuItem[] = [
    { id: "open", label: "Open", onSelect: onOpen },
    { id: "new-chat", label: "New chat", onSelect: onNewChat },
    { id: "add-chats", label: "Add existing chats", onSelect: onAddChats },
    {
      id: "rename",
      label: "Rename",
      onSelect: onRename,
      separatorBefore: true,
    },
    {
      id: "instructions",
      label: "Edit instructions",
      onSelect: onEditInstructions,
    },
    {
      id: "context",
      label: "Manage context",
      onSelect: onManageContext,
    },
    { id: "export", label: "Export", onSelect: onExport, separatorBefore: true },
    {
      id: "archive",
      label: project.archived ? "Restore" : "Archive",
      onSelect: onArchiveToggle,
    },
    {
      id: "delete",
      label: "Delete",
      onSelect: onDelete,
      destructive: true,
      separatorBefore: true,
    },
  ];

  return (
    <ContextMenu
      open={open}
      x={x}
      y={y}
      items={items}
      onClose={onClose}
      ariaLabel={`Actions for ${project.name}`}
    />
  );
}

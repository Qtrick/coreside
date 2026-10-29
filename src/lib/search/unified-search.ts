import type { Conversation } from "@/types/messages";
import type { Project } from "@/types/project";
import type { ToolSummary } from "@/types/tool";

export interface SearchResultItem {
  id: string;
  type: "chat" | "project" | "app";
  title: string;
  subtitle?: string | null;
  updatedAt?: string;
  projectId?: string | null;
}

export interface GroupedSearchResults {
  chats: SearchResultItem[];
  projects: SearchResultItem[];
  apps: SearchResultItem[];
  totalCount: number;
}

export function performUnifiedSearch(params: {
  query: string;
  conversations: Conversation[];
  projects: Project[];
  tools: ToolSummary[];
}): GroupedSearchResults {
  const query = params.query.trim().toLowerCase();
  if (!query) {
    return { chats: [], projects: [], apps: [], totalCount: 0 };
  }

  // Chats: match title
  const matchingChats: SearchResultItem[] = params.conversations
    .filter((c) => !c.archived && c.title.toLowerCase().includes(query))
    .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
    .slice(0, 10)
    .map((c) => ({
      id: c.id,
      type: "chat",
      title: c.title,
      updatedAt: c.updatedAt,
      projectId: c.projectId,
    }));

  // Projects: match name or description
  const matchingProjects: SearchResultItem[] = params.projects
    .filter(
      (p) =>
        p.name.toLowerCase().includes(query) ||
        (p.description && p.description.toLowerCase().includes(query)),
    )
    .slice(0, 5)
    .map((p) => ({
      id: p.id,
      type: "project",
      title: p.name,
      subtitle: p.description ?? undefined,
    }));

  // Apps: match name or description
  const matchingApps: SearchResultItem[] = params.tools
    .filter(
      (t) =>
        t.name.toLowerCase().includes(query) ||
        (t.description && t.description.toLowerCase().includes(query)),
    )
    .slice(0, 5)
    .map((t) => ({
      id: t.id,
      type: "app",
      title: t.name,
      subtitle: t.version != null ? `v${t.version}` : undefined,
    }));

  return {
    chats: matchingChats,
    projects: matchingProjects,
    apps: matchingApps,
    totalCount: matchingChats.length + matchingProjects.length + matchingApps.length,
  };
}


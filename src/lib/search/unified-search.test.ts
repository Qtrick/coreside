import { describe, expect, it } from "vitest";
import { performUnifiedSearch } from "./unified-search";
import type { Conversation } from "@/types/messages";
import type { Project } from "@/types/project";
import type { ToolSummary } from "@/types/tool";

describe("performUnifiedSearch", () => {
  const sampleConversations: Conversation[] = [
    {
      id: "conv-1",
      title: "React Architecture Plan",
      createdAt: "2026-10-20T10:00:00Z",
      updatedAt: "2026-10-28T14:00:00Z",
      projectId: "proj-1",
      pinned: false,
      archived: false,
    },
    {
      id: "conv-2",
      title: "Rust IPC Bridge Discussion",
      createdAt: "2026-10-21T10:00:00Z",
      updatedAt: "2026-10-28T15:00:00Z",
      projectId: null,
      pinned: false,
      archived: false,
    },
    {
      id: "conv-3",
      title: "Archived React Query Discussion",
      createdAt: "2026-10-15T10:00:00Z",
      updatedAt: "2026-10-28T16:00:00Z",
      projectId: null,
      pinned: false,
      archived: true,
    },
    {
      id: "conv-4",
      title: "C++ & Rust Performance Benchmarks v2.0",
      createdAt: "2026-10-22T10:00:00Z",
      updatedAt: "2026-10-28T12:00:00Z",
      projectId: "proj-2",
      pinned: false,
      archived: false,
    },
  ];

  const sampleProjects: Project[] = [
    {
      id: "proj-1",
      name: "React Workspace Redesign",
      description: "Modern split-pane workspace for web applications",
      archived: false,
      pinned: true,
      createdAt: "2026-10-01T00:00:00Z",
      updatedAt: "2026-10-28T10:00:00Z",
    },
    {
      id: "proj-2",
      name: "Desktop Engine",
      description: "Core Rust backend with high-performance IPC",
      archived: false,
      pinned: false,
      createdAt: "2026-10-05T00:00:00Z",
      updatedAt: "2026-10-27T10:00:00Z",
    },
    {
      id: "proj-3",
      name: "Alpha [WIP] (v2.0)",
      description: null,
      archived: false,
      pinned: false,
      createdAt: "2026-10-10T00:00:00Z",
      updatedAt: "2026-10-26T10:00:00Z",
    },
  ];

  const sampleTools: ToolSummary[] = [
    {
      id: "tool-1",
      name: "React Component Inspector",
      description: "Inspect React component trees and state",
      version: 2,
    },
    {
      id: "tool-2",
      name: "Memory Profiler",
      description: "Inspect heap memory usage and Rust allocations",
      version: 1,
    },
    {
      id: "tool-3",
      name: "Simple Note Tracker",
      description: undefined,
      version: null as unknown as undefined,
    },
  ];

  it("returns empty result structure for empty or whitespace-only queries", () => {
    expect(
      performUnifiedSearch({
        query: "",
        conversations: sampleConversations,
        projects: sampleProjects,
        tools: sampleTools,
      }),
    ).toEqual({
      chats: [],
      projects: [],
      apps: [],
      totalCount: 0,
    });

    expect(
      performUnifiedSearch({
        query: "   \t\n  ",
        conversations: sampleConversations,
        projects: sampleProjects,
        tools: sampleTools,
      }),
    ).toEqual({
      chats: [],
      projects: [],
      apps: [],
      totalCount: 0,
    });
  });

  it("handles case insensitivity across chats, projects, and apps", () => {
    const resultUpper = performUnifiedSearch({
      query: "REACT",
      conversations: sampleConversations,
      projects: sampleProjects,
      tools: sampleTools,
    });

    const resultLower = performUnifiedSearch({
      query: "react",
      conversations: sampleConversations,
      projects: sampleProjects,
      tools: sampleTools,
    });

    expect(resultUpper).toEqual(resultLower);
    expect(resultUpper.chats).toHaveLength(1);
    expect(resultUpper.chats[0].title).toBe("React Architecture Plan");
    expect(resultUpper.projects).toHaveLength(1);
    expect(resultUpper.projects[0].title).toBe("React Workspace Redesign");
    expect(resultUpper.apps).toHaveLength(1);
    expect(resultUpper.apps[0].title).toBe("React Component Inspector");
    expect(resultUpper.totalCount).toBe(3);
  });

  it("excludes archived chats from search results", () => {
    const result = performUnifiedSearch({
      query: "React",
      conversations: sampleConversations,
      projects: sampleProjects,
      tools: sampleTools,
    });

    // conv-3 is archived, so it should not appear
    expect(result.chats.some((c) => c.id === "conv-3")).toBe(false);
    expect(result.chats.some((c) => c.id === "conv-1")).toBe(true);
  });

  it("sorts matching chats by updatedAt descending", () => {
    const manyConversations: Conversation[] = [
      {
        id: "c-older",
        title: "Test discussion old",
        createdAt: "2026-10-01T00:00:00Z",
        updatedAt: "2026-10-10T12:00:00Z",
        archived: false,
        pinned: false,
      },
      {
        id: "c-newest",
        title: "Test discussion new",
        createdAt: "2026-10-01T00:00:00Z",
        updatedAt: "2026-10-28T12:00:00Z",
        archived: false,
        pinned: false,
      },
      {
        id: "c-middle",
        title: "Test discussion middle",
        createdAt: "2026-10-01T00:00:00Z",
        updatedAt: "2026-10-20T12:00:00Z",
        archived: false,
        pinned: false,
      },
    ];

    const result = performUnifiedSearch({
      query: "test",
      conversations: manyConversations,
      projects: [],
      tools: [],
    });

    expect(result.chats.map((c) => c.id)).toEqual(["c-newest", "c-middle", "c-older"]);
  });

  it("enforces slicing limits (chats max 10, projects max 5, apps max 5)", () => {
    const fifteenChats: Conversation[] = Array.from({ length: 15 }, (_, i) => ({
      id: `chat-${i}`,
      title: `General chat item ${i}`,
      createdAt: "2026-10-01T00:00:00Z",
      updatedAt: `2026-10-${String(i + 1).padStart(2, "0")}T00:00:00Z`,
      archived: false,
      pinned: false,
    }));

    const eightProjects: Project[] = Array.from({ length: 8 }, (_, i) => ({
      id: `proj-${i}`,
      name: `General project item ${i}`,
      description: `Project description ${i}`,
      archived: false,
      pinned: false,
      createdAt: "2026-10-01T00:00:00Z",
      updatedAt: "2026-10-01T00:00:00Z",
    }));

    const sevenTools: ToolSummary[] = Array.from({ length: 7 }, (_, i) => ({
      id: `tool-${i}`,
      name: `General app item ${i}`,
      description: `App description ${i}`,
      version: 1,
    }));

    const result = performUnifiedSearch({
      query: "General",
      conversations: fifteenChats,
      projects: eightProjects,
      tools: sevenTools,
    });

    expect(result.chats).toHaveLength(10);
    expect(result.projects).toHaveLength(5);
    expect(result.apps).toHaveLength(5);
    expect(result.totalCount).toBe(20);
  });

  it("matches projects and apps by description even when name does not match", () => {
    const result = performUnifiedSearch({
      query: "allocations",
      conversations: sampleConversations,
      projects: sampleProjects,
      tools: sampleTools,
    });

    expect(result.chats).toHaveLength(0);
    expect(result.projects).toHaveLength(0);
    expect(result.apps).toHaveLength(1);
    expect(result.apps[0].id).toBe("tool-2");
    expect(result.apps[0].title).toBe("Memory Profiler");
    expect(result.apps[0].subtitle).toBe("v1");
  });

  it("handles special characters, punctuation, and version strings safely", () => {
    const result = performUnifiedSearch({
      query: "v2.0",
      conversations: sampleConversations,
      projects: sampleProjects,
      tools: sampleTools,
    });

    expect(result.chats.some((c) => c.title.includes("v2.0"))).toBe(true);
    expect(result.projects.some((p) => p.title.includes("v2.0"))).toBe(true);
  });

  it("handles null or missing descriptions cleanly without throwing", () => {
    const result = performUnifiedSearch({
      query: "alpha",
      conversations: sampleConversations,
      projects: sampleProjects,
      tools: sampleTools,
    });

    expect(result.projects).toHaveLength(1);
    expect(result.projects[0].title).toBe("Alpha [WIP] (v2.0)");
    expect(result.projects[0].subtitle).toBeUndefined();
  });

  it("returns totalCount as 0 when no items match the query", () => {
    const result = performUnifiedSearch({
      query: "nonexistent-query-term-xyz",
      conversations: sampleConversations,
      projects: sampleProjects,
      tools: sampleTools,
    });

    expect(result.chats).toHaveLength(0);
    expect(result.projects).toHaveLength(0);
    expect(result.apps).toHaveLength(0);
    expect(result.totalCount).toBe(0);
  });
});

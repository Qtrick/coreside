import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { ProjectContextDialog } from "./ProjectContextDialog";
import { api } from "@/lib/tauri/api";
import type { Project, ProjectContextHit } from "@/types/project";

vi.mock("@/lib/tauri/api", () => ({
  api: {
    searchProjectContext: vi.fn(),
    rebuildProjectIndex: vi.fn(),
  },
}));

const mockProject: Project = {
  id: "proj-1",
  name: "Machine Learning Notes",
  description: "Research notes and implementations",
  instructions: "Focus on PyTorch and Rust",
  pinned: false,
  archived: false,
  createdAt: "2026-09-01T00:00:00Z",
  updatedAt: "2026-09-20T00:00:00Z",
};

const mockHits: ProjectContextHit[] = [
  {
    messageId: "msg-101",
    conversationId: "conv-1",
    conversationTitle: "Attention Mechanisms",
    role: "assistant",
    snippet: "Multi-head attention allows the model to jointly attend to information...",
    rank: 1.5,
  },
  {
    messageId: "msg-102",
    conversationId: "conv-2",
    conversationTitle: "Optimizer Tuning",
    role: "user",
    snippet: "How do we adjust AdamW weight decay?",
    rank: 0.8,
  },
];

describe("ProjectContextDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("does not render when open is false", () => {
    const { container } = render(
      <ProjectContextDialog
        open={false}
        project={mockProject}
        onClose={vi.fn()}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("renders project name and search controls when open", () => {
    render(
      <ProjectContextDialog
        open={true}
        project={mockProject}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getByText("Manage Project Context")).toBeInTheDocument();
    expect(screen.getByText(/Machine Learning Notes/)).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/Search project messages/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Search" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Rebuild Index/i })).toBeInTheDocument();
  });

  it("searches context and displays hits", async () => {
    vi.mocked(api.searchProjectContext).mockResolvedValueOnce(mockHits);

    render(
      <ProjectContextDialog
        open={true}
        project={mockProject}
        onClose={vi.fn()}
      />,
    );

    const input = screen.getByPlaceholderText(/Search project messages/);
    fireEvent.change(input, { target: { value: "attention" } });

    const searchBtn = screen.getByRole("button", { name: "Search" });
    fireEvent.click(searchBtn);

    expect(api.searchProjectContext).toHaveBeenCalledWith("proj-1", "attention", 15);

    await waitFor(() => {
      expect(screen.getByText("Attention Mechanisms")).toBeInTheDocument();
      expect(screen.getByText(/Multi-head attention allows/)).toBeInTheDocument();
      expect(screen.getByText("Optimizer Tuning")).toBeInTheDocument();
    });
  });

  it("navigates to chat when a search hit is clicked", async () => {
    vi.mocked(api.searchProjectContext).mockResolvedValueOnce(mockHits);
    const onOpenChat = vi.fn();
    const onClose = vi.fn();

    render(
      <ProjectContextDialog
        open={true}
        project={mockProject}
        onClose={onClose}
        onOpenChat={onOpenChat}
      />,
    );

    fireEvent.change(screen.getByPlaceholderText(/Search project messages/), {
      target: { value: "attention" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Search" }));

    await waitFor(() => {
      expect(screen.getByText("Attention Mechanisms")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("Attention Mechanisms"));

    expect(onClose).toHaveBeenCalled();
    expect(onOpenChat).toHaveBeenCalledWith("conv-1");
  });

  it("rebuilds index and displays status message", async () => {
    vi.mocked(api.rebuildProjectIndex).mockResolvedValueOnce(42);

    render(
      <ProjectContextDialog
        open={true}
        project={mockProject}
        onClose={vi.fn()}
      />,
    );

    const rebuildBtn = screen.getByRole("button", { name: /Rebuild Index/i });
    fireEvent.click(rebuildBtn);

    expect(api.rebuildProjectIndex).toHaveBeenCalledWith("proj-1");

    await waitFor(() => {
      expect(screen.getByText(/Successfully re-indexed 42 messages in project/)).toBeInTheDocument();
    });
  });

  it("displays error message on search failure", async () => {
    vi.mocked(api.searchProjectContext).mockRejectedValueOnce(new Error("Index database locked"));

    render(
      <ProjectContextDialog
        open={true}
        project={mockProject}
        onClose={vi.fn()}
      />,
    );

    fireEvent.change(screen.getByPlaceholderText(/Search project messages/), {
      target: { value: "test" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Search" }));

    await waitFor(() => {
      expect(screen.getByText("Index database locked")).toBeInTheDocument();
    });
  });
});

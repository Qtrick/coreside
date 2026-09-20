import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AppRouteShell } from "./AppRouteShell";
import type { ApplicationManifest } from "@/types/application-kernel";

const mockGetSurfaceStateWithRevision = vi.fn().mockResolvedValue({
  state: { count: 1 },
  stateRevision: 3,
});
const mockSaveSurfaceState = vi.fn().mockResolvedValue(4);
const mockGetRouteState = vi.fn().mockResolvedValue({
  id: "rs-1",
  applicationId: "app-test",
  windowId: "win-1",
  currentRouteId: "route-main",
  routeParams: {},
  history: [{ routeId: "route-main", params: {} }],
  historyIndex: 0,
  updatedAt: new Date().toISOString(),
});

vi.mock("@/lib/tauri", () => ({
  api: {
    getSurfaceStateWithRevision: (...args: unknown[]) => mockGetSurfaceStateWithRevision(...args),
    getSurfaceState: vi.fn().mockResolvedValue({}),
    saveSurfaceState: (...args: unknown[]) => mockSaveSurfaceState(...args),
    getRouteState: (...args: unknown[]) => mockGetRouteState(...args),
    setRouteState: vi.fn(),
  },
}));

const baseManifest: ApplicationManifest = {
  schemaVersion: "1",
  applicationId: "app-test",
  instanceId: "inst-1",
  name: "Test App",
  description: "Test App Description",
  version: 1,
  surfaces: [],
  routes: [
    {
      routeId: "route-main",
      title: "Main Screen",
      surfaceId: "surface-main",
    },
  ],
  dataModels: [],
  settings: [],
  capabilities: [],
  permissions: [],
  events: [],
  tests: [],
  searchKeywords: [],
  tags: [],
  agentDescription: "",
  applicationActionAccess: [],
  surfaceActionAccess: {},
  componentActionAccess: {},
  actionDescriptorHashes: {},
};

describe("AppRouteShell", () => {
  it("renders null when manifest has no routes without violating React hook rules (P0-19)", () => {
    const emptyManifest: ApplicationManifest = {
      ...baseManifest,
      routes: [],
    };

    const { container, rerender } = render(
      <AppRouteShell
        applicationId="app-test"
        manifest={emptyManifest}
        state={{}}
        onStateChange={vi.fn()}
      />,
    );

    expect(container.firstChild).toBeNull();

    // Re-rendering with routes must not trigger React error #300 / hooks count mismatch
    rerender(
      <AppRouteShell
        applicationId="app-test"
        manifest={baseManifest}
        state={{}}
        onStateChange={vi.fn()}
      />,
    );

    expect(container.querySelector(".app-route-shell")).toBeInTheDocument();
  });

  it("renders navigation header and loads route state", async () => {
    render(
      <AppRouteShell
        applicationId="app-test"
        manifest={baseManifest}
        state={{}}
        onStateChange={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Main Screen")).toBeInTheDocument();
    });
  });

  it("fetches state and revision atomically for active surface", async () => {
    render(
      <AppRouteShell
        applicationId="app-test"
        manifest={baseManifest}
        state={{}}
        onStateChange={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(mockGetSurfaceStateWithRevision).toHaveBeenCalledWith("surface-main");
    });
  });
});

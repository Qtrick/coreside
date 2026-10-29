import { useRef, useState, useEffect, type CSSProperties } from "react";
import { useWorkspaceStore } from "@/stores/workspace-store";
import { WorkspacePane } from "./WorkspacePane";
import { useShallow } from "zustand/react/shallow";

export function WorkspaceGrid() {
  const {
    layoutKind,
    panes,
    focusedPaneId,
    setFocusedPane,
    splitRatios,
    setSplitRatios,
  } = useWorkspaceStore(
    useShallow((s) => ({
      layoutKind: s.layoutKind,
      panes: s.panes,
      focusedPaneId: s.focusedPaneId,
      setFocusedPane: s.setFocusedPane,
      splitRatios: s.splitRatios,
      setSplitRatios: s.setSplitRatios,
    })),
  );

  const containerRef = useRef<HTMLDivElement>(null);
  const [containerWidth, setContainerWidth] = useState(1200);
  const [liveRatio, setLiveRatio] = useState<number | null>(null);
  const draggingRef = useRef(false);

  useEffect(() => {
    const el = containerRef.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (entry) {
        setContainerWidth(entry.contentRect.width);
      }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const ratio = liveRatio ?? (splitRatios[0] ?? 0.5);

  const handlePointerDown = (e: React.PointerEvent) => {
    e.preventDefault();
    draggingRef.current = true;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);

    const onPointerMove = (moveEvent: PointerEvent) => {
      if (!draggingRef.current || !containerRef.current) return;
      const rect = containerRef.current.getBoundingClientRect();
      const currentX = moveEvent.clientX - rect.left;
      const newRatio = Math.max(0.2, Math.min(0.8, currentX / rect.width));
      setLiveRatio(newRatio);
    };

    const onPointerUp = (upEvent: PointerEvent) => {
      draggingRef.current = false;
      try {
        (e.target as HTMLElement).releasePointerCapture(upEvent.pointerId);
      } catch {
        // Safe pointer capture release
      }
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
      if (liveRatio !== null) {
        setSplitRatios([liveRatio, 1 - liveRatio]);
        setLiveRatio(null);
      }
    };

    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
  };

  const handleHorizontalPointerDown = (e: React.PointerEvent) => {
    e.preventDefault();
    draggingRef.current = true;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);

    const onPointerMove = (moveEvent: PointerEvent) => {
      if (!draggingRef.current || !containerRef.current) return;
      const rect = containerRef.current.getBoundingClientRect();
      const currentY = moveEvent.clientY - rect.top;
      const newRatio = Math.max(0.2, Math.min(0.8, currentY / rect.height));
      setLiveRatio(newRatio);
    };

    const onPointerUp = (upEvent: PointerEvent) => {
      draggingRef.current = false;
      try {
        (e.target as HTMLElement).releasePointerCapture(upEvent.pointerId);
      } catch {
        // Safe pointer capture release
      }
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
      if (liveRatio !== null) {
        setSplitRatios([liveRatio, 1 - liveRatio]);
        setLiveRatio(null);
      }
    };

    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
  };

  const handleSplitterKeyDown = (e: React.KeyboardEvent) => {
    const step = 0.05;
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      const nextRatio = Math.max(0.2, ratio - step);
      setSplitRatios([nextRatio, 1 - nextRatio]);
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      const nextRatio = Math.min(0.8, ratio + step);
      setSplitRatios([nextRatio, 1 - nextRatio]);
    }
  };

  const handleHorizontalSplitterKeyDown = (e: React.KeyboardEvent) => {
    const step = 0.05;
    if (e.key === "ArrowUp") {
      e.preventDefault();
      const nextRatio = Math.max(0.2, ratio - step);
      setSplitRatios([nextRatio, 1 - nextRatio]);
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      const nextRatio = Math.min(0.8, ratio + step);
      setSplitRatios([nextRatio, 1 - nextRatio]);
    }
  };

  const isCompact = containerWidth < 700;

  // Single Pane Layout or Compact Fallback
  if (layoutKind === "single" || isCompact) {
    const activePane =
      panes.find((p) => p.id === focusedPaneId) || panes[0] || { id: "pane-1" };

    return (
      <div
        ref={containerRef}
        className="workspace-grid layout-single"
        data-testid="workspace-grid-single"
      >
        {isCompact && layoutKind !== "single" ? (
          <div className="compact-pane-selector" role="tablist" aria-label="Panes">
            {panes.slice(0, layoutKind === "split-grid" ? 4 : layoutKind === "split-three" ? 3 : 2).map((p, idx) => (
              <button
                key={p.id}
                type="button"
                className={`compact-pane-pill${p.id === activePane.id ? " active" : ""}`}
                onClick={() => setFocusedPane(p.id)}
              >
                Pane {idx + 1}
              </button>
            ))}
          </div>
        ) : null}
        <WorkspacePane
          paneId={activePane.id}
          isFocused={true}
          onFocus={() => setFocusedPane(activePane.id)}
        />
      </div>
    );
  }

  // 2 Panes: Vertical Split
  if (layoutKind === "split-vertical") {
    const pane1 = panes[0]?.id || "pane-1";
    const pane2 = panes[1]?.id || "pane-2";

    return (
      <div
        ref={containerRef}
        className="workspace-grid layout-split-vertical"
        style={
          {
            "--split-ratio-1": `${ratio}fr`,
            "--split-ratio-2": `${1 - ratio}fr`,
          } as CSSProperties
        }
        data-testid="workspace-grid-split-vertical"
      >
        <WorkspacePane
          paneId={pane1}
          isFocused={focusedPaneId === pane1}
          onFocus={() => setFocusedPane(pane1)}
        />
        <div
          className="workspace-pane-splitter vertical"
          role="separator"
          tabIndex={0}
          aria-valuenow={Math.round(ratio * 100)}
          aria-valuemin={20}
          aria-valuemax={80}
          aria-label="Pane divider"
          onPointerDown={handlePointerDown}
          onKeyDown={handleSplitterKeyDown}
        />
        <WorkspacePane
          paneId={pane2}
          isFocused={focusedPaneId === pane2}
          onFocus={() => setFocusedPane(pane2)}
        />
      </div>
    );
  }

  // 2 Panes: Horizontal Split
  if (layoutKind === "split-horizontal") {
    const pane1 = panes[0]?.id || "pane-1";
    const pane2 = panes[1]?.id || "pane-2";

    return (
      <div
        ref={containerRef}
        className="workspace-grid layout-split-horizontal"
        style={
          {
            "--split-ratio-1": `${ratio}fr`,
            "--split-ratio-2": `${1 - ratio}fr`,
          } as CSSProperties
        }
        data-testid="workspace-grid-split-horizontal"
      >
        <WorkspacePane
          paneId={pane1}
          isFocused={focusedPaneId === pane1}
          onFocus={() => setFocusedPane(pane1)}
        />
        <div
          className="workspace-pane-splitter horizontal"
          role="separator"
          tabIndex={0}
          aria-valuenow={Math.round(ratio * 100)}
          aria-valuemin={20}
          aria-valuemax={80}
          aria-label="Horizontal pane divider"
          onPointerDown={handleHorizontalPointerDown}
          onKeyDown={handleHorizontalSplitterKeyDown}
        />
        <WorkspacePane
          paneId={pane2}
          isFocused={focusedPaneId === pane2}
          onFocus={() => setFocusedPane(pane2)}
        />
      </div>
    );
  }

  // 3 Panes: Left Full Height, Right 2 Stacked Panes
  if (layoutKind === "split-three") {
    const pane1 = panes[0]?.id || "pane-1";
    const pane2 = panes[1]?.id || "pane-2";
    const pane3 = panes[2]?.id || "pane-3";

    return (
      <div
        ref={containerRef}
        className="workspace-grid layout-split-three"
        style={
          {
            "--split-ratio-1": `${ratio}fr`,
            "--split-ratio-2": `${1 - ratio}fr`,
          } as CSSProperties
        }
        data-testid="workspace-grid-split-three"
      >
        <WorkspacePane
          paneId={pane1}
          isFocused={focusedPaneId === pane1}
          onFocus={() => setFocusedPane(pane1)}
        />
        <div
          className="workspace-pane-splitter vertical"
          role="separator"
          tabIndex={0}
          aria-label="Vertical pane divider"
          onPointerDown={handlePointerDown}
          onKeyDown={handleSplitterKeyDown}
        />
        <div className="three-pane-right-stack">
          <WorkspacePane
            paneId={pane2}
            isFocused={focusedPaneId === pane2}
            onFocus={() => setFocusedPane(pane2)}
          />
          <div
            className="workspace-pane-splitter horizontal"
            role="separator"
            tabIndex={0}
            aria-label="Horizontal pane divider"
            onPointerDown={handleHorizontalPointerDown}
            onKeyDown={handleHorizontalSplitterKeyDown}
          />
          <WorkspacePane
            paneId={pane3}
            isFocused={focusedPaneId === pane3}
            onFocus={() => setFocusedPane(pane3)}
          />
        </div>
      </div>
    );
  }

  // 4 Panes: 2x2 Grid
  if (layoutKind === "split-grid") {
    const pane1 = panes[0]?.id || "pane-1";
    const pane2 = panes[1]?.id || "pane-2";
    const pane3 = panes[2]?.id || "pane-3";
    const pane4 = panes[3]?.id || "pane-4";

    return (
      <div
        ref={containerRef}
        className="workspace-grid layout-split-grid"
        data-testid="workspace-grid-split-grid"
      >
        <div className="grid-row top-row">
          <WorkspacePane
            paneId={pane1}
            isFocused={focusedPaneId === pane1}
            onFocus={() => setFocusedPane(pane1)}
          />
          <div
            className="workspace-pane-splitter vertical"
            role="separator"
            tabIndex={0}
            aria-label="Top vertical divider"
            onPointerDown={handlePointerDown}
            onKeyDown={handleSplitterKeyDown}
          />
          <WorkspacePane
            paneId={pane2}
            isFocused={focusedPaneId === pane2}
            onFocus={() => setFocusedPane(pane2)}
          />
        </div>
        <div
          className="workspace-pane-splitter horizontal"
          role="separator"
          tabIndex={0}
          aria-label="Grid horizontal divider"
          onPointerDown={handleHorizontalPointerDown}
          onKeyDown={handleHorizontalSplitterKeyDown}
        />
        <div className="grid-row bottom-row">
          <WorkspacePane
            paneId={pane3}
            isFocused={focusedPaneId === pane3}
            onFocus={() => setFocusedPane(pane3)}
          />
          <div
            className="workspace-pane-splitter vertical"
            role="separator"
            tabIndex={0}
            aria-label="Bottom vertical divider"
            onPointerDown={handlePointerDown}
            onKeyDown={handleSplitterKeyDown}
          />
          <WorkspacePane
            paneId={pane4}
            isFocused={focusedPaneId === pane4}
            onFocus={() => setFocusedPane(pane4)}
          />
        </div>
      </div>
    );
  }

  // Fallback
  return (
    <div ref={containerRef} className="workspace-grid layout-single">
      <WorkspacePane
        paneId={panes[0]?.id || "pane-1"}
        isFocused={true}
        onFocus={() => setFocusedPane(panes[0]?.id || "pane-1")}
      />
    </div>
  );
}


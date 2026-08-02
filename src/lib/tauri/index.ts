export type { AgentTurnEvent } from "./events";
export { listenAgentTurn, listenApprovalsChanged } from "./events";
export { TauriCommandError } from "./errors";
export { isTauriRuntime, isWebPreview } from "./runtime";
export { api } from "./api";

/** Test helper — dynamic import keeps mocks out of the production main chunk. */
export async function __setMockAiConfigured(configured: boolean): Promise<void> {
  const { __setMockAiConfigured: set } = await import("./mocks");
  set(configured);
}

/** Test helper — dynamic import keeps mocks out of the production main chunk. */
export async function __resetMockDb(): Promise<void> {
  const { __resetMockDb: reset } = await import("./mocks");
  reset();
}

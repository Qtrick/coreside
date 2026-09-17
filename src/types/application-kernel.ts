/** Application Kernel / Manifest types for Coreside. */

export interface ApplicationManifest {
  schemaVersion: string;
  applicationId: string;
  instanceId: string;
  name: string;
  description?: string;
  version?: number;
  surfaces?: Array<{
    surfaceId: string;
    placement?: string;
    definitionRef?: string;
  }>;
  routes?: Array<{ routeId: string; title: string; surfaceId?: string }>;
  dataModels?: Array<{ modelId: string; schemaRef?: string }>;
  settings?: string[];
  capabilities?: string[];
  permissions?: string[];
  events?: string[];
  tests?: string[];
  searchKeywords?: string[];
  tags?: string[];
  agentDescription?: string;
  projectId?: string | null;
  conversationId?: string | null;
  applicationActionAccess?: string[];
  surfaceActionAccess?: Record<string, string[]>;
  componentActionAccess?: Record<string, string[]>;
}

export type ActionRisk = "read" | "write" | "destructive";

export interface ActionDescriptor {
  name: string;
  title: string;
  description: string;
  inputSchema: Record<string, unknown>;
  risk: ActionRisk;
  critical: boolean;
  permissionCategory: string;
  stateBindable?: boolean;
  descriptorHash?: string;
}

export type ActionOutcome =
  | { status: "ok"; data: unknown; stateBindable: boolean }
  | { status: "error"; code: string; message: string }
  | {
      status: "pendingApproval";
      approvalId: string;
      actionName: string;
      title: string;
      risk: string;
      inputPreview: string;
      reason: string;
    }
  | { status: "blocked"; code: string; reason: string };

export interface ApprovalRequest {
  id: string;
  applicationId: string | null;
  actionName: string;
  actionTitle: string;
  risk: string;
  critical: boolean;
  inputPreview: string;
  callHash: string;
  descriptorHash: string;
  venue: string;
  presence: string;
  sessionId: string | null;
  runId: string | null;
  status: string;
  createdAt: string;
  expiresAt: string;
  decidedAt: string | null;
  consumedAt: string | null;
  explanation: string | null;
  surfaceId: string | null;
  componentId: string | null;
}

export interface RuntimeGrant {
  id: string;
  subject: string;
  applicationId: string | null;
  actionName: string;
  descriptorHash: string;
  scopeKind: string;
  scope: Record<string, unknown>;
  duration: string;
  sessionId: string | null;
  source: string;
  status: string;
  grantedAt: string;
  revokedAt: string | null;
  expiresAt: string | null;
}

export interface AuditEvent {
  id: string;
  kind: string;
  actor: string;
  venue: string;
  presence: string;
  applicationId: string | null;
  projectId: string | null;
  conversationId: string | null;
  runId: string | null;
  actionName: string | null;
  inputPreview: string | null;
  outcome: string;
  risk: string | null;
  decisionSource: string | null;
  approvalId: string | null;
  grantId: string | null;
  detail: string | null;
  durationMs: number | null;
  createdAt: string;
}

export interface BuildFailure {
  id: string;
  applicationId: string;
  safeMessage: string;
  retryable: boolean;
  requestRef: string | null;
  createdAt: string;
}

export interface ApplicationVersion {
  version: number;
  validationStatus: string;
  testStatus: string;
  isKnownGood: boolean;
  createdAt: string;
}

export interface ClientActionRequest {
  actionName: string;
  input?: Record<string, unknown>;
  applicationId?: string | null;
  surfaceId?: string | null;
  componentId?: string | null;
  conversationId?: string | null;
  projectId?: string | null;
  approvalId?: string | null;
}

export type RememberScope = "exact" | "action" | "application_action";
export type RememberDuration = "once" | "session" | "standing";

export interface ApprovalDecisionResult {
  approval: ApprovalRequest;
  grant: RuntimeGrant | null;
  outcome?: ActionOutcome | null;
}

export interface ManifestRecord {
  id: string;
  applicationId: string;
  instanceId: string;
  schemaVersion: string;
  currentVersion: number;
  lastKnownGoodVersion: number | null;
  manifest: ApplicationManifest;
  healthState: string;
  lifecycleState: string;
  disabled: boolean;
  crashCount: number;
  projectId: string | null;
  conversationId: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface RecoveryState {
  recoveryMode: boolean;
  disableUserSurfaces: boolean;
  disableCustomLayouts: boolean;
  disableCapabilityPacks: boolean;
  uncleanShutdown: boolean;
  lastFailure: unknown | null;
  updatedAt: string;
}

export interface UnifiedSearchHit {
  resourceType: string;
  id: string;
  title: string;
  projectId: string | null;
  updatedAt: string | null;
}

export interface PackagePreview {
  name: string;
  applicationId: string;
  permissions: string[];
  capabilities: string[];
  recordCount: number;
  trustState: string;
  warnings: string[];
}

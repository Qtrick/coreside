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

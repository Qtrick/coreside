export type AddedSetting = {
  id: string;
  ownerToolId?: string | null;
  label: string;
  description: string;
  settingType: string;
  defaultValue: unknown;
  currentValue: unknown;
  constraints: unknown;
  version: number;
  createdAt: string;
  updatedAt: string;
};

export type UpsertAddedSettingInput = {
  id: string;
  ownerToolId?: string | null;
  label: string;
  description?: string;
  settingType: string;
  defaultValue?: unknown;
  currentValue?: unknown;
  constraints?: unknown;
};

export type ValidateChangeTargetsResult = {
  ok: boolean;
  rejected: string[];
};

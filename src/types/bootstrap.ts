export type BootstrapStatus =
  | { status: "ready" }
  | {
      status: "recoveryRequired";
      reasonCode: string;
      message: string;
      databasePath?: string | null;
      usingShellDatabase: boolean;
    };

import type {
  AiAccountSnapshot,
  AiLoginChallenge,
  AiRuntimeStatus,
  AppSettings,
  BootstrapStatus,
  ToolManifest,
  UpdateCheckResult,
} from "@openconkit/contracts";

export const bootstrap: BootstrapStatus = {
  home_path: "C:\\Users\\e2e\\.openconkit",
  created_fresh: false,
  structure_validated: true,
  recovered_from_interrupt: false,
  config_warnings: [],
  database_migrations: [],
  backups_created: [],
};

export const manifest: ToolManifest = {
  id: "boq-inspector",
  contract_version: 2,
  tool_version: "0.0.1",
  name_key: "tools.boqInspector.name",
  description_key: "tools.boqInspector.description",
  icon: "tools/boq-inspector.svg",
  route: "/tools/boq-inspector",
};

export const defaultSettings: AppSettings = {
  schema_version: 2,
  onboarding_completed: true,
  language: "en",
  theme: "light",
  update_channel: "stable",
  tolerances: {
    absolute_tolerance: "0.01",
    relative_tolerance: "0.001",
    decimal_precision: 2,
  },
  privacy: {
    ai_features_enabled: false,
    diagnostic_logging_enabled: false,
  },
  advanced: {
    use_system_codex: false,
    system_codex_binary: null,
  },
  last_successful_update_check: null,
};

export const aiRuntime: AiRuntimeStatus = {
  enabled: true,
  bundled_runtime_available: true,
  selected_runtime_available: true,
  using_system_runtime: false,
  codex_version: "0.145.0",
};

export const signedOutAccount: AiAccountSnapshot = {
  signed_in: false,
  masked_email: null,
  plan_type: null,
  requires_openai_auth: true,
  codex_version: "0.145.0",
};

export const browserLoginChallenge: AiLoginChallenge = {
  login_id: "e2e-login",
  mode: "browser",
  user_code: null,
};

export const betaUpdate: UpdateCheckResult = {
  checked_at: "2026-07-24T10:00:00Z",
  channel: "beta",
  current_version: "0.0.1",
  portable: false,
  update: {
    version: "0.1.0-beta.1",
    notes: "E2E release notes",
    published_at: "2026-07-24T09:00:00Z",
    size_bytes: 41_943_040,
    can_install: true,
    manual_download_url: "https://github.com/kareem-sf/OpenConKit/releases/tag/v0.1.0-beta.1",
  },
};

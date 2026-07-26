import { useCallback, useEffect, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";

import type {
  AdvancedSettings,
  AiAccountSnapshot,
  AiLoginChallenge,
  AiRateLimitSnapshot,
  AiRuntimeStatus,
  AnalysisTolerances,
  AppSettings,
  Language,
  PrivacySettings,
  SettingsPatch,
  Theme,
  UpdateChannel,
  UpdateCheckResult,
  UpdateProgressEvent,
} from "@openconkit/contracts";
import { Button } from "@openconkit/ui";

import { Icon } from "../components/Icon";
import { desktopApi, desktopRuntimeAvailable, errorCodeOf } from "../lib/ipc";
import { useWorkspaceStore } from "../state/workspace";
import { useThemeStore } from "../theme";

function ResetOpenConKitDialog({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const resetApplication = useWorkspaceStore((state) => state.resetApplication);
  const busy = useWorkspaceStore((state) => state.busyAction === "reset");
  const [confirmation, setConfirmation] = useState("");
  const confirmed = confirmation === "RESET";

  const reset = async () => {
    if (await resetApplication()) {
      onClose();
    }
  };

  return (
    <div className="modal-backdrop" role="presentation">
      <section
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="reset-openconkit-title"
        aria-describedby="reset-openconkit-description"
        className="modal-panel modal-panel-small"
      >
        <header className="modal-header">
          <div>
            <h2 id="reset-openconkit-title">{t("settings.resetTitle")}</h2>
            <p id="reset-openconkit-description">{t("settings.resetWarning")}</p>
          </div>
        </header>
        <div className="reset-confirmation">
          <p>{t("settings.resetKeepsOriginals")}</p>
          <label className="field-control">
            <span>{t("settings.resetConfirmation")}</span>
            <input
              dir="ltr"
              autoComplete="off"
              spellCheck={false}
              value={confirmation}
              onChange={(event) => setConfirmation(event.target.value)}
              placeholder={t("settings.resetConfirmationPhrase")}
            />
          </label>
        </div>
        <footer className="modal-actions">
          <Button variant="secondary" disabled={busy} onClick={onClose}>
            {t("actions.cancel")}
          </Button>
          <Button
            variant="danger"
            data-testid="reset-openconkit-confirm"
            disabled={busy || !confirmed}
            onClick={() => void reset()}
          >
            {busy ? t("settings.resetting") : t("settings.resetAction")}
          </Button>
        </footer>
      </section>
    </div>
  );
}

/** Canonical app-home settings editor. */
export function SettingsPage() {
  const { t } = useTranslation();
  const settings = useWorkspaceStore((state) => state.settings);
  const initialize = useWorkspaceStore((state) => state.initialize);
  const loading = useWorkspaceStore((state) => state.loading);

  if (!settings) {
    return (
      <main className="page-shell settings-page">
        <header className="page-header">
          <div>
            <h1>{t("settings.title")}</h1>
            <p>{t("settings.subtitle")}</p>
          </div>
        </header>
        <section className="settings-section" aria-labelledby="settings-unavailable-title">
          <div>
            <h2 id="settings-unavailable-title">{t("settings.unavailableTitle")}</h2>
            <p role="alert">{t("settings.unavailableHelp")}</p>
          </div>
          <div>
            <Button type="button" disabled={loading} onClick={() => void initialize()}>
              {loading ? t("status.loading") : t("settings.retryLoading")}
            </Button>
          </div>
        </section>
      </main>
    );
  }

  return <SettingsEditor settings={settings} />;
}

function SettingsEditor({ settings }: { settings: AppSettings }) {
  const { t, i18n } = useTranslation();
  const saveSettings = useWorkspaceStore((state) => state.saveSettings);
  const busy = useWorkspaceStore((state) => state.busyAction === "settings");
  const setThemePreference = useThemeStore((state) => state.setPreference);
  const [language, setLanguage] = useState<Language>(settings.language);
  const [theme, setTheme] = useState<Theme>(settings.theme);
  const [updateChannel, setUpdateChannel] = useState<UpdateChannel>(settings.update_channel);
  const [tolerances, setTolerances] = useState<AnalysisTolerances>(settings.tolerances);
  const [privacy, setPrivacy] = useState<PrivacySettings>(settings.privacy);
  const [advanced, setAdvanced] = useState<AdvancedSettings>(settings.advanced);
  const [saved, setSaved] = useState(false);
  const [showReset, setShowReset] = useState(false);

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setSaved(false);
    const patch: SettingsPatch = {
      onboarding_completed: null,
      language,
      theme,
      update_channel: updateChannel,
      tolerances,
      privacy,
      advanced,
      last_successful_update_check: null,
    };
    const updated = await saveSettings(patch);
    if (!updated) {
      return;
    }
    setThemePreference(updated.theme);
    const locale = updated.language === "system" ? navigator.language : updated.language;
    await i18n.changeLanguage(locale);
    setSaved(true);
  };

  return (
    <main className="page-shell settings-page">
      <header className="page-header">
        <div>
          <h1>{t("settings.title")}</h1>
          <p>{t("settings.subtitle")}</p>
        </div>
      </header>

      <form onSubmit={(event) => void submit(event)} className="settings-form">
        <section className="settings-section" aria-labelledby="appearance-title">
          <div>
            <h2 id="appearance-title">{t("settings.appearance")}</h2>
            <p>{t("settings.appearanceHelp")}</p>
          </div>
          <div className="settings-fields">
            <label className="field-control">
              <span>{t("settings.language")}</span>
              <select
                id="settings-language"
                value={language}
                onChange={(event) => setLanguage(event.target.value as Language)}
              >
                <option value="system">{t("language.system")}</option>
                <option value="en">{t("language.en")}</option>
                <option value="ar">{t("language.ar")}</option>
              </select>
            </label>
            <label className="field-control">
              <span>{t("settings.theme")}</span>
              <select
                id="settings-theme"
                value={theme}
                onChange={(event) => setTheme(event.target.value as Theme)}
              >
                <option value="system">{t("theme.system")}</option>
                <option value="light">{t("theme.light")}</option>
                <option value="dark">{t("theme.dark")}</option>
              </select>
            </label>
          </div>
        </section>

        <section className="settings-section" aria-labelledby="analysis-title">
          <div>
            <h2 id="analysis-title">{t("settings.analysis")}</h2>
            <p>{t("settings.analysisHelp")}</p>
          </div>
          <div className="settings-fields settings-fields-three">
            <label className="field-control">
              <span>{t("settings.absoluteTolerance")}</span>
              <input
                dir="ltr"
                required
                inputMode="decimal"
                pattern="(?:0|[1-9][0-9]*)(?:\.[0-9]+)?"
                value={tolerances.absolute_tolerance}
                onChange={(event) =>
                  setTolerances((current) => ({
                    ...current,
                    absolute_tolerance: event.target.value,
                  }))
                }
              />
            </label>
            <label className="field-control">
              <span>{t("settings.relativeTolerance")}</span>
              <input
                dir="ltr"
                required
                inputMode="decimal"
                pattern="(?:0|[1-9][0-9]*)(?:\.[0-9]+)?"
                value={tolerances.relative_tolerance}
                onChange={(event) =>
                  setTolerances((current) => ({
                    ...current,
                    relative_tolerance: event.target.value,
                  }))
                }
              />
            </label>
            <label className="field-control">
              <span>{t("settings.decimalPrecision")}</span>
              <input
                type="number"
                min={0}
                max={6}
                value={tolerances.decimal_precision}
                onChange={(event) =>
                  setTolerances((current) => ({
                    ...current,
                    decimal_precision: Number(event.target.value),
                  }))
                }
              />
            </label>
          </div>
        </section>

        <section className="settings-section" aria-labelledby="privacy-title">
          <div>
            <h2 id="privacy-title">{t("settings.privacy")}</h2>
            <p>{t("settings.privacyHelp")}</p>
          </div>
          <div className="settings-fields">
            <div className="toggle-row">
              <span>
                <strong id="ai-features-label">{t("settings.aiFeatures")}</strong>
                <small id="ai-features-help">{t("settings.aiFeaturesHelp")}</small>
              </span>
              <input
                id="ai-features-enabled"
                type="checkbox"
                aria-labelledby="ai-features-label"
                aria-describedby="ai-features-help"
                checked={privacy.ai_features_enabled}
                onChange={(event) =>
                  setPrivacy((current) => ({
                    ...current,
                    ai_features_enabled: event.target.checked,
                  }))
                }
              />
            </div>
            {settings.privacy.ai_features_enabled ? (
              <AiAccountPanel />
            ) : (
              <p className="muted-copy">{t("settings.aiDisabled")}</p>
            )}
            <div className="toggle-row">
              <span>
                <strong id="diagnostic-logging-label">{t("settings.diagnosticLogging")}</strong>
                <small id="diagnostic-logging-help">{t("settings.diagnosticLoggingHelp")}</small>
              </span>
              <input
                id="diagnostic-logging-enabled"
                type="checkbox"
                aria-labelledby="diagnostic-logging-label"
                aria-describedby="diagnostic-logging-help"
                checked={privacy.diagnostic_logging_enabled}
                onChange={(event) =>
                  setPrivacy((current) => ({
                    ...current,
                    diagnostic_logging_enabled: event.target.checked,
                  }))
                }
              />
            </div>
          </div>
        </section>

        <section className="settings-section" aria-labelledby="updates-title">
          <div>
            <h2 id="updates-title">{t("settings.updates")}</h2>
            <p>{t("settings.updatesHelp")}</p>
          </div>
          <div className="settings-fields">
            <label className="field-control">
              <span>{t("settings.updateChannel")}</span>
              <select
                id="settings-update-channel"
                value={updateChannel}
                onChange={(event) => setUpdateChannel(event.target.value as UpdateChannel)}
              >
                <option value="stable">{t("settings.stable")}</option>
                <option value="beta">{t("settings.beta")}</option>
              </select>
            </label>
            <p className="muted-copy">
              {updateChannel !== settings.update_channel && t("settings.saveChannelBeforeCheck")}
            </p>
            <UpdatePanel
              channel={settings.update_channel}
              channelSelectionDirty={updateChannel !== settings.update_channel}
              lastSuccessfulCheck={settings.last_successful_update_check}
            />
          </div>
        </section>

        <section className="settings-section" aria-labelledby="advanced-title">
          <div>
            <h2 id="advanced-title">{t("settings.advanced")}</h2>
            <p>{t("settings.advancedHelp")}</p>
          </div>
          <div className="settings-fields">
            <div className="toggle-row">
              <span>
                <strong id="system-codex-label">{t("settings.systemCodex")}</strong>
                <small id="system-codex-help">{t("settings.systemCodexHelp")}</small>
              </span>
              <input
                id="system-codex-enabled"
                type="checkbox"
                aria-labelledby="system-codex-label"
                aria-describedby="system-codex-help"
                checked={advanced.use_system_codex}
                onChange={(event) =>
                  setAdvanced((current) => ({
                    ...current,
                    use_system_codex: event.target.checked,
                  }))
                }
              />
            </div>
            {advanced.use_system_codex && (
              <div className="system-codex-picker">
                <label className="field-control">
                  <span>{t("settings.systemCodexPath")}</span>
                  <input
                    dir="ltr"
                    readOnly
                    required
                    value={advanced.system_codex_binary ?? ""}
                    placeholder={t("settings.systemCodexNotSelected")}
                  />
                </label>
                <Button
                  type="button"
                  variant="ghost"
                  disabled={!desktopRuntimeAvailable()}
                  onClick={() => {
                    void desktopApi.chooseSystemCodex().then((path) => {
                      if (path) {
                        setAdvanced((current) => ({
                          ...current,
                          system_codex_binary: path,
                        }));
                      }
                    });
                  }}
                >
                  {t("settings.chooseSystemCodex")}
                </Button>
                <p className="muted-copy">{t("settings.systemCodexRestart")}</p>
              </div>
            )}
          </div>
        </section>

        <footer className="settings-actions">
          {saved && (
            <span role="status">
              <Icon name="check" size={17} />
              {t("settings.saved")}
            </span>
          )}
          <Button type="submit" data-testid="settings-save" disabled={busy}>
            {busy ? t("status.saving") : t("actions.saveChanges")}
          </Button>
        </footer>
      </form>
      <section className="settings-section settings-reset-section" aria-labelledby="reset-title">
        <div>
          <h2 id="reset-title">{t("settings.reset")}</h2>
          <p>{t("settings.resetHelp")}</p>
        </div>
        <div className="reset-panel">
          <div>
            <strong>{t("settings.resetTitle")}</strong>
            <p>{t("settings.resetSummary")}</p>
          </div>
          <Button variant="danger" onClick={() => setShowReset(true)}>
            {t("settings.resetAction")}
          </Button>
        </div>
      </section>
      {showReset ? <ResetOpenConKitDialog onClose={() => setShowReset(false)} /> : null}
    </main>
  );
}

interface UpdatePanelProps {
  channel: UpdateChannel;
  channelSelectionDirty: boolean;
  lastSuccessfulCheck: string | null;
}

function UpdatePanel({ channel, channelSelectionDirty, lastSuccessfulCheck }: UpdatePanelProps) {
  const { t, i18n } = useTranslation();
  const [result, setResult] = useState<UpdateCheckResult | null>(null);
  const [progress, setProgress] = useState<UpdateProgressEvent | null>(null);
  const [action, setAction] = useState<"checking" | "installing" | "opening" | null>(null);
  const [errorCode, setErrorCode] = useState<string | null>(null);

  useEffect(() => {
    if (!desktopRuntimeAvailable()) {
      return;
    }
    let disposed = false;
    let stopListening: (() => void) | undefined;
    void desktopApi
      .onUpdateProgress((event) => {
        if (!disposed) {
          setProgress(event);
        }
      })
      .then((unlisten) => {
        if (disposed) {
          unlisten();
        } else {
          stopListening = unlisten;
        }
      });
    return () => {
      disposed = true;
      stopListening?.();
    };
  }, []);

  const visibleResult = !channelSelectionDirty && result?.channel === channel ? result : null;
  const visibleProgress = channelSelectionDirty ? null : progress;

  const check = async () => {
    setAction("checking");
    setErrorCode(null);
    setProgress(null);
    try {
      setResult(await desktopApi.checkForUpdates());
    } catch (error: unknown) {
      setErrorCode(errorCodeOf(error));
    } finally {
      setAction(null);
    }
  };

  const install = async () => {
    const update = visibleResult?.update;
    if (!update) {
      return;
    }
    setAction("installing");
    setErrorCode(null);
    setProgress(null);
    try {
      await desktopApi.installUpdate(update.version, visibleResult.channel);
    } catch (error: unknown) {
      setErrorCode(errorCodeOf(error));
      setAction(null);
    }
  };

  const openManualDownload = async () => {
    const update = visibleResult?.update;
    if (!update) {
      return;
    }
    setAction("opening");
    setErrorCode(null);
    try {
      await desktopApi.openUpdateDownload(update.version);
    } catch (error: unknown) {
      setErrorCode(errorCodeOf(error));
    } finally {
      setAction(null);
    }
  };

  const checkedAt = visibleResult?.checked_at ?? lastSuccessfulCheck;
  const percent =
    visibleProgress?.total_bytes && visibleProgress.total_bytes > 0
      ? Math.min(100, (visibleProgress.downloaded_bytes / visibleProgress.total_bytes) * 100)
      : null;

  return (
    <div className="update-panel" aria-labelledby="update-status-title">
      <div className="section-heading">
        <div>
          <h3 id="update-status-title">{t("settings.updateStatus")}</h3>
          <p>
            {checkedAt
              ? t("settings.lastChecked", {
                  date: new Intl.DateTimeFormat(i18n.language, {
                    dateStyle: "medium",
                    timeStyle: "short",
                  }).format(new Date(checkedAt)),
                })
              : t("settings.notChecked")}
          </p>
        </div>
        <Button
          type="button"
          data-testid="check-for-updates"
          variant="ghost"
          disabled={!desktopRuntimeAvailable() || channelSelectionDirty || action !== null}
          onClick={() => void check()}
        >
          {action === "checking" ? t("settings.checkingUpdates") : t("settings.checkForUpdates")}
        </Button>
      </div>

      {!desktopRuntimeAvailable() && (
        <p className="muted-copy">{t("settings.updatesDesktopOnly")}</p>
      )}
      {errorCode && (
        <p role="alert" className="inline-error">
          {t(`errors.${errorCode}`, { defaultValue: t("errors.BACKGROUND_TASK_FAILED") })}
        </p>
      )}
      {visibleResult && !visibleResult.update && (
        <p role="status" className="update-current">
          <Icon name="check" size={17} />
          {t("settings.upToDate", { version: visibleResult.current_version })}
        </p>
      )}
      {visibleResult?.update && (
        <div className="available-update">
          <div className="available-update-heading">
            <div>
              <strong>
                {t("settings.updateAvailable", { version: visibleResult.update.version })}
              </strong>
              <span>
                {visibleResult.update.size_bytes !== null &&
                  formatFileSize(visibleResult.update.size_bytes, i18n.language)}
                {visibleResult.update.published_at &&
                  ` · ${new Intl.DateTimeFormat(i18n.language, {
                    dateStyle: "medium",
                  }).format(new Date(visibleResult.update.published_at))}`}
              </span>
            </div>
            {visibleResult.update.can_install ? (
              <Button type="button" disabled={action !== null} onClick={() => void install()}>
                {action === "installing"
                  ? t("settings.installingUpdate")
                  : t("settings.installUpdate")}
              </Button>
            ) : (
              <Button
                type="button"
                disabled={action !== null}
                onClick={() => void openManualDownload()}
              >
                {t("settings.downloadUpdate")}
              </Button>
            )}
          </div>
          {!visibleResult.update.can_install && (
            <p className="muted-copy">{t("settings.portableUpdateHelp")}</p>
          )}
          {visibleResult.update.notes && (
            <div className="release-notes">
              <h4>{t("settings.releaseNotes")}</h4>
              <p>{visibleResult.update.notes}</p>
            </div>
          )}
        </div>
      )}
      {visibleProgress && (
        <div className="update-progress" role="status" aria-live="polite">
          <span>{t(`settings.updatePhase.${visibleProgress.phase}`)}</span>
          {percent !== null && (
            <progress max={100} value={percent} aria-label={t("settings.updateProgress")} />
          )}
          <small>
            {formatFileSize(visibleProgress.downloaded_bytes, i18n.language)}
            {visibleProgress.total_bytes !== null &&
              ` / ${formatFileSize(visibleProgress.total_bytes, i18n.language)}`}
          </small>
        </div>
      )}
    </div>
  );
}

function formatFileSize(bytes: number, locale: string): string {
  const units = ["B", "KB", "MB", "GB"] as const;
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1_024 && unitIndex < units.length - 1) {
    value /= 1_024;
    unitIndex += 1;
  }
  return `${new Intl.NumberFormat(locale, {
    maximumFractionDigits: unitIndex === 0 ? 0 : 1,
  }).format(value)} ${units[unitIndex] ?? "GB"}`;
}

function AiAccountPanel() {
  const { t, i18n } = useTranslation();
  const [runtime, setRuntime] = useState<AiRuntimeStatus | null>(null);
  const [account, setAccount] = useState<AiAccountSnapshot | null>(null);
  const [limits, setLimits] = useState<AiRateLimitSnapshot | null>(null);
  const [challenge, setChallenge] = useState<AiLoginChallenge | null>(null);
  const [busy, setBusy] = useState(false);
  const [errorCode, setErrorCode] = useState<string | null>(null);

  const refresh = useCallback(async (refreshToken = false) => {
    if (!desktopRuntimeAvailable()) {
      return;
    }
    setBusy(true);
    setErrorCode(null);
    try {
      const nextRuntime = await desktopApi.aiRuntimeStatus();
      setRuntime(nextRuntime);
      if (!nextRuntime.enabled || !nextRuntime.selected_runtime_available) {
        setAccount(null);
        setLimits(null);
        return;
      }
      const nextAccount = await desktopApi.getAiAccount(refreshToken);
      setAccount(nextAccount);
      if (nextAccount.signed_in) {
        setLimits(await desktopApi.getAiRateLimits());
        setChallenge(null);
      } else {
        setLimits(null);
      }
    } catch (error: unknown) {
      setErrorCode(errorCodeOf(error));
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void refresh();
    }, 0);
    return () => window.clearTimeout(timer);
  }, [refresh]);

  const login = async (mode: "browser" | "device_code") => {
    setBusy(true);
    setErrorCode(null);
    try {
      setChallenge(await desktopApi.startAiLogin(mode));
    } catch (error: unknown) {
      setErrorCode(errorCodeOf(error));
    } finally {
      setBusy(false);
    }
  };

  const cancelLogin = async () => {
    if (!challenge) {
      return;
    }
    setBusy(true);
    try {
      await desktopApi.cancelAiLogin(challenge.login_id);
      setChallenge(null);
    } catch (error: unknown) {
      setErrorCode(errorCodeOf(error));
    } finally {
      setBusy(false);
    }
  };

  const logout = async () => {
    setBusy(true);
    setErrorCode(null);
    try {
      await desktopApi.logoutAi();
      setAccount(null);
      setLimits(null);
      await refresh();
    } catch (error: unknown) {
      setErrorCode(errorCodeOf(error));
      setBusy(false);
    }
  };

  if (!desktopRuntimeAvailable()) {
    return <p className="muted-copy">{t("settings.aiDesktopOnly")}</p>;
  }

  return (
    <div className="ai-account-panel" aria-labelledby="ai-account-title">
      <div className="section-heading">
        <div>
          <h3 id="ai-account-title">{t("settings.aiAccount")}</h3>
          <p>{t("settings.aiAccountHelp")}</p>
        </div>
        <Button type="button" variant="ghost" disabled={busy} onClick={() => void refresh(true)}>
          {t("actions.refresh")}
        </Button>
      </div>

      {errorCode && (
        <p role="alert" className="inline-error">
          {t(`errors.${errorCode}`, { defaultValue: t("errors.BACKGROUND_TASK_FAILED") })}
        </p>
      )}
      {runtime && !runtime.selected_runtime_available && (
        <p role="status" className="inline-error">
          {t("settings.aiRuntimeMissing")}
        </p>
      )}
      {runtime?.selected_runtime_available && (
        <p className="muted-copy">
          {runtime.using_system_runtime
            ? t("settings.aiSystemRuntime")
            : t("settings.aiBundledRuntime")}
        </p>
      )}
      {account?.signed_in ? (
        <div className="ai-account-details">
          <span className="status-pill status-completed">{t("settings.aiSignedIn")}</span>
          {account.masked_email && <strong dir="ltr">{account.masked_email}</strong>}
          <span>{t("settings.aiPlan", { plan: account.plan_type ?? "unknown" })}</span>
          <span dir="ltr">Codex {account.codex_version}</span>
          {limits?.primary && (
            <span>
              {t("settings.aiUsage", {
                percent: new Intl.NumberFormat(i18n.language).format(limits.primary.used_percent),
              })}
            </span>
          )}
          {(limits?.rate_limit_reached || limits?.spend_control_reached) && (
            <span className="text-status-error">{t("settings.aiLimitReached")}</span>
          )}
          <Button type="button" variant="ghost" disabled={busy} onClick={() => void logout()}>
            {t("settings.aiLogout")}
          </Button>
        </div>
      ) : (
        <div className="ai-login-actions">
          <p>{challenge ? t("settings.aiCompleteLogin") : t("settings.aiSignedOut")}</p>
          {challenge?.user_code && (
            <code dir="ltr" className="device-code">
              {challenge.user_code}
            </code>
          )}
          <Button
            type="button"
            data-testid="ai-login"
            disabled={busy}
            onClick={() => void login("browser")}
          >
            {t("settings.aiLogin")}
          </Button>
          <Button
            type="button"
            variant="ghost"
            disabled={busy}
            onClick={() => void login("device_code")}
          >
            {t("settings.aiDeviceLogin")}
          </Button>
          {challenge && (
            <Button
              type="button"
              variant="ghost"
              disabled={busy}
              onClick={() => void cancelLogin()}
            >
              {t("actions.cancel")}
            </Button>
          )}
        </div>
      )}
    </div>
  );
}

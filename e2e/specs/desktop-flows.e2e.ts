import { mkdir, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";

import { $, expect } from "@wdio/globals";
import { browser } from "@wdio/tauri-service";

import type { AppSettings } from "@openconkit/contracts";

import {
  aiRuntime,
  betaUpdate,
  bootstrap,
  browserLoginChallenge,
  defaultSettings,
  manifest,
  signedOutAccount,
} from "../fixtures.js";

const E2E_URL = "http://127.0.0.1:1421/?openconkit-e2e=1";
let completedTests = 0;

async function mockCommand(command: string, value: unknown) {
  const mock = await browser.tauri.mock(command);
  await mock.mockResolvedValue(value);
  return mock;
}

async function prepareApp(settings: AppSettings) {
  await browser.url(E2E_URL);
  await mockCommand("bootstrap_status", bootstrap);
  await mockCommand("get_settings", settings);
  await mockCommand("list_tool_manifests", [manifest]);
  await mockCommand("list_projects", []);
}

async function startApp() {
  await browser.execute(() => {
    window.dispatchEvent(new Event("openconkit:e2e-ready"));
  });
}

async function openSettings() {
  const settingsLink = await $('a[href="#/settings"]');
  await expect(settingsLink).toBeDisplayed();
  await settingsLink.click();
  await expect($("#settings-language")).toBeDisplayed();
}

describe("OpenConKit desktop renderer", () => {
  after(async () => {
    const resultDirectory = resolve("target", "e2e");
    await mkdir(resultDirectory, { recursive: true });
    await writeFile(
      join(resultDirectory, "wdio-result.json"),
      JSON.stringify({ completedTests }),
      "utf8",
    );
  });

  it("completes first-run onboarding through mocked Tauri settings", async () => {
    const firstRunSettings = { ...defaultSettings, onboarding_completed: false };
    const completedSettings = { ...firstRunSettings, onboarding_completed: true };
    await prepareApp(firstRunSettings);
    const updateSettings = await mockCommand("update_settings", completedSettings);

    await startApp();
    const continueButton = await $('[data-testid="welcome-continue"]');
    await expect(continueButton).toBeDisplayed();
    await continueButton.click();
    await expect($('a[href="#/settings"]')).toBeDisplayed();

    await updateSettings.update();
    expect(updateSettings).toHaveBeenCalledTimes(1);
    expect(updateSettings.mock.calls[0]?.[0]).toEqual({
      patch: {
        advanced: null,
        language: null,
        last_successful_update_check: null,
        onboarding_completed: true,
        privacy: null,
        theme: null,
        tolerances: null,
        update_channel: null,
      },
    });
    completedTests += 1;
  });

  it("persists Arabic RTL and dark theme choices", async () => {
    const arabicDarkSettings: AppSettings = {
      ...defaultSettings,
      language: "ar",
      theme: "dark",
    };
    await prepareApp(defaultSettings);
    const updateSettings = await mockCommand("update_settings", arabicDarkSettings);
    await startApp();
    await openSettings();

    await $("#settings-language").selectByAttribute("value", "ar");
    await $("#settings-theme").selectByAttribute("value", "dark");
    await $('[data-testid="settings-save"]').click();

    await browser.waitUntil(
      async () =>
        browser.execute(
          () =>
            document.documentElement.dir === "rtl" &&
            document.documentElement.dataset.theme === "dark",
        ),
      { timeoutMsg: "expected Arabic RTL and dark theme to be applied" },
    );
    await updateSettings.update();
    expect(updateSettings.mock.calls[0]?.[0]).toMatchObject({
      patch: { language: "ar", theme: "dark" },
    });
    completedTests += 1;
  });

  it("renders and starts the optional AI logged-out flow from mocks", async () => {
    const aiSettings: AppSettings = {
      ...defaultSettings,
      privacy: { ...defaultSettings.privacy, ai_features_enabled: true },
    };
    await prepareApp(aiSettings);
    const runtime = await mockCommand("ai_runtime_status", aiRuntime);
    const account = await mockCommand("get_ai_account", signedOutAccount);
    const login = await mockCommand("start_ai_login", browserLoginChallenge);
    await startApp();
    await openSettings();

    await expect($('[data-testid="ai-login"]')).toBeDisplayed();
    await $('[data-testid="ai-login"]').click();
    await expect(
      $("p=Complete sign-in in the browser, then refresh the account status."),
    ).toBeDisplayed();

    await runtime.update();
    await account.update();
    await login.update();
    expect(runtime).toHaveBeenCalledTimes(1);
    expect(account).toHaveBeenCalledTimes(1);
    expect(login.mock.calls[0]?.[0]).toEqual({ mode: "browser" });
    completedTests += 1;
  });

  it("requires saving a beta channel before checking its mocked feed", async () => {
    const betaSettings: AppSettings = { ...defaultSettings, update_channel: "beta" };
    await prepareApp(defaultSettings);
    const updateSettings = await mockCommand("update_settings", betaSettings);
    const checkForUpdates = await mockCommand("check_for_updates", betaUpdate);
    await startApp();
    await openSettings();

    await $("#settings-update-channel").selectByAttribute("value", "beta");
    await expect($('[data-testid="check-for-updates"]')).toBeDisabled();
    await $('[data-testid="settings-save"]').click();
    await expect($('[data-testid="check-for-updates"]')).toBeEnabled();
    await $('[data-testid="check-for-updates"]').click();
    await expect($("strong=OpenConKit 0.1.0-beta.1 is available")).toBeDisplayed();

    await updateSettings.update();
    await checkForUpdates.update();
    expect(updateSettings.mock.calls[0]?.[0]).toMatchObject({
      patch: { update_channel: "beta" },
    });
    expect(checkForUpdates).toHaveBeenCalledTimes(1);
    completedTests += 1;
  });
});

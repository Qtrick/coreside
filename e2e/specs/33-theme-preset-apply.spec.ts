import {
  closeSettings,
  invokeFromCurrentWindow,
  openSettings,
  scrollSettingsToHeading,
  waitForAppReady,
} from "../helpers.js";

/**
 * Journey 33 — saved theme apply confirmation (Cancel, Escape, Apply).
 *
 * Run alone:
 *   CORESIDE_E2E=1 AI_PROVIDER=mock npx wdio run e2e/wdio.conf.ts --spec e2e/specs/33-theme-preset-apply.spec.ts
 */
describe("Journey 33 — theme preset apply dialog", () => {
  const midnightName = `Midnight Matrix ${Date.now()}`;
  const morningName = `Morning Light ${Date.now()}`;

  const midnightSnapshot = {
    schemaVersion: 1,
    themePreference: "dark",
    accentPrimaryLight: "#164e63",
    accentPrimaryDark: "#67e8f9",
    accentSecondaryLight: "#7c2d12",
    accentSecondaryDark: "#fdba74",
    backgroundLight: "#ecfeff",
    backgroundDark: "#083344",
    surfaceLight: "#ffffff",
    surfaceDark: "#164e63",
    surfaceMutedLight: "#cffafe",
    surfaceMutedDark: "#155e75",
    borderLight: "#a5f3fc",
    borderDark: "#0e7490",
    textPrimaryLight: "#164e63",
    textPrimaryDark: "#ecfeff",
    textSecondaryLight: "#0e7490",
    textSecondaryDark: "#a5f3fc",
    wallpaper: { kind: "matrix" },
    wallpaperJson: "",
    interfaceTransparency: 42,
  };

  const morningSnapshot = {
    ...midnightSnapshot,
    themePreference: "light",
    accentPrimaryLight: "#9a3412",
    accentPrimaryDark: "#fb923c",
    backgroundLight: "#fff7ed",
    backgroundDark: "#431407",
    borderLight: "#fed7aa",
    borderDark: "#9a3412",
    textPrimaryLight: "#431407",
    textPrimaryDark: "#fff7ed",
    wallpaper: { kind: "aurora" },
    interfaceTransparency: 8,
  };

  type SettingsResult = {
    theme?: string;
    accentPrimaryLight?: string;
    accentPrimaryDark?: string;
    borderLight?: string;
    borderDark?: string;
    wallpaper?: { kind?: string };
    interfaceTransparency?: number;
  };

  async function invokeOrThrow<T>(command: string, args?: Record<string, unknown>) {
    const result = await invokeFromCurrentWindow(command, args);
    if (!result.ok) throw new Error(`${command} failed: ${result.error}`);
    return result.result as T;
  }

  function appearance(settings: SettingsResult) {
    return {
      theme: settings.theme,
      accentPrimaryLight: settings.accentPrimaryLight,
      accentPrimaryDark: settings.accentPrimaryDark,
      borderLight: settings.borderLight,
      borderDark: settings.borderDark,
      wallpaperKind: settings.wallpaper?.kind,
      interfaceTransparency: settings.interfaceTransparency,
    };
  }

  async function openCustomizationCategory() {
    await browser.execute(() => {
      const items = Array.from(
        document.querySelectorAll<HTMLButtonElement>(".settings-nav-item"),
      );
      const match = items.find((el) => {
        const lab = el.querySelector(".settings-nav-item-label");
        const text = lab?.textContent?.trim();
        return text === "Customization" || text === "Appearance";
      });
      match?.click();
    });
    await browser.waitUntil(
      async () => {
        const region = await $(".settings-content");
        if (!(await region.isExisting())) return false;
        if ((await region.getAttribute("aria-busy")) === "true") return false;
        const title = await $(".settings-group-title");
        if (!(await title.isExisting())) return false;
        const text = await title.getText();
        return text === "Customization" || text === "Appearance";
      },
      {
        timeout: 15_000,
        timeoutMsg: "Customization / Appearance category did not open",
      },
    );
  }

  async function openSavedThemesSection() {
    await openCustomizationCategory();
    await scrollSettingsToHeading("Saved Themes");
  }

  async function seedSavedThemes() {
    await invokeOrThrow("apply_appearance_bundle", {
      input: { snapshot: midnightSnapshot },
    });
    await invokeOrThrow("save_current_theme_preset", {
      input: { name: midnightName, description: "Desktop fidelity fixture" },
    });
    await invokeOrThrow("apply_appearance_bundle", {
      input: { snapshot: morningSnapshot },
    });
    await invokeOrThrow("save_current_theme_preset", {
      input: { name: morningName, description: "Cancellation baseline" },
    });

    await openCustomizationCategory();
    await scrollSettingsToHeading("Saved Themes");
    await browser.waitUntil(
      async () => {
        const body = await $("body").getText();
        return body.includes(midnightName) && body.includes(morningName);
      },
      { timeout: 15_000, timeoutMsg: "Saved theme fixtures did not appear" },
    );
  }

  async function applyDialogOpen() {
    return browser.execute(() =>
      Array.from(document.querySelectorAll("h2")).some((h) =>
        h.textContent?.includes("Apply"),
      ),
    );
  }

  async function openApplyDialog(themeName: string) {
    await browser.waitUntil(
      async () =>
        browser.execute((name) => {
          const card = Array.from(
            document.querySelectorAll<HTMLElement>(".theme-preset-card"),
          ).find((candidate) => candidate.textContent?.includes(name));
          const btn = Array.from(card?.querySelectorAll("button") ?? []).find(
            (candidate) => candidate.textContent?.trim() === "Apply Theme",
          );
          if (!btn) return false;
          btn.scrollIntoView({ block: "center" });
          btn.click();
          return true;
        }, themeName),
      {
        timeout: 15_000,
        timeoutMsg: "Apply Theme control not found in Saved Themes list",
      },
    );
    await browser.waitUntil(async () => applyDialogOpen(), {
      timeout: 8_000,
      timeoutMsg: "Apply theme confirmation dialog did not open",
    });
  }

  async function waitForApplyDialogClosed() {
    await browser.waitUntil(async () => !(await applyDialogOpen()), {
      timeout: 8_000,
      timeoutMsg: "Apply theme dialog did not close",
    });
  }

  async function appliedStatusVisible() {
    const status = await $(".theme-preset-status");
    if (!(await status.isExisting())) return false;
    const text = (await status.getText()).toLowerCase();
    return text.includes("applied");
  }

  it("Cancel and Escape preserve SQLite state; confirm restores the full saved appearance", async () => {
    await waitForAppReady();
    await openSettings();
    await openSavedThemesSection();
    await seedSavedThemes();

    const morningBefore = appearance(
      await invokeOrThrow<SettingsResult>("get_settings"),
    );
    expect(morningBefore).toEqual({
      theme: "light",
      accentPrimaryLight: "#9a3412",
      accentPrimaryDark: "#fb923c",
      borderLight: "#fed7aa",
      borderDark: "#9a3412",
      wallpaperKind: "aurora",
      interfaceTransparency: 8,
    });

    await openApplyDialog(midnightName);
    await browser.execute(() => {
      const dialog = Array.from(document.querySelectorAll("h2"))
        .find((h) => h.textContent?.includes("Apply"))
        ?.closest(".provider-modal");
      const buttons = dialog
        ? Array.from(dialog.querySelectorAll("button"))
        : [];
      buttons.find((b) => b.textContent?.trim() === "Cancel")?.click();
    });
    await waitForApplyDialogClosed();
    expect(await appliedStatusVisible()).toBe(false);
    expect(appearance(await invokeOrThrow<SettingsResult>("get_settings"))).toEqual(
      morningBefore,
    );

    await openApplyDialog(midnightName);
    await browser.keys("Escape");
    await waitForApplyDialogClosed();
    expect(await appliedStatusVisible()).toBe(false);
    expect(appearance(await invokeOrThrow<SettingsResult>("get_settings"))).toEqual(
      morningBefore,
    );

    await openApplyDialog(midnightName);
    await browser.execute(() => {
      const dialog = Array.from(document.querySelectorAll("h2"))
        .find((h) => h.textContent?.includes("Apply"))
        ?.closest(".provider-modal");
      const buttons = dialog
        ? Array.from(dialog.querySelectorAll("button"))
        : [];
      const confirm = buttons.filter(
        (b) => b.textContent?.trim() === "Apply Theme",
      );
      confirm[confirm.length - 1]?.click();
    });

    await browser.waitUntil(async () => appliedStatusVisible(), {
      timeout: 15_000,
      timeoutMsg: "Applied status did not appear after confirm",
    });
    await waitForApplyDialogClosed();

    expect(appearance(await invokeOrThrow<SettingsResult>("get_settings"))).toEqual({
      theme: "dark",
      accentPrimaryLight: "#164e63",
      accentPrimaryDark: "#67e8f9",
      borderLight: "#a5f3fc",
      borderDark: "#0e7490",
      wallpaperKind: "matrix",
      interfaceTransparency: 42,
    });

    await closeSettings();
    await expect($('[aria-label="New chat"]')).toBeDisplayed();
  });
});

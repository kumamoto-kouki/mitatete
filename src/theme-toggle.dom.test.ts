// @vitest-environment happy-dom
// テーマ切替トグル DOM テスト — ライト/ダーク切替ロジックと localStorage 永続化を検証する。

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { THEME_STORAGE_KEY, loadTheme, applyTheme } from "./theme";

// ─── トグルボタンのヘルパー ───────────────────────────────────────────────────
function createThemeToggle(): HTMLButtonElement {
  const btn = document.createElement("button");
  btn.id = "theme-toggle";
  btn.className = "theme-toggle";
  btn.type = "button";
  btn.setAttribute("aria-label", "ライト/ダークテーマを切り替え");

  const iconLight = document.createElement("span");
  iconLight.className = "theme-toggle__icon theme-toggle__icon--light";
  iconLight.setAttribute("aria-hidden", "true");

  const iconDark = document.createElement("span");
  iconDark.className = "theme-toggle__icon theme-toggle__icon--dark";
  iconDark.setAttribute("aria-hidden", "true");

  const label = document.createElement("span");
  label.className = "theme-toggle__label";
  label.textContent = "テーマ";

  btn.append(iconLight, iconDark, label);
  return btn;
}

describe("テーマ切替トグル", () => {
  let container: HTMLDivElement;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    localStorage.clear();
    // data-theme をリセット
    document.documentElement.removeAttribute("data-theme");
  });

  afterEach(() => {
    container.remove();
    localStorage.clear();
    document.documentElement.removeAttribute("data-theme");
  });

  it("loadTheme: localStorage に値がない場合は 'light' を返す", () => {
    expect(loadTheme()).toBe("light");
  });

  it("loadTheme: localStorage に 'dark' が保存されている場合は 'dark' を返す", () => {
    localStorage.setItem(THEME_STORAGE_KEY, "dark");
    expect(loadTheme()).toBe("dark");
  });

  it("loadTheme: localStorage に 'light' が保存されている場合は 'light' を返す", () => {
    localStorage.setItem(THEME_STORAGE_KEY, "light");
    expect(loadTheme()).toBe("light");
  });

  it("applyTheme('light'): data-theme='light' をセットし localStorage に保存する", () => {
    const toggle = createThemeToggle();
    container.appendChild(toggle);

    applyTheme("light");

    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("light");
  });

  it("applyTheme('dark'): data-theme='dark' をセットし localStorage に保存する", () => {
    const toggle = createThemeToggle();
    container.appendChild(toggle);

    applyTheme("dark");

    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("dark");
  });

  it("applyTheme('dark'): トグルの aria-pressed が 'true' になる", () => {
    const toggle = createThemeToggle();
    container.appendChild(toggle);

    applyTheme("dark");

    const btn = container.querySelector<HTMLButtonElement>("#theme-toggle");
    expect(btn?.getAttribute("aria-pressed")).toBe("true");
  });

  it("applyTheme('light'): トグルの aria-pressed が 'false' になる", () => {
    const toggle = createThemeToggle();
    container.appendChild(toggle);

    applyTheme("light");

    const btn = container.querySelector<HTMLButtonElement>("#theme-toggle");
    expect(btn?.getAttribute("aria-pressed")).toBe("false");
  });

  it("トグルボタンに #theme-toggle・.theme-toggle クラスが存在する", () => {
    const toggle = createThemeToggle();
    container.appendChild(toggle);

    expect(container.querySelector("#theme-toggle")).not.toBeNull();
    expect(container.querySelector(".theme-toggle")).not.toBeNull();
  });

  it("トグルに .theme-toggle__icon--light と .theme-toggle__icon--dark が含まれる", () => {
    const toggle = createThemeToggle();
    container.appendChild(toggle);

    expect(container.querySelector(".theme-toggle__icon--light")).not.toBeNull();
    expect(container.querySelector(".theme-toggle__icon--dark")).not.toBeNull();
  });
});

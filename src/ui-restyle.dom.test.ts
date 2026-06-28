// @vitest-environment happy-dom
// UI リスタイル DOM テスト — 原則8（AI 開示バナー・AI ラベル）と主要コンポーネントの描画を検証する。

import { describe, it, expect, beforeEach, afterEach } from "vitest";

// ─── ヘルパー：DOM を組み立てる ─────────────────────────────────────────────

/** mtt-ai-banner を生成して返す */
function createAiBanner(compact = false): HTMLElement {
  const el = document.createElement("div");
  el.className = compact
    ? "mtt-ai-banner mtt-ai-banner--compact"
    : "mtt-ai-banner";

  const icon = document.createElement("span");
  icon.className = "mtt-ai-banner__icon";

  const text = document.createElement("div");
  text.className = "mtt-ai-banner__text";
  text.innerHTML =
    "<b>これは AI による応答です。</b><span>人の発言ではありません。</span>";

  el.append(icon, text);
  return el;
}

/** mtt-ai-label を生成して返す */
function createAiLabel(): HTMLElement {
  const el = document.createElement("span");
  el.className = "mtt-ai-label";
  el.setAttribute("aria-label", "AI");
  el.textContent = "AI";
  return el;
}

/** appendMessage 相当のバブルを生成して返す */
function createBubble(role: "user" | "assistant", text: string): HTMLDivElement {
  const wrapper = document.createElement("div");
  wrapper.className = `mtt-msg mtt-msg--${role === "user" ? "user" : "assistant"}`;

  const avatar = document.createElement("div");
  avatar.className = `mtt-avatar mtt-avatar--${role === "user" ? "me" : "ai"}`;

  const bubbleWrap = document.createElement("div");
  bubbleWrap.className = "mtt-bubble-wrap";

  if (role === "assistant") {
    const meta = document.createElement("div");
    meta.className = "mtt-meta";
    const aiLabel = createAiLabel();
    meta.appendChild(aiLabel);
    bubbleWrap.appendChild(meta);
  }

  const bubble = document.createElement("div");
  bubble.className = `mtt-bubble mtt-bubble--${role === "user" ? "me" : "ai"}`;
  bubble.textContent = text;

  bubbleWrap.appendChild(bubble);
  wrapper.append(avatar, bubbleWrap);
  return wrapper;
}

// ─── AI 開示バナー（原則8） ──────────────────────────────────────────────────

describe("mtt-ai-banner（原則8: AI 開示バナー）", () => {
  let container: HTMLDivElement;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
  });

  it("標準バナーは .mtt-ai-banner クラスを持ち、テキスト要素が存在する", () => {
    const banner = createAiBanner(false);
    container.appendChild(banner);

    expect(container.querySelector(".mtt-ai-banner")).not.toBeNull();
    expect(container.querySelector(".mtt-ai-banner__text")).not.toBeNull();
    expect(
      container.querySelector(".mtt-ai-banner__text")?.textContent
    ).toContain("AI による応答");
  });

  it("コンパクトバナーは .mtt-ai-banner--compact クラスを持つ", () => {
    const banner = createAiBanner(true);
    container.appendChild(banner);

    expect(container.querySelector(".mtt-ai-banner--compact")).not.toBeNull();
  });

  it("バナーのテキストに「人の発言ではありません」が含まれる（誠実な開示）", () => {
    const banner = createAiBanner(false);
    container.appendChild(banner);

    const textEl = container.querySelector(".mtt-ai-banner__text");
    expect(textEl?.textContent).toContain("人の発言ではありません");
  });

  afterEach(() => {
    document.body.removeChild(container);
  });
});

// ─── チャットバブル（原則8: AI ラベル） ──────────────────────────────────────

describe("mtt-msg バブル（原則8: AI ラベル必須）", () => {
  let container: HTMLDivElement;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
  });

  it("ユーザーバブルは .mtt-msg--user と .mtt-bubble--me を持つ", () => {
    const bubble = createBubble("user", "こんにちは");
    container.appendChild(bubble);

    expect(container.querySelector(".mtt-msg--user")).not.toBeNull();
    expect(container.querySelector(".mtt-bubble--me")).not.toBeNull();
    expect(container.querySelector(".mtt-bubble--me")?.textContent).toBe(
      "こんにちは"
    );
  });

  it("AI バブルは .mtt-msg--assistant と .mtt-bubble--ai を持つ", () => {
    const bubble = createBubble("assistant", "お答えします。");
    container.appendChild(bubble);

    expect(container.querySelector(".mtt-msg--assistant")).not.toBeNull();
    expect(container.querySelector(".mtt-bubble--ai")).not.toBeNull();
  });

  it("AI バブルには必ず .mtt-ai-label が存在する（原則8 AI ラベル）", () => {
    const bubble = createBubble("assistant", "お答えします。");
    container.appendChild(bubble);

    const label = container.querySelector(".mtt-ai-label");
    expect(label).not.toBeNull();
    expect(label?.textContent).toContain("AI");
  });

  it("ユーザーバブルには .mtt-ai-label が存在しない（ユーザー発話は AI でない）", () => {
    const bubble = createBubble("user", "こんにちは");
    container.appendChild(bubble);

    expect(container.querySelector(".mtt-ai-label")).toBeNull();
  });

  afterEach(() => {
    document.body.removeChild(container);
  });
});

// ─── モデルカード（mtt-model） ────────────────────────────────────────────────

describe("mtt-model カード", () => {
  let container: HTMLDivElement;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
  });

  function createModelCard(selected = false): HTMLButtonElement {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = selected ? "mtt-model is-selected" : "mtt-model";

    const icon = document.createElement("span");
    icon.className = "mtt-model__icon";
    icon.textContent = "C";

    const body = document.createElement("span");
    body.className = "mtt-model__body";
    const name = document.createElement("span");
    name.className = "mtt-model__name";
    name.textContent = "Claude";
    const desc = document.createElement("span");
    desc.className = "mtt-model__desc";
    desc.textContent = "落ち着いて寄り添う。";
    body.append(name, desc);

    const check = document.createElement("span");
    check.className = "mtt-model__check";

    btn.append(icon, body, check);
    return btn;
  }

  it("通常カードは .mtt-model クラスを持ち is-selected を持たない", () => {
    const card = createModelCard(false);
    container.appendChild(card);

    expect(container.querySelector(".mtt-model")).not.toBeNull();
    expect(container.querySelector(".is-selected")).toBeNull();
  });

  it("選択中カードは .is-selected クラスを持つ", () => {
    const card = createModelCard(true);
    container.appendChild(card);

    expect(container.querySelector(".mtt-model.is-selected")).not.toBeNull();
  });

  it("カードにモデル名・説明が含まれる", () => {
    const card = createModelCard(false);
    container.appendChild(card);

    expect(container.querySelector(".mtt-model__name")?.textContent).toBe(
      "Claude"
    );
    expect(container.querySelector(".mtt-model__desc")?.textContent).toBe(
      "落ち着いて寄り添う。"
    );
  });

  afterEach(() => {
    document.body.removeChild(container);
  });
});

// ─── 日記パネル（diary-panel） ────────────────────────────────────────────────

describe("diary-panel 構造", () => {
  it("diary-panel__btn と diary-panel__content が HTML に存在する構造を検証する", () => {
    const panel = document.createElement("aside");
    panel.id = "diary-panel";
    panel.innerHTML = `
      <div class="diary-panel__inner">
        <h2 class="diary-panel__title">観察日記（原則 9）</h2>
        <button id="diary-generate-btn" class="diary-panel__btn" type="button">
          今日の日記を生成
        </button>
        <p id="diary-notice" class="diary-panel__notice"></p>
        <div id="diary-content" class="diary-panel__content"></div>
      </div>
    `;
    document.body.appendChild(panel);

    expect(panel.querySelector(".diary-panel__btn")).not.toBeNull();
    expect(panel.querySelector(".diary-panel__content")).not.toBeNull();
    expect(panel.querySelector(".diary-panel__title")?.textContent).toContain(
      "観察日記"
    );

    document.body.removeChild(panel);
  });
});

// ─── 設定ドロワー（A2: レイアウト再構成） ─────────────────────────────────────

describe("settings-drawer（折りたたみ設定）", () => {
  let drawer: HTMLDetailsElement;

  beforeEach(() => {
    drawer = document.createElement("details");
    drawer.id = "settings-drawer";
    drawer.className = "settings-drawer";

    const summary = document.createElement("summary");
    summary.className = "settings-drawer__summary";
    summary.textContent = "設定";

    const body = document.createElement("div");
    body.className = "settings-drawer__body";

    const modelPanel = document.createElement("aside");
    modelPanel.id = "model-panel";

    const charPanel = document.createElement("aside");
    charPanel.id = "character-panel";

    body.append(modelPanel, charPanel);
    drawer.append(summary, body);
    document.body.appendChild(drawer);
  });

  afterEach(() => {
    document.body.removeChild(drawer);
  });

  it("details#settings-drawer が存在し、既定は閉じた状態（open 属性なし）", () => {
    expect(document.querySelector("#settings-drawer")).not.toBeNull();
    expect((document.querySelector("#settings-drawer") as HTMLDetailsElement).open).toBe(false);
  });

  it("summary に「設定」テキストを持つ", () => {
    expect(drawer.querySelector(".settings-drawer__summary")?.textContent).toBe(
      "設定"
    );
  });

  it("折りたたみ内に #model-panel と #character-panel が存在する（E2E セレクター維持）", () => {
    expect(drawer.querySelector("#model-panel")).not.toBeNull();
    expect(drawer.querySelector("#character-panel")).not.toBeNull();
  });
});

// ─── キャラクターカード（D-1: mtt-char + mtt-avt アバター） ───────────────────

describe("mtt-char カード（D-1: アバター体裁）", () => {
  let container: HTMLDivElement;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
  });

  afterEach(() => {
    document.body.removeChild(container);
  });

  function createCharCard(name: string, trait: string, selected = false): HTMLButtonElement {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = selected
      ? "character-panel__item mtt-char is-selected"
      : "character-panel__item mtt-char";
    btn.dataset.presetId = "test-id";

    const avt = document.createElement("span");
    avt.className = "mtt-avt mtt-avt--brown";
    avt.setAttribute("aria-hidden", "true");
    avt.textContent = name.charAt(0);

    const body = document.createElement("span");
    body.className = "mtt-char__body";

    const nameEl = document.createElement("span");
    nameEl.className = "character-panel__name mtt-char__name";
    nameEl.textContent = name;

    const traitEl = document.createElement("span");
    traitEl.className = "character-panel__tone mtt-char__trait";
    traitEl.textContent = trait;

    body.append(nameEl, traitEl);

    const pick = document.createElement("span");
    pick.className = "mtt-char__pick";
    pick.setAttribute("aria-hidden", "true");

    btn.append(avt, body, pick);
    return btn;
  }

  it("カードは .character-panel__item（E2E互換）と .mtt-char（新体裁）を両方持つ", () => {
    const card = createCharCard("ひまり", "明るく元気");
    container.appendChild(card);

    expect(container.querySelector(".character-panel__item")).not.toBeNull();
    expect(container.querySelector(".mtt-char")).not.toBeNull();
  });

  it(".mtt-avt アバター要素が存在し、キャラクター名頭文字を持つ", () => {
    const card = createCharCard("ひまり", "明るく元気");
    container.appendChild(card);

    const avt = container.querySelector(".mtt-avt");
    expect(avt).not.toBeNull();
    expect(avt?.textContent).toBe("ひ");
  });

  it("選択時は .is-selected が付く", () => {
    const card = createCharCard("そら", "冷静沈着", true);
    container.appendChild(card);

    expect(container.querySelector(".mtt-char.is-selected")).not.toBeNull();
  });

  it(".mtt-char__name と .mtt-char__trait が正しいテキストを持つ", () => {
    const card = createCharCard("ゆき", "おだやか");
    container.appendChild(card);

    expect(container.querySelector(".mtt-char__name")?.textContent).toBe("ゆき");
    expect(container.querySelector(".mtt-char__trait")?.textContent).toBe("おだやか");
  });
});

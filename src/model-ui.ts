// モデル選択・API キー設定 UI。
// model-router の set_active_model / get_active_model / set_api_key / get_api_key_status を呼ぶ。
// API キーの平文はフロントへ返らない（get_api_key_status は有無のみ）。(要件 1.1, 3.1, 3.4)

import { invoke } from "@tauri-apps/api/core";

type Provider = "claude" | "openai" | "gemini";

interface ModelSelection {
  provider: Provider;
  model: string;
}
interface ApiKeyStatus {
  provider: Provider;
  has_key: boolean;
}

const PROVIDERS: {
  id: Provider;
  label: string;
  abbr: string;
  desc: string;
  defaultModel: string;
}[] = [
  {
    id: "claude",
    label: "Claude (Anthropic)",
    abbr: "C",
    desc: "落ち着いて寄り添う。",
    defaultModel: "claude-opus-4-8",
  },
  {
    id: "openai",
    label: "GPT (OpenAI)",
    abbr: "G",
    desc: "広い知識と柔軟な対話。",
    defaultModel: "gpt-4o",
  },
  {
    id: "gemini",
    label: "Gemini (Google)",
    abbr: "Gm",
    desc: "マルチモーダル・高速。",
    defaultModel: "gemini-1.5-pro",
  },
];

/** API キー有無の一覧を取得して設定状態の表示を更新する。 */
async function refreshKeyStatus(
  labels: Map<Provider, HTMLElement>
): Promise<void> {
  try {
    const status = await invoke<ApiKeyStatus[]>("get_api_key_status");
    for (const s of status) {
      const el = labels.get(s.provider);
      if (el) el.textContent = s.has_key ? "設定済み" : "未設定";
    }
  } catch (error) {
    console.error("API キー状態の取得に失敗しました。", error);
  }
}

/**
 * モデル選択・API キー設定パネルを #model-panel に構築する。
 * モデル選択は mtt-model カード UI（W-2）。
 */
export async function initModelUI(): Promise<void> {
  const root = document.querySelector<HTMLElement>("#model-panel");
  if (!root) return;
  root.replaceChildren();

  // ── モデル選択カード ──
  let activeProvider: Provider = "claude";
  try {
    const active = await invoke<ModelSelection>("get_active_model");
    activeProvider = active.provider;
  } catch {
    // Tauri 未実行時はデフォルト
  }

  const modelSection = document.createElement("div");
  modelSection.className = "model-ui__cards";

  const cardButtons = new Map<Provider, HTMLButtonElement>();

  const selectProvider = (provider: Provider): void => {
    activeProvider = provider;
    for (const [id, btn] of cardButtons) {
      btn.classList.toggle("is-selected", id === provider);
    }
    const def = PROVIDERS.find((p) => p.id === provider)!.defaultModel;
    void invoke("set_active_model", {
      selection: { provider, model: def },
    }).catch((e) => console.error("モデル切替に失敗しました。", e));
  };

  for (const p of PROVIDERS) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = p.id === activeProvider ? "mtt-model is-selected" : "mtt-model";
    btn.dataset.provider = p.id;

    const icon = document.createElement("span");
    icon.className = "mtt-model__icon";
    icon.textContent = p.abbr;

    const body = document.createElement("span");
    body.className = "mtt-model__body";

    const name = document.createElement("span");
    name.className = "mtt-model__name";
    const nameText = document.createTextNode(p.label);
    name.appendChild(nameText);

    const desc = document.createElement("span");
    desc.className = "mtt-model__desc";
    desc.textContent = p.desc;

    body.append(name, desc);

    const check = document.createElement("span");
    check.className = "mtt-model__check";
    check.setAttribute("aria-hidden", "true");

    btn.append(icon, body, check);
    btn.addEventListener("click", () => selectProvider(p.id));
    cardButtons.set(p.id, btn);
    modelSection.appendChild(btn);
  }

  // ── API キー設定（provider ごと） ──
  const keyList = document.createElement("div");
  keyList.className = "model-ui__keys";
  const statusLabels = new Map<Provider, HTMLElement>();
  for (const p of PROVIDERS) {
    const row = document.createElement("div");
    row.className = "model-ui__key-row";

    const status = document.createElement("span");
    status.className = "model-ui__key-status";
    status.textContent = "—";
    statusLabels.set(p.id, status);

    const input = document.createElement("input");
    input.type = "password";
    input.className = "model-ui__key-input";
    input.placeholder = `${p.label} の API キー`;

    const save = document.createElement("button");
    save.type = "button";
    save.textContent = "保存";
    save.addEventListener("click", async () => {
      const key = input.value.trim();
      if (!key) return;
      try {
        await invoke("set_api_key", { provider: p.id, key });
        input.value = ""; // 平文を画面に残さない
        await refreshKeyStatus(statusLabels);
      } catch (error) {
        console.error("API キーの保存に失敗しました。", error);
      }
    });

    const label = document.createElement("span");
    label.textContent = p.label;
    row.append(label, status, input, save);
    keyList.appendChild(row);
  }

  root.append(modelSection, keyList);
  await refreshKeyStatus(statusLabels);
}

// main ウィンドウ読み込み時に初期化する。
if (typeof document !== "undefined") {
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => void initModelUI());
  } else {
    void initModelUI();
  }
}

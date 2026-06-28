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

const PROVIDERS: { id: Provider; label: string; defaultModel: string }[] = [
  { id: "claude", label: "Claude (Anthropic)", defaultModel: "claude-opus-4-8" },
  { id: "openai", label: "GPT (OpenAI)", defaultModel: "gpt-4o" },
  { id: "gemini", label: "Gemini (Google)", defaultModel: "gemini-1.5-pro" },
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
 */
export async function initModelUI(): Promise<void> {
  const root = document.querySelector<HTMLElement>("#model-panel");
  if (!root) return;
  root.replaceChildren();

  // ── モデル選択 ──
  const select = document.createElement("select");
  select.className = "model-ui__select";
  select.setAttribute("aria-label", "モデル選択");
  for (const p of PROVIDERS) {
    const o = document.createElement("option");
    o.value = p.id;
    o.textContent = p.label;
    select.appendChild(o);
  }
  const modelInput = document.createElement("input");
  modelInput.type = "text";
  modelInput.className = "model-ui__model";
  modelInput.setAttribute("aria-label", "モデルID");

  const applyProvider = (provider: Provider): void => {
    const def = PROVIDERS.find((p) => p.id === provider)!.defaultModel;
    if (!modelInput.value) modelInput.value = def;
  };

  const switchModel = (): void => {
    const selection: ModelSelection = {
      provider: select.value as Provider,
      model: modelInput.value.trim(),
    };
    void invoke("set_active_model", { selection }).catch((e) =>
      console.error("モデル切替に失敗しました。", e)
    );
  };

  select.addEventListener("change", () => {
    modelInput.value = PROVIDERS.find((p) => p.id === select.value)!.defaultModel;
    switchModel();
  });
  modelInput.addEventListener("change", switchModel);

  // 起動時に現在のアクティブモデルを反映する。
  try {
    const active = await invoke<ModelSelection>("get_active_model");
    select.value = active.provider;
    modelInput.value = active.model;
  } catch {
    select.value = "claude";
    applyProvider("claude");
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

  root.append(select, modelInput, keyList);
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

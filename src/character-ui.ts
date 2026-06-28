// character-ui.ts
// プリセットキャラクターの一覧表示・選択UI。(要件 1.1, 1.4, 1.5)
//
// プリセット定義は public/presets/ 配下の静的JSONから読み込む。
// 実行時に「*.json」をグロブ列挙する手段は無いため、読み込むファイル名を列挙した
// マニフェスト public/presets/index.json を起点に各定義を fetch する。
//
// 一覧の読み込み・表示（4.1）に加え、選択時に CharacterSchema を生成して保存・アクティブ化する
// フロー（validate→store.save→store.setActive）を担う（4.2）。

import {
  CharacterValidator,
  type CharacterSchema,
} from "./character-validator";
import { CharacterStore } from "./character-store";
import { emit } from "@tauri-apps/api/event";

// Vite は public/ 配下をルート直下で配信する（public/presets/x.json → /presets/x.json）。
const PRESET_DIR = "/presets";
const MANIFEST_URL = `${PRESET_DIR}/index.json`;

// アクティブキャラクター変更を別ウィンドウ（character ウィンドウ）へ放送するイベント名。
export const CHARACTER_CHANGED_EVENT = "character:changed";

/** プリセット読み込みエラーの通知先（UIがメッセージ表示するためのフック）。 */
export type PresetErrorHandler = (message: string, error?: unknown) => void;

const defaultErrorHandler: PresetErrorHandler = (message, error) =>
  console.error(message, error);

/**
 * public/presets/ からプリセット定義を読み込み、CharacterSchema 配列として返す。(要件 1.1)
 *
 * - マニフェスト（index.json）の取得に失敗した場合は空配列を返し、エラーを通知する。(要件 1.5)
 * - 個別の定義ファイルが欠損・破損していても、読めたものだけを返す（部分縮退）。(要件 1.5)
 */
export async function loadPresets(
  onError: PresetErrorHandler = defaultErrorHandler
): Promise<CharacterSchema[]> {
  let fileNames: string[];
  try {
    const res = await fetch(MANIFEST_URL);
    if (!res.ok) throw new Error(`マニフェスト取得失敗: HTTP ${res.status}`);
    fileNames = (await res.json()) as string[];
  } catch (error) {
    onError("プリセット一覧の読み込みに失敗しました。", error);
    return [];
  }

  const presets: CharacterSchema[] = [];
  for (const file of fileNames) {
    try {
      const res = await fetch(`${PRESET_DIR}/${file}`);
      if (!res.ok) throw new Error(`定義取得失敗: HTTP ${res.status}`);
      presets.push((await res.json()) as CharacterSchema);
    } catch (error) {
      onError(`プリセット定義「${file}」の読み込みに失敗しました。`, error);
    }
  }
  return presets;
}

/** キャラクター ID からアバター配色クラスを決める（循環割り当て）。 */
const AVATAR_COLORS = ["mtt-avt--brown", "mtt-avt--green", "mtt-avt--blue"] as const;
function avatarColor(id: string): string {
  let hash = 0;
  for (const ch of id) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  return AVATAR_COLORS[hash % AVATAR_COLORS.length];
}

/**
 * プリセット一覧を container に描画する。(要件 1.1, 1.4)
 *
 * 各項目はクリック可能で、選択時に onSelect(preset) を発火する。選択項目には
 * 選択状態のスタイル（.is-selected）を付与する。
 * 各カードは mtt-char + mtt-avt アバター体裁（D-1）。
 */
export function renderPresetList(
  container: HTMLElement,
  presets: CharacterSchema[],
  onSelect: (preset: CharacterSchema) => void
): void {
  container.replaceChildren();

  if (presets.length === 0) {
    const empty = document.createElement("p");
    empty.className = "character-panel__empty";
    empty.textContent = "利用できるプリセットがありません。";
    container.appendChild(empty);
    return;
  }

  const list = document.createElement("ul");
  list.className = "character-panel__list mtt-chars";

  for (const preset of presets) {
    const item = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    // 後方互換: character-panel__item (E2E セレクター) + mtt-char (新体裁)
    button.className = "character-panel__item mtt-char";
    button.dataset.presetId = preset.id;

    // アバター
    const avt = document.createElement("span");
    avt.className = `mtt-avt ${avatarColor(preset.id)}`;
    avt.setAttribute("aria-hidden", "true");
    // プレースホルダー: 名前頭文字
    avt.textContent = preset.name.charAt(0);

    // ボディ
    const body = document.createElement("span");
    body.className = "mtt-char__body";

    const name = document.createElement("span");
    name.className = "character-panel__name mtt-char__name";
    name.textContent = preset.name;

    const trait = document.createElement("span");
    trait.className = "character-panel__tone mtt-char__trait";
    trait.textContent = preset.tone;

    body.append(name, trait);

    // 選択チェック
    const pick = document.createElement("span");
    pick.className = "mtt-char__pick";
    pick.setAttribute("aria-hidden", "true");

    button.append(avt, body, pick);
    button.addEventListener("click", () => {
      // 選択状態のハイライトを更新する。
      for (const el of list.querySelectorAll(".character-panel__item")) {
        el.classList.toggle("is-selected", el === button);
      }
      onSelect(preset);
    });

    item.appendChild(button);
    list.appendChild(item);
  }

  container.appendChild(list);
}

/**
 * プリセット選択時の生成・保存・アクティブ化フロー。(要件 1.2, 1.3)
 *
 * 選択された候補を `CharacterValidator.validate` に通して aiDisclosure を固定付与し（要件 1.3）、
 * `CharacterStore.save`（Tauriコマンド経由で永続化）→ `CharacterStore.setActive`（アクティブ化・
 * 下流通知）の順に呼び出す（要件 1.2）。
 *
 * 注: この関数はユーザーのプリセット選択（クリック）からのみ呼ばれる。setActive の
 * 「ユーザー操作起点限定」制約（character-store.ts）を満たす唯一の呼び出し経路の一つ。
 */
export async function selectPreset(
  candidate: CharacterSchema,
  onError: PresetErrorHandler = defaultErrorHandler
): Promise<void> {
  try {
    const schema = CharacterValidator.validate(candidate);
    await CharacterStore.save(schema);
    await CharacterStore.setActive(schema.id);
  } catch (error) {
    onError("キャラクターの選択に失敗しました。", error);
  }
}

/**
 * アクティブキャラクターを切り替える。(要件 4.1)
 *
 * セレクター操作（ユーザーUIイベント）からのみ呼ぶこと。CharacterStore.setActive の
 * 「ユーザー操作起点限定」制約を満たす正規経路。
 */
export function switchCharacter(id: string): Promise<void> {
  return CharacterStore.setActive(id);
}

/**
 * アクティブキャラクター変更を別ウィンドウ（character ウィンドウ）へ放送する接続を確立する。(要件 4.1, 4.4)
 *
 * main ウィンドウの CharacterStore は別 webview の character ウィンドウからは参照できないため、
 * store の購読をトリガーに Tauri イベントで CharacterSchema を放送する。character.ts が listen する。
 *
 * @returns 購読を解除する関数
 */
export function connectCrossWindow(): () => void {
  return CharacterStore.subscribe((schema: CharacterSchema) => {
    void emit(CHARACTER_CHANGED_EVENT, schema);
  });
}

/**
 * キャラクター切り替えセレクターを container に描画する。(要件 4.1)
 *
 * store が保持する全キャラクター（復元済みカスタム・選択済みプリセット・デフォルト）を選択肢にし、
 * 変更時に onSwitch(id) を発火する。
 */
export function renderSwitcher(
  container: HTMLElement,
  characters: CharacterSchema[],
  activeId: string | null,
  onSwitch: (id: string) => void
): void {
  container.replaceChildren();
  if (characters.length === 0) return;

  const select = document.createElement("select");
  select.className = "character-switcher__select";
  select.setAttribute("aria-label", "アクティブキャラクター");

  for (const character of characters) {
    const option = document.createElement("option");
    option.value = character.id;
    option.textContent = character.name;
    if (character.id === activeId) option.selected = true;
    select.appendChild(option);
  }

  select.addEventListener("change", () => onSwitch(select.value));
  container.appendChild(select);
}

/**
 * プリセット選択パネルと切り替えセレクターを初期化する。(要件 1.x, 4.1, 4.4)
 *
 * 1. 別ウィンドウ放送を接続し、store を init して保存済みキャラクターを復元する。
 * 2. プリセット一覧を読み込んで描画する（読み込みエラーはパネル内表示、要件 1.5）。
 * 3. store の全キャラクターから切り替えセレクターを描画し、store 変更時に再描画する。
 */
export async function initCharacterUI(
  onSelect?: (preset: CharacterSchema) => void
): Promise<void> {
  const panel = document.querySelector<HTMLElement>("#character-panel");
  if (!panel) return;

  const message = document.createElement("p");
  message.className = "character-panel__message";
  message.hidden = true;
  panel.appendChild(message);

  const switcherContainer = document.createElement("div");
  switcherContainer.className = "character-switcher";
  panel.appendChild(switcherContainer);

  const listContainer = document.createElement("div");
  panel.appendChild(listContainer);

  const showError: PresetErrorHandler = (text, error) => {
    console.error(text, error);
    message.hidden = false;
    message.textContent = text;
  };

  const handleSelect =
    onSelect ??
    ((preset: CharacterSchema) => void selectPreset(preset, showError));

  // 別ウィンドウ放送を接続してから store を復元する（init の初回通知も放送される）。
  connectCrossWindow();

  const renderCurrentSwitcher = (): void =>
    renderSwitcher(
      switcherContainer,
      CharacterStore.getAll(),
      CharacterStore.getActive()?.id ?? null,
      (id) => void switchCharacter(id)
    );
  // store 変更（init/save/setActive）のたびにセレクターを再描画する。
  CharacterStore.subscribe(renderCurrentSwitcher);

  await CharacterStore.init();

  const presets = await loadPresets(showError);
  renderPresetList(listContainer, presets, handleSelect);
  renderCurrentSwitcher();

  // 起動時に復元されたアクティブキャラクターを記録する（診断・要件5.2の復元確認用）。
  console.info(
    "[character-layer] 起動時アクティブキャラクター:",
    CharacterStore.getActive()?.id ?? "(なし)"
  );
}

// main ウィンドウ読み込み時に初期化する。
if (typeof document !== "undefined") {
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => void initCharacterUI());
  } else {
    void initCharacterUI();
  }
}

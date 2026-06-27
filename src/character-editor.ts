// character-editor.ts
// カスタムキャラクターの作成・編集フォーム。(要件 2.1, 2.2, 2.3, 2.4, 2.5)
//
// フロー（design フロー2）: 名前・口調・ビジュアルを入力 → 必須チェック →
// ビジュアル未設定ならデフォルトアバターを自動適用 → CharacterValidator.validate（aiDisclosure
// 固定付与）→ CharacterStore.save（Tauriコマンド経由で ~/.mitatete/characters/{id}.json へ永続化）。
//
// 注: 著作権同意フロー・VisualConfig(mode='upload') の本格対応はフェーズ2（タスク8.2,
// character-visual-editor.ts）の責務。本タスク（5.1）はテキスト入力＋画像の data URL 取り込みと
// デフォルトアバター縮退までを担う。アクティブ化（setActive）は切り替えUI（6.1）の責務とし、
// 作成時は保存のみ行う（design フロー2 と一致）。

import {
  CharacterValidator,
  type CharacterSchema,
  type VisualConfig,
} from "./character-validator";
import { CharacterStore } from "./character-store";
import {
  initVisualEditor,
  requestImageUpload,
  buildVisualSvg,
  svgToDataUri,
} from "./character-visual-editor";

// 内蔵デフォルトアバター（ビジュアル未設定時に自動適用、要件 2.4）。外部アセットに依存しない SVG。
export const DEFAULT_AVATAR =
  "data:image/svg+xml;utf8," +
  encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96" viewBox="0 0 96 96">' +
      '<circle cx="48" cy="48" r="46" fill="#f0e9df" stroke="#b08968" stroke-width="2"/>' +
      '<circle cx="48" cy="40" r="16" fill="#b08968"/>' +
      '<path d="M20 80a28 28 0 0 1 56 0z" fill="#b08968"/>' +
      "</svg>"
  );

export interface CustomCharacterInput {
  name: string;
  tone: string;
  /** 画像URL／data URL。未設定・空文字なら DEFAULT_AVATAR を適用する。 */
  visual?: string;
  /** フェーズ2：レイヤーテンプレート／アップロード画像のビジュアル設定（要件 6.2, 6.5）。 */
  visualConfig?: VisualConfig;
}

/**
 * visualConfig から表示用の visual 値を導出する。
 * - template: レイヤーSVGを data URI 化して表示に使う。
 * - upload: ローカルファイルパス（uploadedImagePath）を参照する（base64送信はしない、要件 6.5）。
 */
function visualFromConfig(config: VisualConfig): string | undefined {
  if (config.mode === "template" && config.templateParams) {
    return svgToDataUri(buildVisualSvg(config.templateParams));
  }
  if (config.mode === "upload") {
    return config.uploadedImagePath;
  }
  return undefined;
}

/**
 * 入力からカスタムキャラクターの CharacterSchema を生成する（検証済み）。
 *
 * - visualConfig があれば、それに基づく visual を優先する（テンプレSVG／アップロードパス）（要件 6.2, 6.5）。
 * - visual も visualConfig も無ければ DEFAULT_AVATAR を適用する（要件 2.4）。
 * - aiDisclosure は引数で一切指定せず、validate が固定文言を付与する（要件 2.3: フォームで編集不可）。
 * - name/tone が非空でない場合、validate が例外をスローする（要件 2.1）。
 */
export function buildCustomCharacter(
  input: CustomCharacterInput
): CharacterSchema {
  const fromConfig = input.visualConfig
    ? visualFromConfig(input.visualConfig)
    : undefined;
  const visual =
    fromConfig ??
    (input.visual && input.visual.trim() !== ""
      ? input.visual
      : DEFAULT_AVATAR);
  return CharacterValidator.validate({
    name: input.name,
    tone: input.tone,
    visual,
    isPreset: false,
    ...(input.visualConfig ? { visualConfig: input.visualConfig } : {}),
  });
}

/**
 * カスタムキャラクターを生成して永続化する。(要件 2.5)
 *
 * 保存のみを行い、アクティブ化は行わない（切り替えは 6.1 の責務、design フロー2）。
 */
export async function submitCustomCharacter(
  input: CustomCharacterInput
): Promise<CharacterSchema> {
  const schema = buildCustomCharacter(input);
  await CharacterStore.save(schema);
  return schema;
}

/**
 * カスタム作成フォームを #character-editor に構築する。
 *
 * - 名前・口調の入力、レイヤービジュアルエディター（8.1）、画像アップロード＋著作権同意（8.2）、
 *   保存ボタンを配置する（要件 2.1, 2.2, 6.1, 6.3）。
 * - aiDisclosure は読み取り専用テキストで表示し、編集不可とする（要件 2.3）。
 */
export function initCharacterEditor(): void {
  const root = document.querySelector<HTMLElement>("#character-editor");
  if (!root) return;
  root.replaceChildren();

  const form = document.createElement("form");
  form.className = "editor";

  const nameInput = document.createElement("input");
  nameInput.type = "text";
  nameInput.className = "editor__input";
  nameInput.placeholder = "名前";
  nameInput.required = true;

  const toneInput = document.createElement("textarea");
  toneInput.className = "editor__input";
  toneInput.placeholder = "口調（例: 丁寧で落ち着いた口調で話します。）";
  toneInput.required = true;

  // 8.1: レイヤー構造ビジュアルエディター（テンプレート）。リアルタイムプレビュー＋VisualConfig 収集。
  const visualEditorContainer = document.createElement("div");
  visualEditorContainer.className = "visual-editor";
  const getTemplateConfig = initVisualEditor(visualEditorContainer);

  // 8.2: 自作画像アップロード。同意取得後に upload の VisualConfig へ差し替える。
  let uploadConfig: VisualConfig | undefined;
  const fileInput = document.createElement("input");
  fileInput.type = "file";
  fileInput.accept = "image/png,image/svg+xml";
  fileInput.className = "editor__file";
  fileInput.addEventListener("change", async () => {
    const file = fileInput.files?.[0];
    if (!file) return;
    // 著作権注意文への同意を求める（要件 6.3）。拒否時は既存設定を維持する（要件 6.4）。
    const result = await requestImageUpload(
      file,
      (notice) => window.confirm(notice),
      uploadConfig ?? getTemplateConfig()
    );
    if (result?.mode === "upload") {
      uploadConfig = result;
      showMessage("画像を取り込みました（保存時に反映されます）。", false);
    } else {
      // 同意拒否・非対応形式：取り込みをキャンセルし、選択をリセットする。
      fileInput.value = "";
    }
  });

  // 原則8: aiDisclosure は固定・編集不可。読み取り専用テキストで明示する（要件 2.3）。
  const disclosure = document.createElement("p");
  disclosure.className = "editor__disclosure";
  disclosure.textContent = `固定表示: ${CharacterValidator.AI_DISCLOSURE}`;

  const message = document.createElement("p");
  message.className = "editor__message";
  message.hidden = true;

  const saveButton = document.createElement("button");
  saveButton.type = "submit";
  saveButton.className = "editor__save";
  saveButton.textContent = "保存";

  form.append(
    nameInput,
    toneInput,
    visualEditorContainer,
    fileInput,
    disclosure,
    saveButton,
    message
  );
  root.appendChild(form);

  function showMessage(text: string, isError: boolean): void {
    message.hidden = false;
    message.textContent = text;
    message.classList.toggle("editor__message--error", isError);
  }

  form.addEventListener("submit", async (e: SubmitEvent) => {
    e.preventDefault();
    try {
      // アップロード画像があればそれを優先、無ければレイヤーテンプレートの VisualConfig を使う。
      const visualConfig = uploadConfig ?? getTemplateConfig();
      const schema = await submitCustomCharacter({
        name: nameInput.value,
        tone: toneInput.value,
        visualConfig,
      });
      showMessage(`「${schema.name}」を保存しました。`, false);
      form.reset();
      uploadConfig = undefined;
    } catch (error) {
      console.error(error);
      showMessage("保存に失敗しました。名前と口調を入力してください。", true);
    }
  });
}

// main ウィンドウ読み込み時に初期化する。
if (typeof document !== "undefined") {
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => initCharacterEditor());
  } else {
    initCharacterEditor();
  }
}

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

// VisualConfig からテンプレートパラメータ型を導出する（visual-editor の内部型に依存しない）。
type TemplateParams = NonNullable<VisualConfig["templateParams"]>;

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
  /** 観察日記（原則9）を有効にするか。未指定は false（無効）。 */
  diaryEnabled?: boolean;
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
    diaryEnabled: input.diaryEnabled ?? false,
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

  /** 編集モード時に保持する元 id。新規作成時は null。 */
  let editingId: string | null = null;

  const form = document.createElement("form");
  form.className = "editor";

  const modeLabel = document.createElement("p");
  modeLabel.className = "editor__mode-label";
  modeLabel.hidden = true;
  modeLabel.textContent = "編集モード";

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

  // 編集セッションのビジュアル出所を明示管理する（L1: 編集保存で元アバターが失われる／既定テンプレに化けるのを防ぐ）。
  // 'preserve'=元 visual を保持 / 'template'=テンプレ編集を採用 / 'upload'=新規アップロードを採用。
  let editVisualMode: "preserve" | "template" | "upload" = "template";
  let editingOriginalVisual: string | undefined;
  let editingOriginalVisualConfig: VisualConfig | undefined;
  // initVisualEditor は初期化時に refresh()→onChange を1度呼ぶため、その分は dirty 扱いしない。
  let allowTemplateDirty = false;
  const onVisualChange = (): void => {
    // ユーザーがテンプレを操作したらテンプレ採用に切り替える（初期化時の refresh は除外）。
    if (allowTemplateDirty) editVisualMode = "template";
  };
  let getTemplateConfig = initVisualEditor(
    visualEditorContainer,
    undefined,
    onVisualChange
  );
  allowTemplateDirty = true;
  // 既存パラメータでビジュアルエディターを貼り直す（編集時の復元・新規時のリセットに使う）。
  const remountVisualEditor = (initial?: TemplateParams): void => {
    allowTemplateDirty = false;
    getTemplateConfig = initVisualEditor(
      visualEditorContainer,
      initial,
      onVisualChange
    );
    allowTemplateDirty = true;
  };

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
      editVisualMode = "upload"; // 編集セッションでは新規アップロードを採用する。
      showMessage("画像を取り込みました（保存時に反映されます）。", false);
    } else {
      // 同意拒否・非対応形式：取り込みをキャンセルし、選択をリセットする。
      fileInput.value = "";
    }
  });

  // 原則9: 観察日記 ON/OFF トグル（タスク D1）。既定 OFF。
  const diaryRow = document.createElement("div");
  diaryRow.className = "editor__diary-row";

  const diaryCheckbox = document.createElement("input");
  diaryCheckbox.type = "checkbox";
  diaryCheckbox.id = "editor-diary-enabled";
  diaryCheckbox.className = "editor__diary-checkbox";
  diaryCheckbox.checked = false;

  const diaryLabel = document.createElement("label");
  diaryLabel.htmlFor = "editor-diary-enabled";
  diaryLabel.className = "editor__diary-label";
  diaryLabel.textContent = "観察日記を有効にする（原則9）";

  const diaryHint = document.createElement("p");
  diaryHint.className = "editor__diary-hint";
  diaryHint.id = "editor-diary-hint";
  diaryHint.textContent =
    "会話の観察記録をAIが生成します。いつでも切り替えられます。";
  // 守屋レビュー#2: ヒントをチェックボックスへ関連付け（スクリーンリーダー）。
  diaryCheckbox.setAttribute("aria-describedby", "editor-diary-hint");

  diaryRow.append(diaryCheckbox, diaryLabel);

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

  const cancelButton = document.createElement("button");
  cancelButton.type = "button";
  cancelButton.className = "editor__cancel";
  cancelButton.textContent = "キャンセル";
  cancelButton.hidden = true;

  form.append(
    modeLabel,
    nameInput,
    toneInput,
    visualEditorContainer,
    fileInput,
    diaryRow,
    diaryHint,
    disclosure,
    saveButton,
    cancelButton,
    message
  );
  root.appendChild(form);

  function showMessage(text: string, isError: boolean): void {
    message.hidden = false;
    message.textContent = text;
    message.classList.toggle("editor__message--error", isError);
  }

  function resetToNewMode(): void {
    editingId = null;
    form.reset();
    diaryCheckbox.checked = false;
    uploadConfig = undefined;
    // 編集セッションのビジュアル状態を新規用に戻す（前回編集の残りを消す）。
    editingOriginalVisual = undefined;
    editingOriginalVisualConfig = undefined;
    editVisualMode = "template";
    remountVisualEditor();
    modeLabel.hidden = true;
    cancelButton.hidden = true;
    saveButton.textContent = "保存";
    message.hidden = true;
  }

  cancelButton.addEventListener("click", resetToNewMode);

  form.addEventListener("submit", async (e: SubmitEvent) => {
    e.preventDefault();
    try {
      // アップロード画像があればそれを優先、無ければレイヤーテンプレートの VisualConfig を使う。
      const visualConfig = uploadConfig ?? getTemplateConfig();
      const input: CustomCharacterInput = {
        name: nameInput.value,
        tone: toneInput.value,
        visualConfig,
        diaryEnabled: diaryCheckbox.checked,
      };

      let schema: CharacterSchema;
      if (editingId !== null) {
        // 編集モード: 元 id を保持して上書き save（新規 id を振らない）。
        // ビジュアルは editVisualMode で出所を確定する（L1: 触っていなければ元 visual を保持）。
        // 空文字は無効ビジュアルとみなし DEFAULT_AVATAR にフォールバックする。
        const keepOrDefault = (v: string | undefined): string =>
          v && v.trim() !== "" ? v : DEFAULT_AVATAR;
        let visual: string;
        let visualConfigForSchema: VisualConfig | undefined;
        if (editVisualMode === "upload" && uploadConfig) {
          visualConfigForSchema = uploadConfig;
          visual =
            uploadConfig.uploadedImagePath ?? keepOrDefault(editingOriginalVisual);
        } else if (editVisualMode === "template") {
          const cfg = getTemplateConfig();
          visualConfigForSchema = cfg;
          visual = cfg.templateParams
            ? svgToDataUri(buildVisualSvg(cfg.templateParams))
            : keepOrDefault(editingOriginalVisual);
        } else {
          // preserve: ユーザーがビジュアルに触れていない → 元の visual / visualConfig を保持する。
          visual = keepOrDefault(editingOriginalVisual);
          visualConfigForSchema = editingOriginalVisualConfig;
        }
        schema = CharacterValidator.validate({
          id: editingId,
          name: input.name,
          tone: input.tone,
          visual,
          isPreset: false,
          diaryEnabled: input.diaryEnabled ?? false,
          ...(visualConfigForSchema
            ? { visualConfig: visualConfigForSchema }
            : {}),
        });
        await CharacterStore.save(schema);
      } else {
        schema = await submitCustomCharacter(input);
      }

      showMessage(`「${schema.name}」を保存しました。`, false);
      resetToNewMode();
    } catch (error) {
      console.error(error);
      showMessage("保存に失敗しました。名前と口調を入力してください。", true);
    }
  });

  /**
   * エディタを編集モードに切り替え、既存 schema の値をフォームへ流し込む。
   * character-ui.ts の「編集」ボタンから呼ばれる想定。
   */
  (root as HTMLElement & { populateEditor?: (schema: CharacterSchema) => void }).populateEditor =
    (schema: CharacterSchema): void => {
      editingId = schema.id;
      nameInput.value = schema.name;
      toneInput.value = schema.tone;
      diaryCheckbox.checked = schema.diaryEnabled;
      uploadConfig = undefined;
      // ビジュアルを復元する（L1）。元の visual / visualConfig を覚え、保存時に保持できるようにする。
      editingOriginalVisual = schema.visual;
      editingOriginalVisualConfig = schema.visualConfig;
      if (
        schema.visualConfig?.mode === "template" &&
        schema.visualConfig.templateParams
      ) {
        // テンプレ生成だったキャラ: エディターを元パラメータで貼り直し、テンプレ採用にする。
        remountVisualEditor(schema.visualConfig.templateParams);
        editVisualMode = "template";
      } else {
        // アップロード画像／visualConfig 無し: エディターは既定へ戻し、触らない限り元 visual を保持する。
        remountVisualEditor();
        editVisualMode = "preserve";
      }
      modeLabel.hidden = false;
      modeLabel.textContent = `編集モード: ${schema.name}`;
      cancelButton.hidden = false;
      saveButton.textContent = "更新";
      message.hidden = true;
    };
}

// main ウィンドウ読み込み時に初期化する。
if (typeof document !== "undefined") {
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => initCharacterEditor());
  } else {
    initCharacterEditor();
  }
}

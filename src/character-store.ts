// character-store.ts
// CharacterSchema の唯一の権威ソース。アクティブキャラクターの保持・通知・
// 切り替え・永続化（Tauriコマンド経由）を管理する。(要件 3.3, 4.1, 4.2, 4.3, 5.2, 5.3, 5.4)
//
// 設計上の不変条件:
// - getActive() が返す CharacterSchema.aiDisclosure は常に AI_DISCLOSURE（非空）である。
// - 永続I/O はすべて Tauriコマンド経由のみ（フロントからファイルへ直接アクセスしない）。

import { invoke } from "@tauri-apps/api/core";
import {
  CharacterValidator,
  type CharacterSchema,
} from "./character-validator";

/**
 * 内蔵フォールバックキャラクター。
 *
 * プリセット定義ファイル（public/presets/*.json）の読み込みはタスク4.1の責務であり、
 * 本タスク（3.1）の時点ではまだ存在しない。そのため、init 失敗時・保存済みキャラ0件時の
 * 縮退動作（要件5.3）として、store 内に最小限の有効な CharacterSchema を1件だけ内蔵する。
 *
 * TODO(4.1): プリセット読み込み実装後、デフォルトは public/presets/ の第一候補へ差し替える。
 */
const DEFAULT_CHARACTER: CharacterSchema = CharacterValidator.validate({
  id: "default",
  name: "アシスタント",
  visual: "",
  tone: "丁寧で落ち着いた口調で話します。",
  isPreset: true,
});

type Listener = (schema: CharacterSchema) => void;

/** コレクション（保持集合の追加/編集/削除）変更を伝える購読者。ペイロードを取らない。 */
type ChangeListener = () => void;

/** load_characters 失敗などをUIへ伝えるためのエラーハンドラ（UIファイルへ依存しない軽量フック）。 */
type ErrorHandler = (error: unknown) => void;

// ─── モジュール内 state ───────────────────────────────────────────────────────
// id → CharacterSchema。getAll() の列挙順を安定させるため Map を使用する。
const characters = new Map<string, CharacterSchema>();
let activeId: string | null = null;
// 「アクティブが変わった」購読者（原則エンジン・クロスウィンドウ放送）。アクティブ Schema を受け取る。
const listeners = new Set<Listener>();
// 「コレクションが変わった」購読者（一覧・セレクター描画）。アクティブ一致と無関係に発火する。
// notify() とチャネルを分けるのは、非アクティブなキャラの編集/削除でも一覧は再描画が要るため（M1 根本対応）。
const changeListeners = new Set<ChangeListener>();
let errorHandler: ErrorHandler = (error) => console.error(error);

/** 現在のアクティブキャラクターを全購読者へ通知する。アクティブ未設定なら何もしない。 */
function notify(): void {
  if (activeId === null) return;
  const active = characters.get(activeId);
  if (active === undefined) return;
  for (const listener of listeners) {
    listener(active);
  }
}

/** コレクション変更を全購読者へ通知する（アクティブの有無に依存しない）。 */
function notifyChange(): void {
  for (const listener of changeListeners) {
    listener();
  }
}

/** 内蔵デフォルトキャラクターをアクティブとして登録する（縮退動作）。 */
function applyDefault(): void {
  characters.set(DEFAULT_CHARACTER.id, DEFAULT_CHARACTER);
  activeId = DEFAULT_CHARACTER.id;
}

// 「最後に使用したキャラクター」を settings.json に保存するキー（要件5.2）。
// 永続化先は storage-manager の settings コマンド（~/.mitatete/settings.json）を再利用する。
const LAST_ACTIVE_KEY = "lastActiveCharacterId";

/** settings.json に最後に使用したキャラクターIDを記録する（他の設定値は保持してマージする）。 */
async function persistLastActive(id: string): Promise<void> {
  try {
    const settings =
      (await invoke<Record<string, unknown>>("read_settings")) ?? {};
    await invoke("save_settings", {
      data: { ...settings, [LAST_ACTIVE_KEY]: id },
    });
  } catch (error) {
    // 永続化失敗はアクティブ化自体を妨げない（縮退）。
    errorHandler(error);
  }
}

/** settings.json から最後に使用したキャラクターIDを読み出す。未設定・失敗時は null。 */
async function readLastActive(): Promise<string | null> {
  try {
    const settings =
      (await invoke<Record<string, unknown>>("read_settings")) ?? {};
    const value = settings[LAST_ACTIVE_KEY];
    return typeof value === "string" ? value : null;
  } catch (error) {
    errorHandler(error);
    return null;
  }
}

export const CharacterStore = {
  /**
   * アプリ起動時にローカルファイルから保存済みキャラクターを復元する。(要件5.2)
   *
   * load_characters が失敗した場合、または保存済みキャラクターが0件の場合は、
   * 内蔵デフォルトキャラクターへフォールバックし、エラーを errorHandler に通知する。(要件5.3)
   */
  async init(): Promise<void> {
    characters.clear();
    activeId = null;
    try {
      const raw = await invoke<string[]>("load_characters");
      for (const json of raw) {
        try {
          // 破損ファイルは Rust 側でスキップ済みだが、念のためパース／検証で防御する。
          const schema = CharacterValidator.validate(
            JSON.parse(json) as Partial<CharacterSchema>
          );
          characters.set(schema.id, schema);
        } catch (error) {
          errorHandler(error);
        }
      }
      if (characters.size === 0) {
        applyDefault();
      } else {
        // 最後に使用したキャラクターを復元する（要件5.2）。記録が無い／復元集合に存在しない
        // 場合は先頭の復元キャラクターにフォールバックする。
        const lastId = await readLastActive();
        activeId =
          lastId !== null && characters.has(lastId)
            ? lastId
            : (characters.keys().next().value ?? null);
      }
    } catch (error) {
      errorHandler(error);
      applyDefault();
    }
    notify();
    notifyChange();
  },

  /** 現在のアクティブキャラクターを返す。未設定なら null。 */
  getActive(): CharacterSchema | null {
    if (activeId === null) return null;
    return characters.get(activeId) ?? null;
  },

  /** 既知の全キャラクター一覧を返す。（本タスクでは復元済みカスタム＋内蔵デフォルトのみ。プリセットfetchは4.1） */
  getAll(): CharacterSchema[] {
    return [...characters.values()];
  },

  /**
   * アクティブキャラクターを切り替え、購読者（原則エンジン・キャラクターウィンドウ）へ通知する。(要件4.1, 4.2)
   *
   * 設計上の制約: この関数は必ずユーザーのUI操作（クリック・セレクター操作）起点からのみ呼ぶこと。
   * 内部タイマーや AI レスポンスからの自動切り替えは禁止する（structure.md「設計上の不変条件」）。
   */
  async setActive(id: string): Promise<void> {
    if (!characters.has(id)) {
      throw new Error(`未知のキャラクターID: ${id}`);
    }
    activeId = id;
    notify();
    notifyChange(); // 選択ハイライトの更新のため一覧側にも知らせる。
    // 「最後に使用したキャラクター」として永続化する（次回起動時の復元用、要件5.2）。
    await persistLastActive(id);
  },

  /**
   * キャラクターを永続化する。(要件5.1)
   *
   * store 境界で再検証し、aiDisclosure 不変条件を担保したうえで Tauriコマンドへ渡す。
   */
  async save(schema: CharacterSchema): Promise<void> {
    const validated = CharacterValidator.validate(schema);
    await invoke("save_character", {
      name: validated.id,
      data: validated,
    });
    characters.set(validated.id, validated);
    if (activeId === validated.id) notify(); // アクティブ自身の内容が変わったときのみ active 購読者へ。
    notifyChange(); // 追加・編集はアクティブ非一致でも一覧へ反映が要る（M1 根本対応）。
  },

  /** カスタムキャラクターを削除する。(要件5.x) アクティブだった場合はデフォルトへ縮退する。 */
  async delete(id: string): Promise<void> {
    await invoke("delete_character", { name: id });
    characters.delete(id);
    if (activeId === id) {
      if (characters.size === 0) {
        applyDefault();
      } else {
        activeId = characters.keys().next().value ?? null;
      }
      notify(); // アクティブが消えてアクティブが移ったときのみ active 購読者へ。
    }
    notifyChange(); // 非アクティブの削除でも一覧から消えるよう常に発火する（M1 根本対応）。
  },

  /**
   * アクティブキャラクターの変更を購読する。(要件4.2, 4.3)
   * 原則エンジン（principles.ts）・キャラクターウィンドウ（character.ts）が使用する。
   *
   * @returns 購読を解除する関数
   */
  subscribe(listener: Listener): () => void {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  },

  /**
   * コレクション（保持集合）の変更を購読する。init/setActive/save/delete のたびに発火する。
   * 一覧・セレクター描画が使用する。アクティブ変更に限らないため、非アクティブなキャラの
   * 編集・削除でも再描画される（subscribe との違い）。
   *
   * @returns 購読を解除する関数
   */
  subscribeChange(listener: ChangeListener): () => void {
    changeListeners.add(listener);
    return () => {
      changeListeners.delete(listener);
    };
  },

  /** エラー通知先を差し替える（UIがトースト等を表示するためのフック）。 */
  setErrorHandler(handler: ErrorHandler): void {
    errorHandler = handler;
  },
};

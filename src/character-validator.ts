// character-validator.ts
// Schema検証・aiDisclosure不変性保証 (要件 3.1, 3.2, 3.3)

export interface VisualConfig {
  mode: "template" | "upload";
  templateParams?: {
    bodyType: "human" | "animal" | "thing" | "abstract";
    eyeShape: "round" | "narrow" | "star" | "dot";
    hairStyle: "short" | "long" | "bun" | "none" | "ears";
    outfitColor: string;
    skinColor: string;
  };
  uploadedImagePath?: string;
}

export interface CharacterSchema {
  id: string;
  name: string;
  visual: string;
  tone: string;
  aiDisclosure: string;
  principleDefaults: {
    固有性を与える: number;
    信頼から始める: number;
    一貫性を守る: number;
    余白を持つ: number;
    距離感を大切にする: number;
    行動で示す: number;
    多様な向き合い方を認める: number;
  };
  diaryEnabled: boolean;
  isPreset: boolean;
  visualConfig?: VisualConfig;
}

// 原則8の固定文言。いかなる入力によっても変更・上書きを受け付けない。
export const AI_DISCLOSURE = "私はAIアシスタントです。人間ではありません。";

const DEFAULT_PRINCIPLE_DEFAULTS: CharacterSchema["principleDefaults"] = {
  固有性を与える: 3,
  信頼から始める: 3,
  一貫性を守る: 3,
  余白を持つ: 3,
  距離感を大切にする: 3,
  行動で示す: 3,
  多様な向き合い方を認める: 3,
};

export const CharacterValidator = {
  // aiDisclosure の固定文言（外部から参照可能）
  AI_DISCLOSURE,

  /**
   * CharacterSchema 候補を検証し、aiDisclosure を強制付与して返す。
   *
   * 前提条件: candidate.name と candidate.tone が非空文字列であること。
   * 事後条件: 返り値の aiDisclosure は AI_DISCLOSURE と同一であること。
   * 不変条件: aiDisclosure はいかなる引数でも上書きできない。
   *
   * @throws {Error} name が空文字列またはホワイトスペースのみの場合
   * @throws {Error} tone が空文字列またはホワイトスペースのみの場合
   */
  validate(candidate: Partial<CharacterSchema>): CharacterSchema {
    const name = candidate.name ?? "";
    if (name.trim() === "") {
      throw new Error(
        "CharacterSchema の name は必須です（空文字またはホワイトスペースのみは無効）。"
      );
    }

    const tone = candidate.tone ?? "";
    if (tone.trim() === "") {
      throw new Error(
        "CharacterSchema の tone は必須です（空文字またはホワイトスペースのみは無効）。"
      );
    }

    // aiDisclosure は候補の値を一切参照せず、常に固定文言で上書きする（要件 3.2, 3.3）。
    return {
      id: candidate.id ?? crypto.randomUUID(),
      name: name.trim(),
      visual: candidate.visual ?? "",
      tone: tone.trim(),
      aiDisclosure: AI_DISCLOSURE,
      principleDefaults:
        candidate.principleDefaults ?? { ...DEFAULT_PRINCIPLE_DEFAULTS },
      diaryEnabled: candidate.diaryEnabled ?? false,
      isPreset: candidate.isPreset ?? false,
      ...(candidate.visualConfig !== undefined
        ? { visualConfig: candidate.visualConfig }
        : {}),
    };
  },
};

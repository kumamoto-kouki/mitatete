// character-visual-editor.ts
// フェーズ2：レイヤー構造ビジュアルエディター（8.1）と自作画像アップロード・著作権同意フロー（8.2）。
// VisualConfig を生成し、character-editor.ts から呼び出して CharacterSchema.visualConfig に格納する。
// (要件 6.1, 6.2, 6.3, 6.4, 6.5)

import type { VisualConfig } from "./character-validator";

type TemplateParams = NonNullable<VisualConfig["templateParams"]>;

// ─── テンプレート選択肢（要件 6.1） ───────────────────────────────────────────
export const BODY_TYPES: TemplateParams["bodyType"][] = [
  "human",
  "animal",
  "thing",
  "abstract",
];
export const EYE_SHAPES: TemplateParams["eyeShape"][] = [
  "round",
  "narrow",
  "star",
  "dot",
];
export const HAIR_STYLES: TemplateParams["hairStyle"][] = [
  "short",
  "long",
  "bun",
  "none",
  "ears",
];

export const DEFAULT_TEMPLATE_PARAMS: TemplateParams = {
  bodyType: "human",
  eyeShape: "round",
  hairStyle: "short",
  outfitColor: "#b08968",
  skinColor: "#f0e0d0",
};

// ─── 8.1 テンプレートエディター ───────────────────────────────────────────────

/** テンプレートパラメータから VisualConfig（mode: 'template'）を生成する。(要件 6.2) */
export function buildTemplateVisualConfig(
  params: TemplateParams
): VisualConfig {
  return { mode: "template", templateParams: { ...params } };
}

/** 目のレイヤー SVG を返す。 */
function eyeLayer(shape: TemplateParams["eyeShape"]): string {
  switch (shape) {
    case "narrow":
      return '<rect x="30" y="44" width="12" height="3" rx="1.5" fill="#2b2622"/><rect x="54" y="44" width="12" height="3" rx="1.5" fill="#2b2622"/>';
    case "star":
      return '<text x="30" y="49" font-size="12" fill="#2b2622">★</text><text x="54" y="49" font-size="12" fill="#2b2622">★</text>';
    case "dot":
      return '<circle cx="36" cy="45" r="2" fill="#2b2622"/><circle cx="60" cy="45" r="2" fill="#2b2622"/>';
    case "round":
    default:
      return '<circle cx="36" cy="45" r="4" fill="#2b2622"/><circle cx="60" cy="45" r="4" fill="#2b2622"/>';
  }
}

/** 髪のレイヤー SVG を返す。 */
function hairLayer(style: TemplateParams["hairStyle"]): string {
  switch (style) {
    case "long":
      return '<path d="M20 36a28 28 0 0 1 56 0v34H66V40a18 18 0 0 0-36 0v30H20z" fill="#5a4632"/>';
    case "bun":
      return '<circle cx="48" cy="14" r="9" fill="#5a4632"/><path d="M22 38a26 26 0 0 1 52 0z" fill="#5a4632"/>';
    case "none":
      return "";
    case "ears":
      return '<polygon points="26,12 36,30 18,28" fill="#5a4632"/><polygon points="70,12 78,28 60,30" fill="#5a4632"/>';
    case "short":
    default:
      return '<path d="M22 38a26 26 0 0 1 52 0v2H22z" fill="#5a4632"/>';
  }
}

/**
 * テンプレートパラメータからプレビュー SVG マークアップ文字列を生成する。(要件 6.1)
 *
 * 体型・目・髪・服の色・肌色のレイヤーを重ねる。リアルタイムプレビューはこの文字列を再生成して反映する。
 */
export function buildVisualSvg(params: TemplateParams): string {
  const { bodyType, eyeShape, hairStyle, outfitColor, skinColor } = params;
  // 体型による下地の形（顔/体）。
  const base =
    bodyType === "abstract"
      ? `<rect x="14" y="14" width="68" height="68" rx="14" fill="${skinColor}"/>`
      : `<circle cx="48" cy="46" r="32" fill="${skinColor}"/>`;
  // 動物体型は耳を足す（hairStyle と独立の体型マーカー）。
  const bodyMarker =
    bodyType === "animal"
      ? '<polygon points="24,18 34,34 16,32" fill="#5a4632"/><polygon points="72,18 80,32 62,34" fill="#5a4632"/>'
      : "";
  const outfit = `<path d="M16 84a32 32 0 0 1 64 0z" fill="${outfitColor}"/>`;
  return [
    '<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96" viewBox="0 0 96 96" data-body="' +
      bodyType +
      '">',
    base,
    bodyMarker,
    hairLayer(hairStyle),
    eyeLayer(eyeShape),
    outfit,
    "</svg>",
  ].join("");
}

/** SVG マークアップを data URI（img の src 用）に変換する。 */
export function svgToDataUri(svg: string): string {
  return "data:image/svg+xml;utf8," + encodeURIComponent(svg);
}

/**
 * レイヤー構造ビジュアルエディターを container に構築する。(要件 6.1, 6.2)
 *
 * 体型・目・髪・服の色・肌色の選択UIとリアルタイム SVG プレビューを表示し、変更のたびに
 * onChange(VisualConfig) を発火する。character-editor.ts から呼び出して visualConfig を収集する。
 *
 * @returns 現在の VisualConfig を取得する関数
 */
export function initVisualEditor(
  container: HTMLElement,
  initial: TemplateParams = DEFAULT_TEMPLATE_PARAMS,
  onChange?: (config: VisualConfig) => void
): () => VisualConfig {
  const params: TemplateParams = { ...initial };
  container.replaceChildren();

  const preview = document.createElement("img");
  preview.className = "visual-editor__preview";
  preview.width = 96;
  preview.height = 96;

  const refresh = (): void => {
    preview.src = svgToDataUri(buildVisualSvg(params));
    preview.alt = "ビジュアルプレビュー";
    onChange?.(buildTemplateVisualConfig(params));
  };

  const addSelect = <K extends keyof TemplateParams>(
    label: string,
    key: K,
    options: TemplateParams[K][]
  ): void => {
    const wrap = document.createElement("label");
    wrap.className = "visual-editor__field";
    wrap.textContent = label;
    const select = document.createElement("select");
    for (const opt of options) {
      const o = document.createElement("option");
      o.value = String(opt);
      o.textContent = String(opt);
      if (params[key] === opt) o.selected = true;
      select.appendChild(o);
    }
    select.addEventListener("change", () => {
      params[key] = select.value as TemplateParams[K];
      refresh();
    });
    wrap.appendChild(select);
    container.appendChild(wrap);
  };

  const addColor = (label: string, key: "outfitColor" | "skinColor"): void => {
    const wrap = document.createElement("label");
    wrap.className = "visual-editor__field";
    wrap.textContent = label;
    const input = document.createElement("input");
    input.type = "color";
    input.value = params[key];
    input.addEventListener("input", () => {
      params[key] = input.value;
      refresh();
    });
    wrap.appendChild(input);
    container.appendChild(wrap);
  };

  container.appendChild(preview);
  addSelect("体型", "bodyType", BODY_TYPES);
  addSelect("目", "eyeShape", EYE_SHAPES);
  addSelect("髪", "hairStyle", HAIR_STYLES);
  addColor("服の色", "outfitColor");
  addColor("肌色", "skinColor");

  refresh();
  return () => buildTemplateVisualConfig(params);
}

// ─── 8.2 自作画像アップロード・著作権同意フロー ───────────────────────────────

// アップロード前に表示する著作権注意文（要件 6.3）。
export const COPYRIGHT_NOTICE =
  "既存のアニメ・ゲーム・商標キャラクターに似せた画像のアップロードは著作権侵害になる場合があります。アップロードする画像がご自身に権利のあるものであることを確認してください。";

const ACCEPTED_UPLOAD_TYPES = ["image/png", "image/svg+xml"];

/** 同意取得の抽象（テストや UI 差し替えのため注入可能）。true=同意。 */
export type ConsentPrompt = (notice: string) => boolean | Promise<boolean>;

/** ローカルファイルパス解決の抽象。Tauri dialog プラグイン導入後に実パスへ差し替える（現状は名前）。 */
export type PathResolver = (file: File) => string;

/**
 * 自作画像アップロードを著作権同意ゲートを通して処理する。(要件 6.3, 6.4, 6.5)
 *
 * - PNG/SVG 以外は受け付けず、既存設定を維持する（要件 6.3）。
 * - アップロード前に著作権注意文の同意を求め、同意した場合のみ進める（要件 6.3, 6.4）。
 * - 同意拒否時はキャンセルして既存のビジュアル設定を維持する（要件 6.4）。
 * - 同意時は VisualConfig（mode: 'upload', uploadedImagePath）を返す（要件 6.5）。
 *   画像はローカルパス参照のみ（base64 でのネットワーク送信は行わない）。
 */
export async function requestImageUpload(
  file: File,
  getConsent: ConsentPrompt,
  currentConfig?: VisualConfig,
  resolvePath: PathResolver = (f) => f.name
): Promise<VisualConfig | undefined> {
  if (!ACCEPTED_UPLOAD_TYPES.includes(file.type)) {
    // 受け付けない形式：既存設定を維持する。
    return currentConfig;
  }
  const consented = await getConsent(COPYRIGHT_NOTICE);
  if (!consented) {
    // 同意拒否：アップロードをキャンセルし、既存設定を維持する（要件 6.4）。
    return currentConfig;
  }
  return { mode: "upload", uploadedImagePath: resolvePath(file) };
}

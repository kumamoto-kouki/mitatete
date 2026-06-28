// テーマ初期化（FOUC 防止・望月レビュー F-1）。
// head で同期実行し、最初のペイント前に保存済みテーマ（既定 light）を
// data-theme へ反映する。外部 'self' スクリプトなので CSP(script-src 'self') 準拠。
(function () {
  try {
    var t = localStorage.getItem("mitatete-theme");
    document.documentElement.setAttribute(
      "data-theme",
      t === "dark" ? "dark" : "light"
    );
  } catch (e) {
    document.documentElement.setAttribute("data-theme", "light");
  }
})();

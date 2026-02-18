import { defineConfig, presetUno } from "unocss";

export default defineConfig({
  presets: [presetUno()],
  theme: {
    colors: {
      "bg-0": "var(--bg-0)",
      "bg-1": "var(--bg-1)",
      "bg-2": "var(--bg-2)",
      "bg-3": "var(--bg-3)",
      "bg-4": "var(--bg-4)",
      panel: "var(--panel)",
      "panel-soft": "var(--panel-soft)",
      "ink-0": "var(--ink-0)",
      "ink-1": "var(--ink-1)",
      "ink-2": "var(--ink-2)",
      line: "var(--line)",
      "line-soft": "var(--line-soft)",
      brand: "var(--brand)",
      "brand-strong": "var(--brand-strong)",
      danger: "var(--danger)",
      "danger-panel": "var(--danger-panel)",
      "danger-panel-strong": "var(--danger-panel-strong)",
      "danger-ink": "var(--danger-ink)",
      ok: "var(--ok)",
    },
    borderRadius: {
      panel: "1rem",
      control: "0.75rem",
    },
    boxShadow: {
      panel: "0 0.6rem 1.45rem rgba(2, 8, 24, 0.28)",
    },
    fontFamily: {
      main: "var(--font-main)",
      code: "var(--font-code)",
    },
  },
  shortcuts: {
    "fx-panel":
      "border border-line rounded-panel bg-panel backdrop-blur-[10px] shadow-panel",
    "fx-chip":
      "inline-flex items-center rounded-control border border-line-soft bg-bg-3 px-2 py-1 text-sm text-ink-1",
    "fx-button":
      "inline-flex items-center justify-center rounded-control border border-line-soft bg-bg-3 px-3 py-2 font-semibold text-ink-0 transition-transform duration-[140ms] ease-out hover:-translate-y-px",
  },
});

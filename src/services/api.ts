// 呼叫後端的前端封裝。前端絕不直接打第三方 API（架構紅線）。
import { invoke } from "@tauri-apps/api/core";

export type Provider = "google" | "deepl";

export interface Settings {
  provider: Provider;
  target_lang: string;
  double_press_ms: number;
  idle_close_ms: number;
  always_on_monitor: boolean;
}

export type GlanceState =
  | { kind: "loading"; original: string; truncated: boolean; target_lang: string; provider: string }
  | {
      kind: "result";
      original: string;
      translated: string;
      detected_source: string | null;
      truncated: boolean;
      target_lang: string;
      provider: string;
    }
  | { kind: "secret" }
  | { kind: "error"; message: string };

export const GLANCE_STATE_EVENT = "glance://state";
export const GLANCE_WILL_HIDE_EVENT = "glance://will-hide";

export const api = {
  accessibilityStatus: () => invoke<boolean>("accessibility_status"),
  requestAccessibility: () => invoke<void>("request_accessibility"),
  openAccessibilitySettings: () => invoke<void>("open_accessibility_settings"),
  glanceActivity: () => invoke<void>("glance_activity"),
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<void>("set_settings", { settings }),
  setApiKey: (provider: Provider, key: string) => invoke<void>("set_api_key", { provider, key }),
  apiKeySet: (provider: Provider) => invoke<boolean>("api_key_set", { provider }),
  clearApiKey: (provider: Provider) => invoke<void>("clear_api_key", { provider }),
};

export const LANG_OPTIONS: { value: string; label: string }[] = [
  { value: "zh-TW", label: "繁體中文（台灣）" },
  { value: "zh-CN", label: "簡體中文" },
  { value: "en", label: "English" },
  { value: "ja", label: "日本語" },
  { value: "ko", label: "한국어" },
];

export function langShortLabel(code: string): string {
  const map: Record<string, string> = {
    "zh-TW": "繁中",
    "zh-CN": "簡中",
    en: "EN",
    ja: "日",
    ko: "韓",
  };
  return map[code] ?? code.toUpperCase();
}

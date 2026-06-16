// 呼叫後端的前端封裝。前端絕不直接打第三方 API（架構紅線）。
import { invoke } from "@tauri-apps/api/core";

export type Provider = "google" | "deepl";

export type LangMode = "fixed" | "pairing";

export interface Settings {
  provider: Provider;
  lang_mode: LangMode;
  target_lang: string;
  my_lang: string;
  counterpart_lang: string;
  double_press_ms: number;
  idle_close_ms: number;
  always_on_monitor: boolean;
  google_key_set: boolean;
  deepl_key_set: boolean;
  gemini_key_set: boolean;
}

export interface WorkbenchInput {
  original: string;
  translated: string;
  target_lang: string;
}

export type WbTranslation =
  | {
      kind: "ok";
      translated: string;
      detected_source: string | null;
      truncated: boolean;
      target_lang: string;
    }
  | { kind: "secret" }
  | { kind: "empty" }
  | { kind: "error"; message: string };

export interface DictMeaning {
  part_of_speech: string;
  definitions: string[];
}

export interface DictionaryEntry {
  word: string;
  phonetic: string | null;
  meanings: DictMeaning[];
}

export interface DictLookup {
  entry: DictionaryEntry | null;
  lemma: string; // 還原後的原形（快取鍵用）
}

export const WORKBENCH_INPUT_EVENT = "workbench://input";
export const LLM_TOKEN_EVENT = "workbench://llm-token";
export const LLM_DONE_EVENT = "workbench://llm-done";
export const LLM_ERROR_EVENT = "workbench://llm-error";
// 上段字典查無 → Gemini 短釋義補充（與下段文法分開的事件通道）。
export const DEF_TOKEN_EVENT = "workbench://def-token";
export const DEF_DONE_EVENT = "workbench://def-done";
export const DEF_ERROR_EVENT = "workbench://def-error";

export type LlmEvent =
  | { kind: "token"; seq: number; delta: string }
  | { kind: "done"; seq: number }
  | { kind: "error"; seq: number; message: string };

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
  setLlmKey: (key: string) => invoke<void>("set_llm_key", { key }),
  llmKeySet: () => invoke<boolean>("llm_key_set"),
  clearLlmKey: () => invoke<void>("clear_llm_key"),
  hideGlance: () => invoke<void>("hide_glance"),
  // Workbench
  openWorkbench: (original: string, translated: string, targetLang: string) =>
    invoke<void>("open_workbench", { original, translated, targetLang }),
  getWorkbenchInput: () => invoke<WorkbenchInput | null>("get_workbench_input"),
  closeWorkbench: () => invoke<void>("close_workbench"),
  workbenchTranslate: (text: string) => invoke<WbTranslation>("workbench_translate", { text }),
  dictionaryLookup: (word: string) => invoke<DictLookup>("dictionary_lookup", { word }),
  geminiExplain: (word: string, sentence: string, targetLang: string) =>
    invoke<number>("gemini_explain", { word, sentence, targetLang }),
  geminiDefine: (word: string, sentence: string, targetLang: string) =>
    invoke<number>("gemini_define", { word, sentence, targetLang }),
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

import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  api,
  DEF_DONE_EVENT,
  DEF_ERROR_EVENT,
  DEF_TOKEN_EVENT,
  DictionaryEntry,
  LangMode,
  langShortLabel,
  LANG_OPTIONS,
  LlmEvent,
  WbTranslation,
  WORKBENCH_INPUT_EVENT,
  WorkbenchInput,
} from "../services/api";
import DictionaryCard, { DictCardState } from "./DictionaryCard";
import { wordAtCaret, sentenceAtCaret } from "./caretText";
import "./Workbench.css";

const RETRANSLATE_DEBOUNCE_MS = 400;
const NARROW_BREAKPOINT = 520;

// Session 快取一筆：字典結果（命中）或單一 AI 字義（查無）。done = 可命中（AI 字義已結束）。
interface CacheEntry {
  dictEntry: DictionaryEntry | null;
  dictMiss: boolean;
  fallbackText: string;
  fallbackError: string | null;
  done: boolean; // 命中字典→true；查無→AI 字義結束時 true
}

export default function Workbench() {
  const [original, setOriginal] = useState("");
  const [translated, setTranslated] = useState("");
  const [targetLang, setTargetLang] = useState("zh-TW");
  const [provider, setProvider] = useState("");
  const [langMode, setLangMode] = useState<LangMode>("pairing");
  const [status, setStatus] = useState<string>("");
  const [narrow, setNarrow] = useState(false);
  const [card, setCard] = useState<DictCardState | null>(null);

  const rootRef = useRef<HTMLDivElement>(null);
  const originalRef = useRef<HTMLTextAreaElement>(null);
  const debounceTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const defSeq = useRef(0);
  // Session 快取：鍵 = 還原後原形 + 語言方向；in-memory，applyInput（關閉再開）時清空。
  const cache = useRef<Map<string, CacheEntry>>(new Map());
  const activeKey = useRef<string | null>(null);

  const applyInput = (input: WorkbenchInput) => {
    setOriginal(input.original);
    setTranslated(input.translated);
    setTargetLang(input.target_lang);
    setStatus("");
    setCard(null); // 清掉上一次殘留的單字卡
    defSeq.current = 0; // 作廢上一次的串流
    cache.current.clear(); // 關閉再開 → 快取清空、重新查
    activeKey.current = null;
    // 游標就緒：空白 ⌘CC 開的空 Workbench 要能直接打字。
    requestAnimationFrame(() => originalRef.current?.focus());
  };

  // 把 AI 字義完成的內容寫回 session 快取（def done/error 時呼叫）。
  const cacheComplete = (patch: Partial<CacheEntry>) => {
    const key = activeKey.current;
    const existing = cache.current.get(key ?? "");
    if (!key || !existing) return;
    cache.current.set(key, { ...existing, ...patch });
  };

  // 帶入 Glance 的原文 + 譯文：初次掛載讀一次 + 每次展開收事件更新。
  useEffect(() => {
    api.getWorkbenchInput().then((input) => {
      if (input) applyInput(input);
    });
    api.getSettings().then((s) => {
      setProvider(s.provider);
      setLangMode(s.lang_mode);
    });
    const unInput = listen<WorkbenchInput>(WORKBENCH_INPUT_EVENT, (e) => applyInput(e.payload));
    return () => {
      unInput.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 自適應斷點：寬度 < 520 → 上下堆疊。
  useEffect(() => {
    const el = rootRef.current;
    if (!el) return;
    const observer = new ResizeObserver(() => setNarrow(el.offsetWidth < NARROW_BREAKPOINT));
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  // Esc：先關字典卡，再關 Workbench。
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (card) setCard(null);
        else api.closeWorkbench();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [card]);

  // 點字典卡以外的地方關閉它（選新字時：mousedown 先關、mouseup 再開新卡）。
  useEffect(() => {
    if (!card) return;
    const onDown = (e: MouseEvent) => {
      if (!(e.target as HTMLElement).closest(".dict-card")) setCard(null);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [card]);

  // 字典查無時的 AI 字義串流（def-* 通道，依 defSeq 過濾）。
  useEffect(() => {
    const unToken = listen<LlmEvent>(DEF_TOKEN_EVENT, (e) => {
      if (e.payload.kind !== "token" || e.payload.seq !== defSeq.current) return;
      const delta = e.payload.delta;
      setCard((c) => (c ? { ...c, fallbackText: c.fallbackText + delta, fallbackStreaming: true } : c));
    });
    const unDone = listen<LlmEvent>(DEF_DONE_EVENT, (e) => {
      if (e.payload.kind !== "done" || e.payload.seq !== defSeq.current) return;
      setCard((c) => {
        if (!c) return c;
        cacheComplete({ fallbackText: c.fallbackText, fallbackError: null, done: true });
        return { ...c, fallbackStreaming: false };
      });
    });
    const unErr = listen<LlmEvent>(DEF_ERROR_EVENT, (e) => {
      if (e.payload.kind !== "error" || e.payload.seq !== defSeq.current) return;
      const message = e.payload.message;
      setCard((c) => {
        if (!c) return c;
        cacheComplete({ fallbackText: c.fallbackText, fallbackError: message, done: true });
        return { ...c, fallbackStreaming: false, fallbackError: message };
      });
    });
    return () => {
      unToken.then((f) => f());
      unDone.then((f) => f());
      unErr.then((f) => f());
    };
  }, []);

  const retranslate = useCallback(
    (text: string) => {
      api.workbenchTranslate(text).then((res: WbTranslation) => {
        switch (res.kind) {
          case "ok":
            setTranslated(res.translated);
            setTargetLang(res.target_lang); // 配對模式：反映解析後的方向
            setStatus(res.truncated ? "（已截斷）" : "");
            break;
          case "secret":
            setTranslated("");
            setStatus("已略過可能的機密內容");
            break;
          case "empty":
            setTranslated("");
            setStatus("");
            break;
          case "stale":
            // 已被更新的編輯取代：忽略，不回填（避免舊譯文蓋掉新的）。
            break;
          case "error":
            setStatus(res.message);
            break;
        }
      });
    },
    [],
  );

  const onOriginalChange = (value: string) => {
    setOriginal(value);
    setStatus("");
    clearTimeout(debounceTimer.current);
    debounceTimer.current = setTimeout(() => retranslate(value), RETRANSLATE_DEBOUNCE_MS);
  };

  const onTargetLangChange = (value: string) => {
    setTargetLang(value);
    api.getSettings().then((s) => api.setSettings({ ...s, target_lang: value }));
    if (original.trim()) retranslate(original);
  };

  // 點選原文一個字 → 詞形還原 → 查 session 快取 → 命中即回；未命中才查 ECDICT / Gemini。
  const onOriginalMouseUp = (e: React.MouseEvent<HTMLTextAreaElement>) => {
    const ta = originalRef.current;
    if (!ta) return;
    const word = wordAtCaret(ta.value, ta.selectionStart, ta.selectionEnd);
    if (!word) return;
    const anchor = { x: e.clientX, y: e.clientY }; // 先抓，避免 async 後事件被回收
    // 只送「該字所在句」而非整個原文框（H2：降低外送量、符合隱私；後端仍會再過機密過濾）。
    const sentence = sentenceAtCaret(ta.value, ta.selectionStart);

    api.dictionaryLookup(word).then(({ entry, lemma }) => {
      const key = `${lemma}|${targetLang}`; // 鍵 = 還原後原形 + 語言方向
      activeKey.current = key;
      const hit = cache.current.get(key);

      // 快取命中 → 秒回，不再打 Gemini。
      if (hit && hit.done) {
        setCard({
          word,
          anchor,
          dictEntry: hit.dictEntry,
          dictLoading: false,
          dictMiss: hit.dictMiss,
          fallbackText: hit.fallbackText,
          fallbackStreaming: false,
          fallbackError: hit.fallbackError,
        });
        return;
      }

      // 命中字典：純本地、不打 Gemini。
      if (entry) {
        cache.current.set(key, {
          dictEntry: entry,
          dictMiss: false,
          fallbackText: "",
          fallbackError: null,
          done: true,
        });
        setCard({
          word,
          anchor,
          dictEntry: entry,
          dictLoading: false,
          dictMiss: false,
          fallbackText: "",
          fallbackStreaming: false,
          fallbackError: null,
        });
        return;
      }

      // ECDICT 查無 → Gemini fallback。但 Gemini 是選配：先問後端有沒有設定 key
      // （llmKeySet 只回布林，key 始終留在 Keychain），沒設就不發那個注定失敗的請求，
      // 直接顯示友善提示，避免無聲卡在載入中。
      api.llmKeySet().then((hasKey) => {
        if (activeKey.current !== key) return; // 已被新點擊取代
        if (!hasKey) {
          const message =
            "此字典未收錄；到「設定 → 深度理解（Gemini）」填入 API key 可啟用 AI 補充字義";
          cache.current.set(key, {
            dictEntry: null,
            dictMiss: true,
            fallbackText: "",
            fallbackError: message,
            done: true,
          });
          setCard({
            word,
            anchor,
            dictEntry: null,
            dictLoading: false,
            dictMiss: true,
            fallbackText: "",
            fallbackStreaming: false,
            fallbackError: message,
          });
          return;
        }
        // 有設 key → 照原本流程發單一 AI 字義請求。
        cache.current.set(key, {
          dictEntry: null,
          dictMiss: true,
          fallbackText: "",
          fallbackError: null,
          done: false, // 查無待 AI 字義結束
        });
        setCard({
          word,
          anchor,
          dictEntry: null,
          dictLoading: false,
          dictMiss: true,
          fallbackText: "",
          fallbackStreaming: true,
          fallbackError: null,
        });
        api.geminiDefine(word, sentence, targetLang).then((seq) => {
          defSeq.current = seq;
        });
      });
    });
  };

  const copyTranslation = () => {
    navigator.clipboard.writeText(translated).then(
      () => setStatus("已複製譯文"),
      () => setStatus("複製失敗"),
    );
  };

  return (
    <div ref={rootRef} className="wb-root">
      <header className="wb-toolbar">
        <div className="wb-brand">
          <span className="seal" aria-hidden />
          <span className="wb-wordmark">Sumi</span>
        </div>
        <div className="wb-tools">
          {langMode === "pairing" ? (
            // 配對模式：方向由路由自動決定，顯示解析後的目標（唯讀）。
            <span className="wb-target" title="語言配對模式：方向自動決定">
              → {langShortLabel(targetLang)}
            </span>
          ) : (
            <select
              value={targetLang}
              onChange={(e) => onTargetLangChange(e.target.value)}
              aria-label="目標語言"
            >
              {LANG_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {langShortLabel(o.value)}
                </option>
              ))}
            </select>
          )}
          {provider && <span className="wb-provider">{provider === "google" ? "Google" : "DeepL"}</span>}
          <button className="wb-copy" onClick={copyTranslation}>
            複製譯文
          </button>
        </div>
      </header>

      <main className={`wb-main ${narrow ? "stacked" : "columns"}`}>
        <section className="wb-pane">
          <label className="wb-pane-label">原文 · 可編輯，點字查詢</label>
          <textarea
            ref={originalRef}
            className="wb-original"
            value={original}
            spellCheck={false}
            onChange={(e) => onOriginalChange(e.target.value)}
            onMouseUp={onOriginalMouseUp}
          />
        </section>
        <section className="wb-pane">
          <label className="wb-pane-label">譯文</label>
          <div className="wb-translated" lang={targetLang}>
            {translated}
          </div>
        </section>
      </main>

      <footer className="wb-status">{status}</footer>

      {card && <DictionaryCard card={card} onClose={() => setCard(null)} />}
    </div>
  );
}

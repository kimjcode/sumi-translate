import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  api,
  DictionaryEntry,
  langShortLabel,
  LANG_OPTIONS,
  LLM_DONE_EVENT,
  LLM_ERROR_EVENT,
  LLM_TOKEN_EVENT,
  LlmEvent,
  WbTranslation,
} from "../services/api";
import DictionaryCard, { DictCardState } from "./DictionaryCard";
import "./Workbench.css";

const RETRANSLATE_DEBOUNCE_MS = 400;
const NARROW_BREAKPOINT = 520;

export default function Workbench() {
  const [original, setOriginal] = useState("");
  const [translated, setTranslated] = useState("");
  const [targetLang, setTargetLang] = useState("zh-TW");
  const [provider, setProvider] = useState("");
  const [status, setStatus] = useState<string>("");
  const [narrow, setNarrow] = useState(false);
  const [card, setCard] = useState<DictCardState | null>(null);

  const rootRef = useRef<HTMLDivElement>(null);
  const originalRef = useRef<HTMLTextAreaElement>(null);
  const debounceTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const llmSeq = useRef(0);

  // 帶入 Glance 的原文 + 譯文。
  useEffect(() => {
    api.getWorkbenchInput().then((input) => {
      if (input) {
        setOriginal(input.original);
        setTranslated(input.translated);
        setTargetLang(input.target_lang);
      }
    });
    api.getSettings().then((s) => setProvider(s.provider));
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

  // LLM 串流事件：依 seq 過濾，逐 token 累積到字典卡的文法段。
  useEffect(() => {
    const unToken = listen<LlmEvent>(LLM_TOKEN_EVENT, (e) => {
      if (e.payload.kind !== "token" || e.payload.seq !== llmSeq.current) return;
      const delta = e.payload.delta;
      setCard((c) => (c ? { ...c, grammar: c.grammar + delta, grammarStreaming: true } : c));
    });
    const unDone = listen<LlmEvent>(LLM_DONE_EVENT, (e) => {
      if (e.payload.kind !== "done" || e.payload.seq !== llmSeq.current) return;
      setCard((c) => (c ? { ...c, grammarStreaming: false } : c));
    });
    const unErr = listen<LlmEvent>(LLM_ERROR_EVENT, (e) => {
      if (e.payload.kind !== "error" || e.payload.seq !== llmSeq.current) return;
      const message = e.payload.message;
      setCard((c) => (c ? { ...c, grammarStreaming: false, grammarError: message } : c));
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

  // 點選原文一個字 → 字典卡 + Gemini 文法。
  const onOriginalMouseUp = (e: React.MouseEvent<HTMLTextAreaElement>) => {
    const ta = originalRef.current;
    if (!ta) return;
    const word = wordAtCaret(ta.value, ta.selectionStart, ta.selectionEnd);
    if (!word) return;

    setCard({
      word,
      anchor: { x: e.clientX, y: e.clientY },
      dictEntry: null,
      dictLoading: true,
      grammar: "",
      grammarStreaming: true,
      grammarError: null,
    });

    api.dictionaryLookup(word).then((entry: DictionaryEntry | null) => {
      setCard((c) => (c && c.word === word ? { ...c, dictEntry: entry, dictLoading: false } : c));
    });
    // 串流事件用後端回傳的真實 seq 過濾（IPC 比首 token 快，更新先於 token 抵達）。
    api.geminiExplain(word, ta.value, targetLang).then((seq) => {
      llmSeq.current = seq;
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

      {card && (
        <DictionaryCard
          card={card}
          targetLang={targetLang}
          onClose={() => setCard(null)}
        />
      )}
    </div>
  );
}

/// 從游標位置抓出英文單字（向左右掃描到非字母為止）。有選取就優先用選取。
function wordAtCaret(text: string, start: number, end: number): string | null {
  const isWordChar = (c: string) => /[A-Za-z'-]/.test(c);

  if (end > start) {
    const sel = text.slice(start, end).trim();
    return /^[A-Za-z][A-Za-z'-]*$/.test(sel) ? sel : null;
  }
  let l = start;
  let r = start;
  while (l > 0 && isWordChar(text[l - 1])) l--;
  while (r < text.length && isWordChar(text[r])) r++;
  const word = text.slice(l, r).replace(/^[-']+|[-']+$/g, "");
  return /^[A-Za-z][A-Za-z'-]*$/.test(word) ? word : null;
}

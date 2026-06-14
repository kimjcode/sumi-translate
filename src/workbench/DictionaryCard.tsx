import { useLayoutEffect, useRef, useState } from "react";
import { DictionaryEntry } from "../services/api";
import "./DictionaryCard.css";

export interface DictCardState {
  word: string;
  anchor: { x: number; y: number };
  dictEntry: DictionaryEntry | null;
  dictLoading: boolean;
  grammar: string;
  grammarStreaming: boolean;
  grammarError: string | null;
}

const CARD_WIDTH = 320;
const GAP = 12;

export default function DictionaryCard({
  card,
  onClose,
}: {
  card: DictCardState;
  targetLang: string;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: card.anchor.x, top: card.anchor.y + GAP });

  // 夾在視窗內，避免溢出邊緣。
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const h = el.offsetHeight;
    let left = card.anchor.x;
    let top = card.anchor.y + GAP;
    if (left + CARD_WIDTH > window.innerWidth - 8) left = window.innerWidth - CARD_WIDTH - 8;
    if (left < 8) left = 8;
    if (top + h > window.innerHeight - 8) top = card.anchor.y - h - GAP;
    if (top < 8) top = 8;
    setPos({ left, top });
  }, [card.anchor, card.dictEntry, card.grammar]);

  return (
    <div
      ref={ref}
      className="dict-card"
      style={{ left: pos.left, top: pos.top, width: CARD_WIDTH }}
      onClick={(e) => e.stopPropagation()}
    >
      {/* 第一段：真字典（事實性，非 LLM） */}
      <section className="dict-section">
        <div className="dict-head">
          <span className="dict-word">{card.dictEntry?.word ?? card.word}</span>
          {card.dictEntry?.phonetic && (
            <span className="dict-phonetic">{card.dictEntry.phonetic}</span>
          )}
        </div>
        {card.dictLoading ? (
          <p className="dict-muted">查詢字典中…</p>
        ) : card.dictEntry && card.dictEntry.meanings.length > 0 ? (
          <ul className="dict-meanings">
            {card.dictEntry.meanings.map((m, i) => (
              <li key={i}>
                <span className="dict-pos">{m.part_of_speech}</span>
                <ol className="dict-defs">
                  {m.definitions.map((d, j) => (
                    <li key={j}>{d}</li>
                  ))}
                </ol>
              </li>
            ))}
          </ul>
        ) : (
          <p className="dict-muted">字典查無此字</p>
        )}
      </section>

      <div className="dict-divider" />

      {/* 第二段：Gemini 文法 / 語境（LLM，標明來源） */}
      <section className="dict-section">
        <div className="llm-label">
          文法 / 語境 · <span className="llm-provider">Gemini</span>
        </div>
        {card.grammarError ? (
          <p className="dict-muted">{card.grammarError}</p>
        ) : (
          <p className="llm-text">
            {card.grammar}
            {card.grammarStreaming && <span className="brush-cursor" aria-label="生成中" />}
          </p>
        )}
      </section>

      <button className="dict-close" onClick={onClose} aria-label="關閉">
        ✕
      </button>
    </div>
  );
}

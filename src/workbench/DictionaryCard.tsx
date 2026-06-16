import { useLayoutEffect, useRef, useState } from "react";
import { DictionaryEntry } from "../services/api";
import "./DictionaryCard.css";

export interface DictCardState {
  word: string;
  anchor: { x: number; y: number };
  // ECDICT 真字典命中
  dictEntry: DictionaryEntry | null;
  dictLoading: boolean;
  // ECDICT 查無 → 單一 AI 字義（明確標示 AI，非字典）
  dictMiss: boolean;
  fallbackText: string;
  fallbackStreaming: boolean;
  fallbackError: string | null;
}

const CARD_WIDTH = 320;
const GAP = 12;

export default function DictionaryCard({
  card,
  onClose,
}: {
  card: DictCardState;
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
  }, [card.anchor, card.dictEntry, card.fallbackText]);

  return (
    <div
      ref={ref}
      className="dict-card"
      style={{ left: pos.left, top: pos.top, width: CARD_WIDTH }}
      onClick={(e) => e.stopPropagation()}
    >
      {/* 字典卡只剩字典：ECDICT 命中=真字典；查無=單一 AI 字義（明確標示 AI）。 */}
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
        ) : card.dictMiss ? (
          <div className="dict-fallback">
            <div className="llm-label">
              AI 字義 · <span className="llm-provider">Gemini</span>
              <span className="ai-note">（字典未收錄，AI 推測）</span>
            </div>
            {card.fallbackError ? (
              <p className="dict-muted">{card.fallbackError}</p>
            ) : (
              <p className="llm-text">
                {card.fallbackText}
                {card.fallbackStreaming && <span className="brush-cursor" aria-label="生成中" />}
              </p>
            )}
          </div>
        ) : (
          <p className="dict-muted">英漢字典未收錄</p>
        )}
      </section>

      <button className="dict-close" onClick={onClose} aria-label="關閉">
        ✕
      </button>
    </div>
  );
}

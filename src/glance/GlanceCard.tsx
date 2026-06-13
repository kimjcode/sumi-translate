import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import {
  api,
  GlanceState,
  GLANCE_STATE_EVENT,
  GLANCE_WILL_HIDE_EVENT,
  langShortLabel,
} from "../services/api";
import "./GlanceCard.css";

const CARD_WIDTH = 340;

export default function GlanceCard() {
  const [state, setState] = useState<GlanceState | null>(null);
  const [hiding, setHiding] = useState(false);
  const [expandHint, setExpandHint] = useState(false);
  const [stateSeq, setStateSeq] = useState(0);
  const cardRef = useRef<HTMLDivElement>(null);
  const lastActivity = useRef(0);

  useEffect(() => {
    const unlistenState = listen<GlanceState>(GLANCE_STATE_EVENT, (event) => {
      setState(event.payload);
      setHiding(false);
      setExpandHint(false);
      setStateSeq((n) => n + 1);
    });
    const unlistenHide = listen(GLANCE_WILL_HIDE_EVENT, () => setHiding(true));
    return () => {
      unlistenState.then((fn) => fn());
      unlistenHide.then((fn) => fn());
    };
  }, []);

  // 高度依內容：量測卡片實際高度，回寫視窗尺寸。
  // 觀察的是穩定的外層 .glance-card（不隨 stateSeq 重新掛載），內層內容才換。
  useEffect(() => {
    const el = cardRef.current;
    if (!el) return;
    const observer = new ResizeObserver(() => {
      const height = Math.min(Math.max(Math.ceil(el.offsetHeight), 48), 480);
      getCurrentWindow().setSize(new LogicalSize(CARD_WIDTH, height));
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  // 滑鼠在浮窗上活動 → 重置閒置計時（節流 1s）。
  const onMouseMove = () => {
    const now = Date.now();
    if (now - lastActivity.current > 1000) {
      lastActivity.current = now;
      api.glanceActivity();
    }
  };

  return (
    <div ref={cardRef} className="glance-card" onMouseMove={onMouseMove}>
      {state && (
        <div key={stateSeq} className={`glance-content ${hiding ? "hiding" : "showing"}`}>
          <GlanceContent state={state} expandHint={expandHint} onExpand={() => setExpandHint(true)} />
        </div>
      )}
    </div>
  );
}

function GlanceContent({
  state,
  expandHint,
  onExpand,
}: {
  state: GlanceState;
  expandHint: boolean;
  onExpand: () => void;
}) {
  if (state.kind === "secret") {
    return (
      <div className="glance-secret">
        <p className="secret-title">已略過可能的機密內容</p>
        <p className="secret-hint">內容看起來像密碼或金鑰，Sumi 不會送出。</p>
      </div>
    );
  }
  if (state.kind === "error") {
    return (
      <div className="glance-error">
        <p>{state.message}</p>
      </div>
    );
  }

  const langLabel =
    state.kind === "result" && state.detected_source
      ? `${state.detected_source.toUpperCase()} → ${langShortLabel(state.target_lang)}`
      : `→ ${langShortLabel(state.target_lang)}`;

  return (
    <>
      <header className="glance-header">
        <span className="lang-label">{langLabel}</span>
        <button className="expand-button" onClick={onExpand} title="Workbench 模式">
          {expandHint ? "即將推出" : "⌘↩ 展開"}
        </button>
      </header>
      <p className="glance-original">
        {state.original}
        {state.truncated && <span className="truncated-mark">（已截斷）</span>}
      </p>
      <div className="hairline" />
      {state.kind === "loading" ? (
        <div className="glance-loading">
          <span className="brush-cursor" aria-label="翻譯中" />
        </div>
      ) : (
        <p className="glance-translated" lang={state.target_lang}>
          {state.translated}
        </p>
      )}
      <footer className="glance-footer">esc 關閉 · ⌘↩ 展開</footer>
    </>
  );
}

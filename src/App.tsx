import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./App.css";

const CAPTURED_EVENT = "sumi://captured";

interface CapturedPayload {
  text: string;
  char_count: number;
}

function App() {
  const [trusted, setTrusted] = useState<boolean | null>(null);
  const [captured, setCaptured] = useState<CapturedPayload | null>(null);

  // 輪詢 Accessibility 權限狀態，授權完成後自動切換為待命畫面（毋需重啟）。
  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const check = async () => {
      const ok = await invoke<boolean>("accessibility_status");
      if (cancelled) return;
      setTrusted(ok);
      if (!ok) timer = setTimeout(check, 1000);
    };
    check();

    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, []);

  useEffect(() => {
    const unlisten = listen<CapturedPayload>(CAPTURED_EVENT, (event) => {
      setCaptured(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Esc 隱藏視窗
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") getCurrentWindow().hide();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  if (trusted === null) {
    return <main className="container">檢查權限中…</main>;
  }

  if (!trusted) {
    return (
      <main className="container">
        <h1>Sumi 需要「輔助使用」權限</h1>
        <p>
          Sumi 透過全域鍵盤監聽偵測「雙擊 ⌘C」這個觸發手勢。macOS
          要求這類監聽必須由你在系統設定中明確授權。
        </p>
        <p>
          Sumi 只偵測觸發鍵，不會記錄你輸入的內容；剪貼簿文字也只在你雙擊 ⌘C
          時才讀取。
        </p>
        <button onClick={() => invoke("open_accessibility_settings")}>
          打開「系統設定 → 輔助使用」
        </button>
        <p className="hint">
          開發模式下請把權限授予啟動 <code>npm run tauri dev</code> 的程式
          （如「終端機」）。授權後此畫面會自動更新，不需重啟。
        </p>
      </main>
    );
  }

  return (
    <main className="container">
      {captured ? (
        <>
          <p className="status">
            已擷取剪貼簿文字（{captured.char_count} 字元）：
          </p>
          <pre className="captured">{captured.text}</pre>
        </>
      ) : (
        <p className="status">
          已就緒 — 在任何 App 反白文字後雙擊 <kbd>⌘C</kbd>
        </p>
      )}
      <p className="hint">Esc 隱藏視窗</p>
    </main>
  );
}

export default App;

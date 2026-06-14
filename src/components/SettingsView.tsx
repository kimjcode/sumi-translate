import { useCallback, useEffect, useState } from "react";
import { api, LANG_OPTIONS, Provider, Settings } from "../services/api";
import "./SettingsView.css";

export default function SettingsView() {
  const [trusted, setTrusted] = useState<boolean | null>(null);

  // 輪詢權限狀態：授權後自動切到設定畫面，不需重啟。
  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const check = async () => {
      const ok = await api.accessibilityStatus();
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

  return (
    <main className="settings-root">
      <header className="app-header">
        <span className="seal" aria-hidden />
        <span className="wordmark">Sumi</span>
      </header>
      {trusted === null ? null : trusted ? <SettingsForm /> : <Onboarding />}
    </main>
  );
}

function Onboarding() {
  const [requested, setRequested] = useState(false);

  return (
    <section className="onboarding">
      <h1>讓 Sumi 看得到你的觸發鍵</h1>
      <p>
        Sumi 用全域鍵盤監聽偵測「雙擊 ⌘C」這一個手勢。macOS
        要求這類監聽必須由你在系統設定中明確授權（輔助使用）。
      </p>
      <p>
        Sumi 只比對觸發鍵的鍵碼，不記錄你輸入的內容；剪貼簿文字也只在你雙擊 ⌘C
        時才讀取。
      </p>
      {!requested ? (
        <button
          className="primary"
          onClick={() => {
            api.requestAccessibility();
            setRequested(true);
          }}
        >
          啟用權限
        </button>
      ) : (
        <>
          <p className="onboarding-followup">
            在系統跳出的視窗中允許 Sumi，或手動到設定裡開啟。授權完成後這個畫面會自動更新；若
            10 秒內沒反應，重新啟動 Sumi 一次。
          </p>
          <button className="secondary" onClick={() => api.openAccessibilitySettings()}>
            打開「系統設定 → 輔助使用」
          </button>
        </>
      )}
    </section>
  );
}

function SettingsForm() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [keyStatus, setKeyStatus] = useState<Record<Provider, boolean>>({
    google: false,
    deepl: false,
  });
  const [keyDraft, setKeyDraft] = useState("");
  const [keyMessage, setKeyMessage] = useState("");
  const [pendingDeepl, setPendingDeepl] = useState(false);
  const [geminiSet, setGeminiSet] = useState(false);
  const [geminiDraft, setGeminiDraft] = useState("");
  const [geminiMessage, setGeminiMessage] = useState("");

  const refreshKeyStatus = useCallback(async () => {
    const [google, deepl, gemini] = await Promise.all([
      api.apiKeySet("google"),
      api.apiKeySet("deepl"),
      api.llmKeySet(),
    ]);
    setKeyStatus({ google, deepl });
    setGeminiSet(gemini);
  }, []);

  useEffect(() => {
    api.getSettings().then(setSettings);
    refreshKeyStatus();
  }, [refreshKeyStatus]);

  if (!settings) return null;

  const apply = async (patch: Partial<Settings>) => {
    const next = { ...settings, ...patch };
    setSettings(next);
    try {
      await api.setSettings(next);
    } catch (e) {
      setKeyMessage(String(e));
    }
  };

  const activeProvider = settings.provider;

  const saveKey = async () => {
    try {
      await api.setApiKey(activeProvider, keyDraft);
      setKeyDraft("");
      setKeyMessage("已存入 macOS Keychain");
      refreshKeyStatus();
    } catch (e) {
      setKeyMessage(String(e));
    }
  };

  const clearKey = async () => {
    await api.clearApiKey(activeProvider);
    setKeyMessage("已從 Keychain 移除");
    refreshKeyStatus();
  };

  const saveGeminiKey = async () => {
    try {
      await api.setLlmKey(geminiDraft);
      setGeminiDraft("");
      setGeminiMessage("已存入 macOS Keychain");
      refreshKeyStatus();
    } catch (e) {
      setGeminiMessage(String(e));
    }
  };

  const clearGeminiKey = async () => {
    await api.clearLlmKey();
    setGeminiMessage("已從 Keychain 移除");
    refreshKeyStatus();
  };

  return (
    <div className="settings-form">
      <section>
        <h2>翻譯引擎</h2>
        <div className="radio-row">
          <label>
            <input
              type="radio"
              name="provider"
              checked={activeProvider === "google" && !pendingDeepl}
              onChange={() => {
                setPendingDeepl(false);
                apply({ provider: "google" });
              }}
            />
            Google（預設）
          </label>
          <label>
            <input
              type="radio"
              name="provider"
              checked={activeProvider === "deepl" || pendingDeepl}
              onChange={() => {
                if (activeProvider !== "deepl") setPendingDeepl(true);
              }}
            />
            DeepL
          </label>
        </div>
        {pendingDeepl && (
          <div className="notice" role="alert">
            <p>
              DeepL <strong>免費層</strong>可能會用你送出的文字改善服務；付費 Pro
              方案預設不用於訓練。Google 付費 API 不會用內容訓練，因此為預設。
            </p>
            <div className="notice-actions">
              <button
                className="primary"
                onClick={() => {
                  setPendingDeepl(false);
                  apply({ provider: "deepl" });
                }}
              >
                了解，切換到 DeepL
              </button>
              <button className="secondary" onClick={() => setPendingDeepl(false)}>
                留在 Google
              </button>
            </div>
          </div>
        )}

        <div className="field">
          <label htmlFor="api-key">
            {activeProvider === "google" ? "Google" : "DeepL"} API key
            <span className={`key-status ${keyStatus[activeProvider] ? "ok" : ""}`}>
              {keyStatus[activeProvider] ? "已設定" : "未設定"}
            </span>
          </label>
          <div className="key-row">
            <input
              id="api-key"
              type="password"
              value={keyDraft}
              placeholder="貼上 API key（只存入 macOS Keychain）"
              onChange={(e) => setKeyDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && keyDraft.trim()) saveKey();
              }}
            />
            <button className="primary" disabled={!keyDraft.trim()} onClick={saveKey}>
              儲存
            </button>
            {keyStatus[activeProvider] && (
              <button className="secondary" onClick={clearKey}>
                清除
              </button>
            )}
          </div>
          {keyMessage && <p className="field-message">{keyMessage}</p>}
        </div>
      </section>

      <section>
        <h2>深度理解（Gemini）</h2>
        <div className="field">
          <label htmlFor="gemini-key">
            Gemini API key
            <span className={`key-status ${geminiSet ? "ok" : ""}`}>
              {geminiSet ? "已設定" : "未設定"}
            </span>
          </label>
          <div className="key-row">
            <input
              id="gemini-key"
              type="password"
              value={geminiDraft}
              placeholder="Workbench 文法 / 語境用，只存入 macOS Keychain"
              onChange={(e) => setGeminiDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && geminiDraft.trim()) saveGeminiKey();
              }}
            />
            <button className="primary" disabled={!geminiDraft.trim()} onClick={saveGeminiKey}>
              儲存
            </button>
            {geminiSet && (
              <button className="secondary" onClick={clearGeminiKey}>
                清除
              </button>
            )}
          </div>
          <p className="field-hint">
            字典查詢免 key（公開字典 API）；只有文法 / 語境 / 改寫才用 Gemini。
          </p>
          {geminiMessage && <p className="field-message">{geminiMessage}</p>}
        </div>
      </section>

      <section>
        <h2>翻譯行為</h2>
        <div className="field">
          <label htmlFor="target-lang">目標語言</label>
          <select
            id="target-lang"
            value={settings.target_lang}
            onChange={(e) => apply({ target_lang: e.target.value })}
          >
            {LANG_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
          <p className="field-hint">來源語言由翻譯引擎自動偵測。</p>
        </div>
        <div className="field">
          <label htmlFor="double-press">雙擊 ⌘C 時間窗（毫秒）</label>
          <input
            id="double-press"
            type="number"
            min={100}
            max={1500}
            step={50}
            value={settings.double_press_ms}
            onChange={(e) => {
              const v = Number(e.target.value);
              if (v >= 100 && v <= 1500) apply({ double_press_ms: v });
            }}
          />
        </div>
        <div className="field">
          <label htmlFor="idle-close">浮窗閒置自動關閉（秒）</label>
          <input
            id="idle-close"
            type="number"
            min={2}
            max={60}
            step={1}
            value={Math.round(settings.idle_close_ms / 1000)}
            onChange={(e) => {
              const v = Number(e.target.value) * 1000;
              if (v >= 2000 && v <= 60000) apply({ idle_close_ms: v });
            }}
          />
        </div>
      </section>

      <section>
        <h2>進階</h2>
        <div className="field toggle-row">
          <label htmlFor="always-on">always-on 剪貼簿監聽（複製即翻）</label>
          <input id="always-on" type="checkbox" checked={false} disabled />
          <span className="field-hint">即將推出，預設關閉。</span>
        </div>
      </section>

      <footer className="settings-footer">
        在任何 App 反白文字後快速按兩次 ⌘C，譯文會出現在游標附近。
      </footer>
    </div>
  );
}

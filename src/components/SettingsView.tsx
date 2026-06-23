import { useCallback, useEffect, useState } from "react";
import { api, LANG_OPTIONS, Provider, Settings } from "../services/api";
import "./SettingsView.css";

export default function SettingsView() {
  const [trusted, setTrusted] = useState<boolean | null>(null);
  const [checking, setChecking] = useState(false);

  // 立即重查一次（手動按鈕 / 回到視窗時用）。回傳目前狀態。
  const recheck = useCallback(async () => {
    setChecking(true);
    const ok = await api.accessibilityStatus();
    setTrusted(ok);
    setChecking(false);
    return ok;
  }, []);

  // 輪詢權限狀態：授權後自動切到設定畫面，不需重啟。回到視窗時也立即重查
  // （從系統設定切回 Sumi 即偵測，毋需等輪詢）。
  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const poll = async () => {
      const ok = await api.accessibilityStatus();
      if (cancelled) return;
      setTrusted(ok);
      if (!ok) timer = setTimeout(poll, 1000);
    };
    poll();
    const onFocus = () => {
      if (!cancelled) recheck();
    };
    window.addEventListener("focus", onFocus);
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
      window.removeEventListener("focus", onFocus);
    };
  }, [recheck]);

  return (
    <main className="settings-root">
      <header className="app-header">
        <span className="seal" aria-hidden />
        <span className="wordmark">Sumi</span>
      </header>
      {trusted === null ? null : trusted ? (
        <SettingsForm />
      ) : (
        <Onboarding recheck={recheck} checking={checking} />
      )}
    </main>
  );
}

function Onboarding({
  recheck,
  checking,
}: {
  recheck: () => Promise<boolean>;
  checking: boolean;
}) {
  const [requested, setRequested] = useState(false);
  const [stillBlocked, setStillBlocked] = useState(false);

  const onRecheck = async () => {
    const ok = await recheck();
    if (!ok) setStillBlocked(true); // 還是沒過 → 攤開疑難排解
  };

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
            在系統跳出的視窗中允許 Sumi，或到設定裡開啟。回到 Sumi 會自動偵測；沒反應就按「重新檢查」。
          </p>
          <div className="onboarding-actions">
            <button className="secondary" onClick={() => api.openAccessibilitySettings()}>
              打開「系統設定 → 輔助使用」
            </button>
            <button className="secondary" onClick={onRecheck} disabled={checking}>
              {checking ? "檢查中…" : "重新檢查"}
            </button>
          </div>
        </>
      )}

      <details className="onboarding-trouble" open={stillBlocked}>
        <summary>已經授權了，卻還停在這頁？</summary>
        <p>
          多半是因為這版 Sumi 還沒正式簽章，每次重新打包身分會變，系統設定裡那條「Sumi」可能綁到舊版本。
          <b>切換開關沒用</b>，要整條移除再重授：
        </p>
        <ol>
          <li>系統設定 → 隱私權與安全性 → 輔助使用</li>
          <li>
            選到「Sumi」→ 按 <b>減號（−）整條移除</b>（不是只關開關）
          </li>
          <li>完全結束 Sumi（⌘Q）再重新打開 → 按「啟用權限」重新授權</li>
          <li>回到這頁按「重新檢查」即可</li>
        </ol>
      </details>
    </section>
  );
}

// MT 引擎清單：兩把 key 各自獨立管理，與「啟用哪個引擎」無關。
const MT_PROVIDERS: { id: Provider; label: string }[] = [
  { id: "google", label: "Google" },
  { id: "deepl", label: "DeepL" },
];

function SettingsForm() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [keyStatus, setKeyStatus] = useState<Record<Provider, boolean>>({
    google: false,
    deepl: false,
  });
  // 兩把 MT key 各自一份 draft / 訊息（彼此獨立，不論啟用哪個引擎）。
  const [keyDrafts, setKeyDrafts] = useState<Record<Provider, string>>({
    google: "",
    deepl: "",
  });
  const [keyMessages, setKeyMessages] = useState<Record<Provider, string>>({
    google: "",
    deepl: "",
  });
  const [formMessage, setFormMessage] = useState("");
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
      setFormMessage(String(e));
    }
  };

  const activeProvider = settings.provider;

  // 任一引擎的 key 都能獨立存（不需先切到該引擎）。
  const saveKey = async (provider: Provider) => {
    try {
      await api.setApiKey(provider, keyDrafts[provider]);
      setKeyDrafts((d) => ({ ...d, [provider]: "" }));
      setKeyMessages((m) => ({ ...m, [provider]: "已存入 macOS Keychain" }));
      refreshKeyStatus();
    } catch (e) {
      setKeyMessages((m) => ({ ...m, [provider]: String(e) }));
    }
  };

  // 任一引擎的 key 都能獨立清（刪 DeepL 不需先切到 DeepL）。
  const clearKey = async (provider: Provider) => {
    try {
      await api.clearApiKey(provider);
      setKeyMessages((m) => ({ ...m, [provider]: "已從 Keychain 移除" }));
      refreshKeyStatus();
    } catch (e) {
      setKeyMessages((m) => ({ ...m, [provider]: String(e) }));
    }
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

        {/* 兩把 key 各自獨立：不論啟用哪個引擎，都能設定 / 清除任一把。 */}
        {MT_PROVIDERS.map(({ id, label }) => (
          <div className="field" key={id}>
            <label htmlFor={`api-key-${id}`}>
              {label} API key
              {id === activeProvider && <span className="key-active">使用中</span>}
              <span className={`key-status ${keyStatus[id] ? "ok" : ""}`}>
                {keyStatus[id] ? "已設定" : "未設定"}
              </span>
            </label>
            <div className="key-row">
              <input
                id={`api-key-${id}`}
                type="password"
                value={keyDrafts[id]}
                placeholder="貼上 API key（只存入 macOS Keychain）"
                onChange={(e) =>
                  setKeyDrafts((d) => ({ ...d, [id]: e.target.value }))
                }
                onKeyDown={(e) => {
                  if (e.key === "Enter" && keyDrafts[id].trim()) saveKey(id);
                }}
              />
              <button
                className="primary"
                disabled={!keyDrafts[id].trim()}
                onClick={() => saveKey(id)}
              >
                儲存
              </button>
              {keyStatus[id] && (
                <button className="secondary" onClick={() => clearKey(id)}>
                  清除
                </button>
              )}
            </div>
            {keyMessages[id] && <p className="field-message">{keyMessages[id]}</p>}
          </div>
        ))}
        {formMessage && <p className="field-message">{formMessage}</p>}
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
            字典查詢免 key（本地 ECDICT，離線、零外送）；只有字典查無時的 AI 字義才用 Gemini。
          </p>
          {geminiMessage && <p className="field-message">{geminiMessage}</p>}
        </div>
      </section>

      <section>
        <h2>語言</h2>
        <div className="radio-row">
          <label>
            <input
              type="radio"
              name="lang-mode"
              checked={settings.lang_mode === "pairing"}
              onChange={() => apply({ lang_mode: "pairing" })}
            />
            語言配對（雙向）
          </label>
          <label>
            <input
              type="radio"
              name="lang-mode"
              checked={settings.lang_mode === "fixed"}
              onChange={() => apply({ lang_mode: "fixed" })}
            />
            固定目標語言
          </label>
        </div>

        {settings.lang_mode === "pairing" ? (
          <>
            <div className="pairing-row">
              <div className="field">
                <label htmlFor="my-lang">我的語言</label>
                <select
                  id="my-lang"
                  value={settings.my_lang}
                  onChange={(e) => apply({ my_lang: e.target.value })}
                >
                  {LANG_OPTIONS.map((o) => (
                    <option key={o.value} value={o.value}>
                      {o.label}
                    </option>
                  ))}
                </select>
              </div>
              <span className="pairing-swap" aria-hidden>
                ⇄
              </span>
              <div className="field">
                <label htmlFor="counterpart-lang">對照語言</label>
                <select
                  id="counterpart-lang"
                  value={settings.counterpart_lang}
                  onChange={(e) => apply({ counterpart_lang: e.target.value })}
                >
                  {LANG_OPTIONS.map((o) => (
                    <option key={o.value} value={o.value}>
                      {o.label}
                    </option>
                  ))}
                </select>
              </div>
            </div>
            <p className="field-hint">
              對照語言來源 → 翻成我的語言；我的語言來源 → 翻成對照語言；其他外語 →
              翻成我的語言。來源由翻譯引擎自動偵測，不需反轉鈕。
            </p>
          </>
        ) : (
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
            <p className="field-hint">永遠翻成這個語言；來源由翻譯引擎自動偵測。</p>
          </div>
        )}
      </section>

      <section>
        <h2>觸發與浮窗</h2>
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

      <footer className="settings-footer">
        在任何 App 反白文字後快速按兩次 ⌘C，譯文會出現在游標附近。
      </footer>
    </div>
  );
}

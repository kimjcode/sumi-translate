#!/usr/bin/env python3
"""產生 Workbench 字典用的 ECDICT 英漢 SQLite（已簡轉繁 / 台灣用詞）。

流程：下載 pin 住的 ECDICT 1.0.28 SQLite → 用 OpenCC s2twp 把中文釋義轉成
繁中（台灣用詞）→ 產出精簡的 src-tauri/resources/ecdict.sqlite（只留查詢需要的欄位、
建索引）。產物不進 git（見 .gitignore），build 前需先跑：`npm run build:dict`。

需求（建置工具，非 app 依賴）：python3 + `pip3 install opencc`。
"""
import os
import re
import sqlite3
import sys
import zipfile
import urllib.request

# 只收「單一英文單字」（可含 ' 與 -，無空白）——Workbench 是點單字查詢，
# 多字片語/慣用語永遠不會被點到，濾掉可大幅縮小體積。對齊前端 wordAtCaret 的取詞規則。
SINGLE_WORD = re.compile(r"^[A-Za-z][A-Za-z'\-]*$")

# ECDICT exchange 欄的詞形變化 tag：p=過去式 d=過去分詞 i=現在分詞 3=三單 s=複數 r=比較級 t=最高級。
# 用這些反建「變化型 → 原形」對照，讓 wakes/waking/woke 都能還原成 wake。
INFLECT_TAGS = {"p", "d", "i", "3", "s", "r", "t"}


def parse_inflections(exchange):
    forms = []
    for seg in (exchange or "").split("/"):
        if ":" not in seg:
            continue
        tag, val = seg.split(":", 1)
        if tag in INFLECT_TAGS:
            for v in val.split(","):
                v = v.strip().lower()
                if v:
                    forms.append(v)
    return forms

# ── pin 住的來源（換版只改這裡，並更新 docs/decisions.md）────────────────────
ECDICT_VERSION = "1.0.28"
ECDICT_URL = (
    "https://github.com/skywind3000/ECDICT/releases/download/1.0.28/ecdict-sqlite-28.zip"
)
ECDICT_LICENSE = "MIT (Copyright 2025 Linwei, skywind3000/ECDICT)"

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CACHE = os.path.join(ROOT, "scripts", ".cache")
OUT = os.path.join(ROOT, "src-tauri", "resources", "ecdict.sqlite")


def load_converter():
    try:
        import opencc
    except ImportError:
        sys.exit("缺少 OpenCC。請先安裝建置工具：pip3 install opencc")
    # s2twp：簡體 → 繁體 + 台灣慣用詞（軟件→軟體、内存→記憶體、数组→陣列…）
    return opencc.OpenCC("s2twp")


def download_source():
    os.makedirs(CACHE, exist_ok=True)
    zip_path = os.path.join(CACHE, f"ecdict-sqlite-{ECDICT_VERSION}.zip")
    if not os.path.exists(zip_path):
        print(f"下載 ECDICT {ECDICT_VERSION}（約 207MB，只需一次）…")
        urllib.request.urlretrieve(ECDICT_URL, zip_path)
    db_path = os.path.join(CACHE, "stardict.db")
    if not os.path.exists(db_path):
        print("解壓 stardict.db…")
        with zipfile.ZipFile(zip_path) as z:
            name = next(n for n in z.namelist() if n.endswith(".db"))
            with z.open(name) as src, open(db_path, "wb") as dst:
                dst.write(src.read())
    return db_path


def build(src_db, cc):
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    if os.path.exists(OUT):
        os.remove(OUT)

    src = sqlite3.connect(src_db)
    out = sqlite3.connect(OUT)
    out.execute("PRAGMA journal_mode=OFF")
    out.execute(
        "CREATE TABLE ecdict (word_lower TEXT, word TEXT, phonetic TEXT, pos TEXT, translation TEXT)"
    )

    out.execute("CREATE TABLE lemma (form TEXT, word TEXT)")  # 變化型 → 原形

    # 只收「有真實訊號」的詞：有語料頻率(bnc/frq)、或在考試詞表(tag)、或有 collins/oxford
    # 標記。濾掉完整版的長尾（罕見學名、變形、專名），把體積從 100MB+ 壓到數十 MB。
    rows = src.execute(
        "SELECT word, phonetic, pos, translation, exchange, bnc, frq, tag, collins, oxford FROM stardict"
    )
    batch, n, skipped = [], 0, 0
    lemma_map = {}  # form(lower) → 原形 word；同形取第一個
    for word, phonetic, pos, translation, exchange, bnc, frq, tag, collins, oxford in rows:
        if not word or not translation:
            continue
        if not SINGLE_WORD.match(word):  # 濾掉片語/慣用語/含數字符號的條目
            skipped += 1
            continue
        notable = (bnc or 0) > 0 or (frq or 0) > 0 or (tag or "").strip() or (collins or 0) > 0 or (oxford or 0) > 0
        if not notable:  # 長尾罕見詞 → 交給 Gemini fallback
            skipped += 1
            continue
        zh_tw = cc.convert(translation)  # 簡轉繁（台灣用詞）
        batch.append((word.lower(), word, phonetic or "", pos or "", zh_tw))
        # 收集此原形的變化型 → 原形對照（變化型若本身也是收錄字，查詢時 direct 先命中，不受影響）
        for form in parse_inflections(exchange):
            if form != word.lower():
                lemma_map.setdefault(form, word)
        n += 1
        if len(batch) >= 5000:
            out.executemany("INSERT INTO ecdict VALUES (?,?,?,?,?)", batch)
            batch.clear()
            print(f"\r  轉換中… {n} 詞", end="", flush=True)
    if batch:
        out.executemany("INSERT INTO ecdict VALUES (?,?,?,?,?)", batch)

    out.executemany("INSERT INTO lemma VALUES (?,?)", lemma_map.items())
    out.execute("CREATE INDEX idx_word_lower ON ecdict(word_lower)")
    out.execute("CREATE INDEX idx_lemma_form ON lemma(form)")
    out.commit()
    out.execute("VACUUM")
    out.close()
    src.close()
    size_mb = os.path.getsize(OUT) // (1024 * 1024)
    print(f"\n完成：收錄 {n} 單字、{len(lemma_map)} 變化型對照（濾掉 {skipped} 片語/非單字）")
    print(f"      → {OUT}（{size_mb}MB）")
    print(f"來源：ECDICT {ECDICT_VERSION}，授權 {ECDICT_LICENSE}")


if __name__ == "__main__":
    cc = load_converter()
    db = download_source()
    build(db, cc)

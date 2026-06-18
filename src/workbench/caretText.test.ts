import { describe, it, expect } from "vitest";
import { wordAtCaret, sentenceAtCaret } from "./caretText";

// 小工具：以游標「點在」某字元上來呼叫（click 情境 start === end）。
const at = (text: string, pos: number) => wordAtCaret(text, pos, pos);

describe("wordAtCaret — 選取（end > start）", () => {
  it("選到一個乾淨的字 → 回該字", () => {
    const text = "the quick fox";
    // 選取 "quick"
    expect(wordAtCaret(text, 4, 9)).toBe("quick");
  });

  it("選取含前後空白 → 修剪後仍是合法字", () => {
    const text = "  hello  world";
    // 選 "  hello " （含兩側空白）
    expect(wordAtCaret(text, 0, 8)).toBe("hello");
  });

  it("選到跨空白的多個字 → null（非單一字）", () => {
    const text = "the quick fox";
    expect(wordAtCaret(text, 0, 9)).toBeNull(); // "the quick"
  });

  it("選取包含尾端標點 → null", () => {
    const text = "hello.";
    expect(wordAtCaret(text, 0, 6)).toBeNull(); // "hello."
  });

  it("選取連字號字 → 保留連字號", () => {
    const text = "a well-known fact";
    expect(wordAtCaret(text, 2, 12)).toBe("well-known");
  });

  it("選取以數字開頭 → null（須以字母開頭）", () => {
    const text = "h2o2 sample";
    expect(wordAtCaret(text, 0, 4)).toBeNull(); // "h2o2" 含數字
  });
});

describe("wordAtCaret — 純點擊（start === end）", () => {
  const text = "the quick brown fox";

  it("游標在詞首（右側是字母）→ 回整個字", () => {
    // "quick" 從 index 4 開始；點在 q 之前
    expect(at(text, 4)).toBe("quick");
  });

  it("游標在詞中 → 回整個字", () => {
    expect(at(text, 6)).toBe("quick"); // 在 'i' 上
  });

  it("游標在詞尾（右側非字母）→ null，避免誤觸最後一字（issue #8）", () => {
    // "quick" 結束於 index 9（其右為空白）
    expect(at(text, 9)).toBeNull();
  });

  it("點在字與字之間的空白 → null", () => {
    expect(at(text, 3)).toBeNull(); // 'the' 後的空白
  });

  it("點在文字最尾端（句尾無標點）→ null", () => {
    expect(at(text, text.length)).toBeNull();
  });

  it("句尾有標點時點在標點上 → null", () => {
    const t = "hello world.";
    expect(at(t, 11)).toBeNull(); // '.' 上
  });
});

describe("wordAtCaret — 連字號與撇號", () => {
  it("點在連字號字中間 → 跨連字號展開整個字", () => {
    const text = "a well-known fact";
    expect(at(text, 7)).toBe("well-known"); // 在 'k' (known) 上
    expect(at(text, 4)).toBe("well-known"); // 在 'l' (well) 上
  });

  it("撇號縮寫 → 保留撇號", () => {
    const text = "I don't know";
    expect(at(text, 4)).toBe("don't"); // 在 'o' 上
  });

  it("修剪字首/字尾多餘的連字號與撇號", () => {
    // "--foo--" 周圍是空白；點在 foo 上應得 "foo"，邊緣連字號被修掉
    const text = "x --foo-- y";
    expect(at(text, 5)).toBe("foo"); // 在第一個 'o' 上
  });

  it("純連字號（修剪後為空）→ null", () => {
    const text = "a --- b";
    expect(at(text, 3)).toBeNull(); // 點在中間 '-' 上
  });
});

describe("sentenceAtCaret — 句界切分", () => {
  it("以句號切出所在句", () => {
    const text = "First one. Second two. Third three.";
    // 游標落在 "Second" 區段
    const pos = text.indexOf("Second") + 2;
    expect(sentenceAtCaret(text, pos)).toBe("Second two.");
  });

  it("換行也算句界 → 夾在多行裡的機密那行被獨立", () => {
    const text = "intro line\napi_key=sk-secretvalue\noutro line";
    const pos = text.indexOf("api_key") + 3;
    expect(sentenceAtCaret(text, pos)).toBe("api_key=sk-secretvalue");
  });

  it("沒有任何句界 → 回整段修剪後文字", () => {
    const text = "  just one clause without boundary  ";
    expect(sentenceAtCaret(text, 5)).toBe("just one clause without boundary");
  });

  it("含 CJK 句界（。！？）", () => {
    const text = "你好世界。第二句！第三句？";
    const pos = text.indexOf("第二") + 1;
    expect(sentenceAtCaret(text, pos)).toBe("第二句！");
  });
});

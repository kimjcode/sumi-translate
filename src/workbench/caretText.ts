//! 從 textarea 游標位置抽出「單字」或「句子」的純函式（無 DOM、無 React，可單測）。
//! 抽出自 Workbench.tsx，行為不變；測試見 caretText.test.ts。

/// 從游標位置抓出英文單字（向左右掃描到非字母為止）。有選取就優先用選取。
export function wordAtCaret(text: string, start: number, end: number): string | null {
  const isWordChar = (c: string | undefined) => c != null && /[A-Za-z'-]/.test(c);

  if (end > start) {
    const sel = text.slice(start, end).trim();
    return /^[A-Za-z][A-Za-z'-]*$/.test(sel) ? sel : null;
  }
  // 純點擊：游標右側必須是字母，代表真的點在字上。點空白／行尾／空格（游標
  // 會吸附到文字結尾）時右側非字母 → 不查，避免誤觸最後一個字。
  if (!isWordChar(text[start])) return null;
  let l = start;
  let r = start;
  while (l > 0 && isWordChar(text[l - 1])) l--;
  while (r < text.length && isWordChar(text[r])) r++;
  const word = text.slice(l, r).replace(/^[-']+|[-']+$/g, "");
  return /^[A-Za-z][A-Za-z'-]*$/.test(word) ? word : null;
}

/// 抓出游標所在的「句子」：向左右掃描到句界（. ! ? 。！？ 換行）為止。
/// 換行也算邊界，讓夾在多行設定/log 裡的機密那行被獨立成一段，後端機密過濾才能命中。
export function sentenceAtCaret(text: string, pos: number): string {
  const isBoundary = (c: string) => c === "\n" || /[.!?。！？]/.test(c);
  let l = pos;
  let r = pos;
  while (l > 0 && !isBoundary(text[l - 1])) l--;
  while (r < text.length && !isBoundary(text[r])) r++;
  if (r < text.length) r++; // 含句尾標點，語境更完整
  const sentence = text.slice(l, r).trim();
  return sentence || text.trim();
}

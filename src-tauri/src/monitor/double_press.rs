//! 雙擊 ⌘C 判定的純邏輯，不碰 OS API，可單元測試。

use std::time::{Duration, Instant};

/// 預設雙擊判定時間窗（毫秒）。實際值由設定提供（可調），此為預設與測試用。
pub const DOUBLE_PRESS_WINDOW_MS: u64 = 300;

/// 一次按下的判定結果。
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Press {
    /// 按住不放的 key-repeat，不算新按下。
    Ignored,
    /// 記錄為新一輪的「第一次按下」（呼叫端可在此抓 changeCount 基準）。
    Started,
    /// 與第一次間隔在時間窗內 → 構成雙擊。
    Fired,
}

/// 判定「時間窗內的第二次按下」。
///
/// 規則：
/// - 第一次按下只記錄時間（`Started`）。
/// - 第二次按下與第一次間隔在時間窗內 → `Fired`，並重置（第三次按下重新從頭計）。
/// - 兩次按下之間必須有放開（key release），排除按住不放的 key-repeat 連發（`Ignored`）。
pub struct DoublePressDetector {
    last_press: Option<Instant>,
    released_since_last_press: bool,
}

impl DoublePressDetector {
    pub fn new() -> Self {
        Self {
            last_press: None,
            released_since_last_press: true,
        }
    }

    /// 回報一次按下事件。`window` 每次傳入，支援設定即時生效。
    pub fn on_press(&mut self, now: Instant, window: Duration) -> Press {
        if !self.released_since_last_press {
            return Press::Ignored;
        }
        self.released_since_last_press = false;

        match self.last_press {
            Some(prev) if now.duration_since(prev) <= window => {
                self.last_press = None;
                Press::Fired
            }
            _ => {
                self.last_press = Some(now);
                Press::Started
            }
        }
    }

    /// 回報按鍵放開事件。
    pub fn on_release(&mut self) {
        self.released_since_last_press = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: Duration = Duration::from_millis(DOUBLE_PRESS_WINDOW_MS);

    fn press_release(d: &mut DoublePressDetector, t: Instant) -> Press {
        let p = d.on_press(t, WINDOW);
        d.on_release();
        p
    }

    #[test]
    fn single_press_starts_not_fires() {
        let mut d = DoublePressDetector::new();
        assert_eq!(press_release(&mut d, Instant::now()), Press::Started);
    }

    #[test]
    fn two_presses_within_window_fire() {
        let mut d = DoublePressDetector::new();
        let t0 = Instant::now();
        assert_eq!(press_release(&mut d, t0), Press::Started);
        assert_eq!(press_release(&mut d, t0 + Duration::from_millis(150)), Press::Fired);
    }

    #[test]
    fn two_presses_outside_window_both_start() {
        let mut d = DoublePressDetector::new();
        let t0 = Instant::now();
        assert_eq!(press_release(&mut d, t0), Press::Started);
        assert_eq!(press_release(&mut d, t0 + Duration::from_millis(301)), Press::Started);
    }

    #[test]
    fn key_repeat_without_release_is_ignored() {
        let mut d = DoublePressDetector::new();
        let t0 = Instant::now();
        assert_eq!(d.on_press(t0, WINDOW), Press::Started);
        // 按住不放，OS 連發 press、沒有 release。
        assert_eq!(d.on_press(t0 + Duration::from_millis(100), WINDOW), Press::Ignored);
        assert_eq!(d.on_press(t0 + Duration::from_millis(200), WINDOW), Press::Ignored);
    }

    #[test]
    fn fires_again_after_reset() {
        let mut d = DoublePressDetector::new();
        let t0 = Instant::now();
        assert_eq!(press_release(&mut d, t0), Press::Started);
        assert_eq!(press_release(&mut d, t0 + Duration::from_millis(100)), Press::Fired);
        // 觸發後重置：下一次按下是新的「第一次」。
        let t1 = t0 + Duration::from_millis(250);
        assert_eq!(press_release(&mut d, t1), Press::Started);
        assert_eq!(press_release(&mut d, t1 + Duration::from_millis(100)), Press::Fired);
    }

    #[test]
    fn late_second_press_starts_new_window() {
        let mut d = DoublePressDetector::new();
        let t0 = Instant::now();
        assert_eq!(press_release(&mut d, t0), Press::Started);
        // 超窗的第二下不觸發，但成為新窗的第一下。
        let t1 = t0 + Duration::from_millis(400);
        assert_eq!(press_release(&mut d, t1), Press::Started);
        assert_eq!(press_release(&mut d, t1 + Duration::from_millis(100)), Press::Fired);
    }

    #[test]
    fn respects_custom_window() {
        let mut d = DoublePressDetector::new();
        let wide = Duration::from_millis(600);
        let t0 = Instant::now();
        assert_eq!(d.on_press(t0, wide), Press::Started);
        d.on_release();
        assert_eq!(d.on_press(t0 + Duration::from_millis(450), wide), Press::Fired);
    }
}

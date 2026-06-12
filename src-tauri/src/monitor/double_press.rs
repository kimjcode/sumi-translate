//! 雙擊 ⌘C 判定的純邏輯，不碰 OS API，可單元測試。

use std::time::{Duration, Instant};

/// 雙擊判定時間窗（毫秒）。之後要做成使用者設定，先以常數集中管理。
pub const DOUBLE_PRESS_WINDOW_MS: u64 = 300;

/// 判定「時間窗內的第二次按下」。
///
/// 規則：
/// - 第一次按下只記錄時間，不觸發。
/// - 第二次按下與第一次間隔在時間窗內 → 觸發，並重置狀態（第三次按下重新從頭計）。
/// - 兩次按下之間必須有放開（key release），排除按住不放的 key-repeat 連發。
pub struct DoublePressDetector {
    window: Duration,
    last_press: Option<Instant>,
    released_since_last_press: bool,
}

impl DoublePressDetector {
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            last_press: None,
            released_since_last_press: true,
        }
    }

    /// 回報一次按下事件，回傳是否構成「雙擊」。
    pub fn on_press(&mut self, now: Instant) -> bool {
        if !self.released_since_last_press {
            // 按住不放的 OS key-repeat：不算新的一次按下。
            return false;
        }
        self.released_since_last_press = false;

        match self.last_press {
            Some(prev) if now.duration_since(prev) <= self.window => {
                self.last_press = None;
                true
            }
            _ => {
                self.last_press = Some(now);
                false
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

    fn detector() -> DoublePressDetector {
        DoublePressDetector::new(Duration::from_millis(DOUBLE_PRESS_WINDOW_MS))
    }

    fn press_release(d: &mut DoublePressDetector, t: Instant) -> bool {
        let fired = d.on_press(t);
        d.on_release();
        fired
    }

    #[test]
    fn single_press_does_not_fire() {
        let mut d = detector();
        assert!(!press_release(&mut d, Instant::now()));
    }

    #[test]
    fn two_presses_within_window_fire() {
        let mut d = detector();
        let t0 = Instant::now();
        assert!(!press_release(&mut d, t0));
        assert!(press_release(&mut d, t0 + Duration::from_millis(150)));
    }

    #[test]
    fn two_presses_outside_window_do_not_fire() {
        let mut d = detector();
        let t0 = Instant::now();
        assert!(!press_release(&mut d, t0));
        assert!(!press_release(&mut d, t0 + Duration::from_millis(301)));
    }

    #[test]
    fn key_repeat_without_release_does_not_fire() {
        let mut d = detector();
        let t0 = Instant::now();
        assert!(!d.on_press(t0));
        // 按住不放，OS 連發 press、沒有 release。
        assert!(!d.on_press(t0 + Duration::from_millis(100)));
        assert!(!d.on_press(t0 + Duration::from_millis(200)));
    }

    #[test]
    fn fires_again_after_reset() {
        let mut d = detector();
        let t0 = Instant::now();
        assert!(!press_release(&mut d, t0));
        assert!(press_release(&mut d, t0 + Duration::from_millis(100)));
        // 觸發後重置：下一次按下是新的「第一次」。
        let t1 = t0 + Duration::from_millis(250);
        assert!(!press_release(&mut d, t1));
        assert!(press_release(&mut d, t1 + Duration::from_millis(100)));
    }

    #[test]
    fn late_second_press_starts_new_window() {
        let mut d = detector();
        let t0 = Instant::now();
        assert!(!press_release(&mut d, t0));
        // 超窗的第二下不觸發，但成為新窗的第一下。
        let t1 = t0 + Duration::from_millis(400);
        assert!(!press_release(&mut d, t1));
        assert!(press_release(&mut d, t1 + Duration::from_millis(100)));
    }
}

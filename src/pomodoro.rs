/// Pomodoro Timer Module
///
/// Implements a simple Pomodoro technique timer with:
/// - 25 minute work sessions
/// - 5 minute short breaks
/// - 15 minute long breaks (every 4 pomodoros)

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Default durations in seconds
pub const DEFAULT_WORK_MINS: u64 = 25;
pub const DEFAULT_SHORT_BREAK_MINS: u64 = 5;
pub const DEFAULT_LONG_BREAK_MINS: u64 = 15;
pub const POMODOROS_UNTIL_LONG_BREAK: u32 = 4;

/// Timer state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimerState {
    Idle,
    Working,
    ShortBreak,
    LongBreak,
    Paused,
}

impl TimerState {
    pub fn as_str(&self) -> &'static str {
        match self {
            TimerState::Idle => "idle",
            TimerState::Working => "working",
            TimerState::ShortBreak => "short_break",
            TimerState::LongBreak => "long_break",
            TimerState::Paused => "paused",
        }
    }
}

/// Pomodoro timer
pub struct PomodoroTimer {
    state: RwLock<TimerState>,
    state_before_pause: RwLock<TimerState>,
    remaining_secs: AtomicU64,
    completed_pomodoros: AtomicU64,
    last_tick: RwLock<Option<Instant>>,
    enabled: AtomicBool,

    // Configurable durations (in seconds)
    work_duration: u64,
    short_break_duration: u64,
    long_break_duration: u64,
}

impl PomodoroTimer {
    /// Create a new timer with default durations
    pub fn new() -> Self {
        Self {
            state: RwLock::new(TimerState::Idle),
            state_before_pause: RwLock::new(TimerState::Idle),
            remaining_secs: AtomicU64::new(DEFAULT_WORK_MINS * 60),
            completed_pomodoros: AtomicU64::new(0),
            last_tick: RwLock::new(None),
            enabled: AtomicBool::new(true),
            work_duration: DEFAULT_WORK_MINS * 60,
            short_break_duration: DEFAULT_SHORT_BREAK_MINS * 60,
            long_break_duration: DEFAULT_LONG_BREAK_MINS * 60,
        }
    }

    /// Create with custom durations (in minutes)
    pub fn with_durations(work_mins: u64, short_break_mins: u64, long_break_mins: u64) -> Self {
        Self {
            state: RwLock::new(TimerState::Idle),
            state_before_pause: RwLock::new(TimerState::Idle),
            remaining_secs: AtomicU64::new(work_mins * 60),
            completed_pomodoros: AtomicU64::new(0),
            last_tick: RwLock::new(None),
            enabled: AtomicBool::new(true),
            work_duration: work_mins * 60,
            short_break_duration: short_break_mins * 60,
            long_break_duration: long_break_mins * 60,
        }
    }

    /// Start a work session
    pub async fn start_work(&self) {
        let mut state = self.state.write().await;
        *state = TimerState::Working;
        self.remaining_secs.store(self.work_duration, Ordering::SeqCst);
        *self.last_tick.write().await = Some(Instant::now());
    }

    /// Start a break (auto-selects short or long based on completed pomodoros)
    pub async fn start_break(&self) {
        let completed = self.completed_pomodoros.load(Ordering::SeqCst) as u32;
        let mut state = self.state.write().await;

        if completed > 0 && completed % POMODOROS_UNTIL_LONG_BREAK == 0 {
            *state = TimerState::LongBreak;
            self.remaining_secs.store(self.long_break_duration, Ordering::SeqCst);
        } else {
            *state = TimerState::ShortBreak;
            self.remaining_secs.store(self.short_break_duration, Ordering::SeqCst);
        }
        *self.last_tick.write().await = Some(Instant::now());
    }

    /// Pause the timer
    pub async fn pause(&self) {
        let mut state = self.state.write().await;
        if *state != TimerState::Idle && *state != TimerState::Paused {
            *self.state_before_pause.write().await = *state;
            *state = TimerState::Paused;
        }
    }

    /// Resume from pause
    pub async fn resume(&self) {
        let mut state = self.state.write().await;
        if *state == TimerState::Paused {
            *state = *self.state_before_pause.read().await;
            *self.last_tick.write().await = Some(Instant::now());
        }
    }

    /// Reset to idle state
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        *state = TimerState::Idle;
        self.remaining_secs.store(self.work_duration, Ordering::SeqCst);
        self.completed_pomodoros.store(0, Ordering::SeqCst);
        *self.last_tick.write().await = None;
    }

    /// Skip current session (work or break)
    pub async fn skip(&self) {
        let state = *self.state.read().await;
        match state {
            TimerState::Working => {
                // Don't count skipped work sessions
                self.start_break().await;
            }
            TimerState::ShortBreak | TimerState::LongBreak => {
                self.start_work().await;
            }
            _ => {}
        }
    }

    /// Tick the timer (call this every second)
    /// Returns true if the session just completed
    pub async fn tick(&self) -> bool {
        let state = *self.state.read().await;

        if state == TimerState::Idle || state == TimerState::Paused {
            return false;
        }

        let remaining = self.remaining_secs.load(Ordering::SeqCst);

        if remaining > 0 {
            self.remaining_secs.fetch_sub(1, Ordering::SeqCst);
            false
        } else {
            // Session complete
            match state {
                TimerState::Working => {
                    self.completed_pomodoros.fetch_add(1, Ordering::SeqCst);
                    self.start_break().await;
                }
                TimerState::ShortBreak | TimerState::LongBreak => {
                    self.start_work().await;
                }
                _ => {}
            }
            true
        }
    }

    /// Get current state
    pub async fn get_state(&self) -> TimerState {
        *self.state.read().await
    }

    /// Get remaining seconds
    pub fn get_remaining_secs(&self) -> u64 {
        self.remaining_secs.load(Ordering::SeqCst)
    }

    /// Get completed pomodoros count
    pub fn get_completed_pomodoros(&self) -> u64 {
        self.completed_pomodoros.load(Ordering::SeqCst)
    }

    /// Check if timer is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    /// Enable/disable the timer
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    /// Format remaining time as MM:SS
    pub fn format_remaining(&self) -> String {
        let secs = self.remaining_secs.load(Ordering::SeqCst);
        let mins = secs / 60;
        let secs = secs % 60;
        format!("{:02}:{:02}", mins, secs)
    }

    /// Get status for tray display
    pub async fn get_tray_status(&self) -> Option<String> {
        let state = *self.state.read().await;
        match state {
            TimerState::Idle => None,
            TimerState::Working => Some(format!(" | {} W", self.format_remaining())),
            TimerState::ShortBreak => Some(format!(" | {} B", self.format_remaining())),
            TimerState::LongBreak => Some(format!(" | {} LB", self.format_remaining())),
            TimerState::Paused => Some(format!(" | {} P", self.format_remaining())),
        }
    }
}

impl Default for PomodoroTimer {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe wrapper for sharing across async tasks
pub type SharedPomodoro = Arc<PomodoroTimer>;

/// Create a new shared pomodoro timer
pub fn create_shared_pomodoro() -> SharedPomodoro {
    Arc::new(PomodoroTimer::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_timer_start_work() {
        let timer = PomodoroTimer::new();
        timer.start_work().await;

        assert_eq!(timer.get_state().await, TimerState::Working);
        assert_eq!(timer.get_remaining_secs(), DEFAULT_WORK_MINS * 60);
    }

    #[tokio::test]
    async fn test_timer_pause_resume() {
        let timer = PomodoroTimer::new();
        timer.start_work().await;
        timer.pause().await;

        assert_eq!(timer.get_state().await, TimerState::Paused);

        timer.resume().await;
        assert_eq!(timer.get_state().await, TimerState::Working);
    }

    #[tokio::test]
    async fn test_timer_tick() {
        let timer = PomodoroTimer::with_durations(1, 1, 1); // 1 minute each
        timer.start_work().await;

        // Tick 60 times
        for _ in 0..60 {
            timer.tick().await;
        }

        // Should have switched to break
        let state = timer.get_state().await;
        assert!(state == TimerState::ShortBreak || state == TimerState::LongBreak);
        assert_eq!(timer.get_completed_pomodoros(), 1);
    }
}

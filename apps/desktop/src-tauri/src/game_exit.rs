#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Observation {
    Running,
    Stopped,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct GameExitState {
    pub status: &'static str,
    pub reason: Option<&'static str>,
    pub token: u64,
    armed: bool,
    exited_at: Option<u64>,
    changed_at: u64,
    sampled_at: u64,
}
impl Default for GameExitState {
    fn default() -> Self {
        Self {
            status: "waiting",
            reason: None,
            token: 0,
            armed: false,
            exited_at: None,
            changed_at: 0,
            sampled_at: 0,
        }
    }
}
impl GameExitState {
    pub fn changed(&mut self, now: u64) {
        self.changed_at = now;
        self.token += 1;
        if self.status == "backing_up" {
            self.status = "settling";
        }
    }
    pub fn suppress(&mut self) {
        self.token += 1;
        self.armed = false;
        self.exited_at = None;
        self.status = "waiting";
        self.reason = None;
    }
    pub fn step(
        &mut self,
        now: u64,
        observation: Observation,
        quiet: u64,
        suppressed: bool,
    ) -> bool {
        self.sampled_at = now;
        if suppressed {
            self.suppress();
            return false;
        }
        match observation {
            Observation::Running => {
                if self.status != "running" {
                    self.token += 1;
                }
                self.armed = true;
                self.exited_at = None;
                self.status = "running";
                self.reason = None;
                return false;
            }
            Observation::Unknown => {
                self.token += 1;
                self.exited_at = None;
                self.status = "needs_attention";
                self.reason = Some("backup_process_unknown");
                return false;
            }
            Observation::Stopped => {}
        }
        if !self.armed {
            return false;
        }
        let exit = *self.exited_at.get_or_insert(now);
        // The upper bound is for waiting on continuing writes, not for hashing
        // or compression of a large, already stable save.
        if self.status == "backing_up" {
            return false;
        }
        let limit = if quiet <= 120_000 {
            120_000
        } else {
            quiet + 120_000
        };
        if now.saturating_sub(exit) > limit {
            self.token += 1;
            self.armed = false;
            self.status = "needs_attention";
            self.reason = Some("backup_settle_timeout");
            return false;
        }
        self.status = "settling";
        self.reason = None;
        if now.saturating_sub(exit.max(self.changed_at)) >= quiet {
            self.token += 1;
            self.status = "backing_up";
            return true;
        }
        false
    }
    pub fn can_commit(&self, token: u64, now: u64) -> bool {
        self.token == token
            && self.status == "backing_up"
            && now.saturating_sub(self.sampled_at) <= 3_000
    }
    pub fn finish(&mut self, token: u64, outcome: &str) {
        if self.token != token {
            return;
        }
        self.armed = false;
        self.exited_at = None;
        self.status = match outcome {
            "created" => "waiting",
            "unchanged" => "unchanged",
            _ => "needs_attention",
        };
        self.reason = if outcome == "failed" {
            Some("backup_capture_failed")
        } else {
            None
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_stable_capture_can_take_longer_than_settling_timeout() {
        let mut state = GameExitState::default();
        state.step(0, Observation::Running, 5000, false);
        state.step(1, Observation::Stopped, 5000, false);
        assert!(state.step(5001, Observation::Stopped, 5000, false));
        let token = state.token;
        assert!(!state.step(300_000, Observation::Stopped, 5000, false));
        assert!(state.can_commit(token, 300_000));
    }
    #[test]
    fn continuing_writes_timeout_and_success_does_not_repeat() {
        let mut state = GameExitState::default();
        state.step(0, Observation::Running, 5000, false);
        state.step(1, Observation::Stopped, 5000, false);
        state.changed(120_000);
        assert!(!state.step(120_002, Observation::Stopped, 5000, false));
        assert_eq!(state.reason, Some("backup_settle_timeout"));
        assert!(!state.step(130_000, Observation::Stopped, 5000, false));
        state.step(140_000, Observation::Running, 5000, false);
        state.step(150_000, Observation::Stopped, 5000, false);
        assert!(state.step(155_000, Observation::Stopped, 5000, false));
        state.finish(state.token, "unchanged");
        assert!(!state.step(180_000, Observation::Stopped, 5000, false));
        assert_eq!(state.status, "unchanged");
    }
    #[test]
    fn only_observed_session_exits_trigger_once_after_quiet_period() {
        let mut state = GameExitState::default();
        assert!(!state.step(0, Observation::Stopped, 5000, false));
        assert!(!state.step(10, Observation::Running, 5000, false));
        assert!(!state.step(20, Observation::Stopped, 5000, false));
        assert!(!state.step(5019, Observation::Stopped, 5000, false));
        assert!(state.step(5020, Observation::Stopped, 5000, false));
        assert!(!state.step(7020, Observation::Stopped, 5000, false));
    }
    #[test]
    fn unknown_restart_changes_and_restore_never_commit_stale_request() {
        let mut state = GameExitState::default();
        state.step(0, Observation::Running, 5000, false);
        state.step(10, Observation::Unknown, 5000, false);
        assert!(!state.step(6000, Observation::Stopped, 5000, false));
        state.changed(8000);
        assert!(!state.step(11000, Observation::Stopped, 5000, false));
        assert!(state.step(13000, Observation::Stopped, 5000, false));
        let token = state.token;
        assert!(state.can_commit(token, 13000));
        state.step(14000, Observation::Running, 5000, false);
        assert!(!state.can_commit(token, 14000));
        state.step(15000, Observation::Stopped, 5000, true);
        assert!(!state.step(30000, Observation::Stopped, 5000, false));
    }
}

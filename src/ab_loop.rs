//! A-B repeat: once both points are set, playback past B jumps back to A.

/// Loop points in seconds. B is always after A once both are set.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AbLoop {
    pub a: Option<f64>,
    pub b: Option<f64>,
}

/// A loop shorter than this would just stutter; setting B that close to A is refused.
pub const MIN_LOOP_SECS: f64 = 0.2;

impl AbLoop {
    pub fn is_active(&self) -> bool {
        self.a.is_some() && self.b.is_some()
    }

    pub fn set_a(&mut self, t: f64) {
        self.a = Some(t.max(0.0));
        // A moved past B: the old B no longer makes sense.
        if self.b.is_some_and(|b| b < t + MIN_LOOP_SECS) {
            self.b = None;
        }
    }

    /// Sets B; returns false (and changes nothing) if A is unset or B isn't after A.
    pub fn set_b(&mut self, t: f64) -> bool {
        match self.a {
            Some(a) if t >= a + MIN_LOOP_SECS => {
                self.b = Some(t);
                true
            }
            _ => false,
        }
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Where to seek when playback at `t` (or ended, with `t` at the duration) has passed B.
    pub fn jump_target(&self, t: f64) -> Option<f64> {
        match (self.a, self.b) {
            (Some(a), Some(b)) if t >= b => Some(a),
            _ => None,
        }
    }
}

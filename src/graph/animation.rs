/// Animation utilities for smooth transitions.

#[derive(Debug, Clone)]
pub struct Animation {
    elapsed: f64,
    duration: f64,
    active: bool,
}

impl Animation {
    pub fn new() -> Self {
        Self {
            elapsed: 0.0,
            duration: 200.0,
            active: false,
        }
    }

    pub fn start(&mut self, duration_ms: f64) {
        self.elapsed = 0.0;
        self.duration = duration_ms;
        self.active = true;
    }

    pub fn update(&mut self, dt_ms: f64) {
        if self.active {
            self.elapsed += dt_ms;
            if self.elapsed >= self.duration {
                self.elapsed = self.duration;
                self.active = false;
            }
        }
    }

    /// Returns eased progress (0.0..=1.0) with ease-out cubic.
    pub fn progress(&self) -> f64 {
        if self.duration <= 0.0 {
            return 1.0;
        }
        let t = (self.elapsed / self.duration).clamp(0.0, 1.0);
        Self::ease_out(t)
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn is_complete(&self) -> bool {
        !self.active && self.elapsed >= self.duration
    }

    /// Ease-out cubic: 1 - (1 - t)^3
    fn ease_out(t: f64) -> f64 {
        let inv = 1.0 - t;
        1.0 - inv * inv * inv
    }
}

impl Default for Animation {
    fn default() -> Self {
        Self::new()
    }
}

/// Linear interpolation between two values.
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Smooth interpolation for hover effects (approaches target over time).
pub fn smooth_towards(current: f64, target: f64, dt_ms: f64, speed: f64) -> f64 {
    let factor = 1.0 - (-speed * dt_ms / 1000.0).exp();
    current + (target - current) * factor
}

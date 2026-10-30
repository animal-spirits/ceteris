//! Presented time (ADRs 0208, 0232): monthly ticks shown as a daily date that advances at the
//! chosen speed between the two bracketing views. Panels show the current tick's values; the
//! fraction only interpolates the map.

/// Simulated days per wall second at speeds 1 to 4; speed 5 runs as fast as views arrive.
const DAYS_PER_SECOND: [f64; 4] = [1.0, 7.0, 365.25 / 12.0, 365.25 / 4.0];
pub const FASTEST: u8 = 5;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Tick 0 is 1 January 2000; tick t starts on the first of the t-th month after it.
pub fn month(tick: u32) -> (u32, usize) {
    (2000 + tick / 12, (tick % 12) as usize)
}

pub fn days_in_month(tick: u32) -> u32 {
    let (year, m) = month(tick);
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    match m {
        1 if leap => 29,
        1 => 28,
        3 | 5 | 8 | 10 => 30,
        _ => 31,
    }
}

/// "15 Mar 2000": day `day` (from 0) of tick `tick`'s month.
pub fn date(tick: u32, day: u32) -> String {
    let (year, m) = month(tick);
    format!("{} {} {year}", day + 1, MONTHS[m])
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clock {
    pub playing: bool,
    /// 1 to 5.
    pub speed: u8,
    /// Presented tick, and how far into its month (0 to 1).
    pub tick: u32,
    pub fraction: f64,
    /// Playing but held at the month's last day for want of the next view.
    pub sim_bound: bool,
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            playing: false,
            speed: 3,
            tick: 0,
            fraction: 0.0,
            sim_bound: false,
        }
    }
}

impl Clock {
    pub fn day(&self) -> u32 {
        let days = days_in_month(self.tick);
        ((self.fraction * f64::from(days)) as u32).min(days - 1)
    }

    pub fn date(&self) -> String {
        date(self.tick, self.day())
    }

    /// Advances `seconds` of wall time. `ready` says whether the next tick's view is in hand;
    /// returns true when the clock crosses into it, which the caller must then present.
    pub fn advance(&mut self, seconds: f64, ready: bool) -> bool {
        self.sim_bound = false;
        if !self.playing {
            return false;
        }
        if self.speed >= FASTEST {
            if ready {
                self.cross();
                return true;
            }
            self.sim_bound = true;
            return false;
        }
        let rate = DAYS_PER_SECOND[usize::from(self.speed - 1)];
        self.fraction += seconds * rate / f64::from(days_in_month(self.tick));
        if self.fraction < 1.0 {
            return false;
        }
        if ready {
            self.cross();
            return true;
        }
        let days = f64::from(days_in_month(self.tick));
        self.fraction = (days - 1.0) / days;
        self.sim_bound = true;
        false
    }

    /// Steps to the next tick's start; the caller checks the view is ready.
    pub fn step(&mut self) {
        self.playing = false;
        self.cross();
    }

    fn cross(&mut self) {
        // Carry the overshoot at speeds 1-4, but never past one month in a frame.
        self.fraction = (self.fraction - 1.0).clamp(0.0, 0.999);
        if self.speed >= FASTEST {
            self.fraction = 0.0;
        }
        self.tick += 1;
    }

    pub fn set_speed(&mut self, speed: u8) {
        self.speed = speed.clamp(1, FASTEST);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar() {
        assert_eq!(date(0, 0), "1 Jan 2000");
        assert_eq!(days_in_month(1), 29);
        assert_eq!(days_in_month(13), 28);
        assert_eq!(days_in_month(12 * 100 + 1), 28);
        assert_eq!(date(14, 20), "21 Mar 2001");
    }

    #[test]
    fn speed_one_counts_a_day_per_second() {
        let mut c = Clock {
            playing: true,
            speed: 1,
            ..Clock::default()
        };
        c.advance(14.5, true);
        assert_eq!(c.date(), "15 Jan 2000");
        assert!(c.advance(17.0, true));
        assert_eq!((c.tick, c.date()), (1, "1 Feb 2000".to_owned()));
    }

    #[test]
    fn without_the_next_view_the_date_holds_at_the_months_last_day() {
        let mut c = Clock {
            playing: true,
            speed: 4,
            ..Clock::default()
        };
        assert!(!c.advance(1.0, false));
        assert!(c.sim_bound);
        assert_eq!(c.date(), "31 Jan 2000");
        assert!(c.advance(0.1, true));
        assert_eq!(c.tick, 1);
        assert!(!c.sim_bound);
    }

    #[test]
    fn paused_time_does_not_move_and_step_crosses_one_tick() {
        let mut c = Clock::default();
        assert!(!c.advance(100.0, true));
        assert_eq!(c.tick, 0);
        c.step();
        assert_eq!((c.tick, c.fraction, c.playing), (1, 0.0, false));
    }

    #[test]
    fn fastest_presents_each_view_as_it_arrives() {
        let mut c = Clock {
            playing: true,
            speed: FASTEST,
            ..Clock::default()
        };
        assert!(c.advance(0.001, true));
        assert!(!c.advance(0.001, false));
        assert!(c.sim_bound);
        assert_eq!(c.tick, 1);
    }
}

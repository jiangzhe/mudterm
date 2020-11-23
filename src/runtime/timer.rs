use crate::error::Result;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Timer {
    pub model: TimerModel,
    
}

#[derive(Debug, Clone)]
pub struct TimerModel {
    pub name: String,
    pub group: String,
    pub tick_time: Duration,
    pub enabled: bool,
    pub oneshot: bool,
}

// impl TimerModel {
//     pub fn compile(self) -> Result<Timer> {

//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instant_timer() {
        let inst = Instant::now() + Duration::from_secs(3);
        println!("{:?}", inst.checked_duration_since(Instant::now()));
        // std::thread::sleep(Duration::from_secs(1));
        // println!("{:?}", inst.saturating_duration_since(Instant::now())); 
        // println!("{:?}", Duration::from_secs(3) - Duration::from_secs(5));
    }
}
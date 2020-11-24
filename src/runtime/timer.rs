use std::time::{Duration, Instant};
use std::collections::{HashMap, BinaryHeap};
use std::cmp::{Ord, Ordering};
use uuid::Uuid;

#[derive(Debug)]
pub struct Timers {
    schedule: BinaryHeap<Timer>,
    models: HashMap<String, TimerModel>,
}

impl Timers {

    pub fn new() -> Self {
        Self{
            schedule: BinaryHeap::new(),
            models: HashMap::new(),
        }
    }

    pub fn insert(&mut self, tm: TimerModel) {
        if !tm.enabled {
            // 仅插入而不启动
            self.models.insert(tm.name.to_owned(), tm);
            return;
        }

        let (timer, tm) = tm.start();
        self.schedule.push(timer);
        self.models.insert(tm.name.to_owned(), tm);
    }

    pub fn enable(&mut self, name: &str, enabled: bool) {
        if let Some(tm) = self.models.get(name) {
            if tm.enabled == enabled {
                // 无需任何操作
                return;
            }
            if tm.enabled && !enabled {
                // 关闭已启动的定时器
                let tm = self.models.get_mut(name).unwrap();
                tm.enabled = false;
                tm.uuid.take();
                return;
            }
            if !tm.enabled && enabled {
                // 开启定时器
                let mut tm = self.models.remove(name).unwrap();
                tm.enabled = true;
                tm.uuid.take();
                self.insert(tm);
                return;
            }
        }
        // 查询不到，无需任何操作
    }

    pub fn remove(&mut self, name: &str) -> Option<TimerModel> {
        // 无需处理已调度的定时任务，在每次pop时将检验
        self.models.remove(name)
    }

    pub fn on_schedule(&mut self) -> Option<Timer> {
        let now = Instant::now();
        while let Some(earliest) = self.schedule.peek() {
            if earliest.schedule_time >= now {
                let timer = self.schedule.pop().unwrap();
                // 检查状态
                if let Some(tm) = self.models.get(&timer.name) {
                    if let Some(uuid) = tm.uuid {
                        if timer.uuid == uuid && tm.enabled {
                            // 只有在uuid相同且定时器被激活时，返回该
                            // 定时任务，否则直接丢弃
                            return Some(timer);
                        }
                    }
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct Timer {
    pub name: String,
    pub uuid: u128,
    tick_time: Duration,
    schedule_time: Instant,
}

impl Ord for Timer {
    fn cmp(&self, other: &Self) -> Ordering {
        other.schedule_time.cmp(&self.schedule_time)
    }
}

impl PartialOrd for Timer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Timer {
    fn eq(&self, other: &Self) -> bool {
        self.schedule_time == other.schedule_time
    }
}

impl Eq for Timer {}

#[derive(Debug, Clone)]
pub struct TimerModel {
    pub name: String,
    pub group: String,
    pub tick_time: Duration,
    pub enabled: bool,
    pub oneshot: bool,
    uuid: Option<u128>,
}

impl TimerModel {

    pub fn new(name: impl Into<String>, group: impl Into<String>, tick_time: Duration, enabled: bool, oneshot: bool) -> Self {
        Self{
            name: name.into(),
            group: group.into(),
            tick_time,
            enabled,
            oneshot,
            uuid: None,
        }
    }

    pub fn start(mut self) -> (Timer, TimerModel) {
        let next_time = Instant::now() + self.tick_time;
        // uuid将作为检验定时任务是否与当前定时器匹配的依据
        let uuid = Uuid::new_v4().as_u128();
        self.uuid.replace(uuid);
        let timer = Timer{
            name: self.name.to_owned(),
            uuid,
            tick_time: self.tick_time,
            schedule_time: next_time,
        };
        (timer, self)
    }
}

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
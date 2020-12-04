use crate::runtime::delay_queue::{Delay, DelayQueue, Delayed};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;
use bitflags::bitflags;

bitflags! {
    pub struct TimerFlags: u16 {
        const ENABLED = 0x0001;
        const ONESHOT = 0x0004;
    }
}

#[derive(Debug)]
pub struct Timers {
    schedule: DelayQueue<Delay<Timer>>,
    models: HashMap<String, TimerModel>,
}

impl Timers {
    pub fn new() -> Self {
        Self {
            schedule: DelayQueue::new(),
            models: HashMap::new(),
        }
    }

    /// 获取调度队列，调度队列为线程安全
    pub fn schedule(&self) -> DelayQueue<Delay<Timer>> {
        self.schedule.clone()
    }

    pub fn get(&self, name: impl AsRef<str>) -> Option<&TimerModel> {
        self.models.get(name.as_ref())
    }

    pub fn len(&self) -> usize {
        self.models.len()
    }

    pub fn insert(&mut self, tm: TimerModel) {
        if !tm.enabled() {
            // 仅插入而不启动
            self.models.insert(tm.name.to_owned(), tm);
            return;
        }
        self.insert_at(tm, Instant::now());
    }

    pub fn insert_at(&mut self, tm: TimerModel, start_time: Instant) {
        debug_assert!(tm.enabled());
        let (timer, tm) = tm.start_at(start_time);
        self.schedule.push(timer);
        self.models.insert(tm.name.to_owned(), tm);
    }

    pub fn is_enabled(&self, name: &str) -> bool {
        if let Some(tm) = self.models.get(name) {
            return tm.enabled();
        }
        false
    }

    pub fn enable(&mut self, name: &str, enabled: bool) {
        if let Some(tm) = self.models.get(name) {
            if tm.enabled() == enabled {
                // 无需任何操作
                return;
            }
            if tm.enabled() && !enabled {
                // 关闭已启动的定时器
                let tm = self.models.get_mut(name).unwrap();
                tm.set_enabled(false);
                tm.uuid.take();
                return;
            }
            if !tm.enabled() && enabled {
                // 开启定时器
                let mut tm = self.models.remove(name).unwrap();
                tm.set_enabled(true);
                tm.uuid.take();
                self.insert(tm);
                return;
            }
        }
        // 查询不到，无需任何操作
    }

    pub fn enable_group(&mut self, group: &str, enabled: bool) -> usize {
        let mut n = 0;
        for tm in self.models.values_mut() {
            if tm.group == group {
                n += 1;
                if !tm.enabled() && enabled {
                    // 从禁用变为启用，生成调度
                    tm.set_enabled(true);
                    let (new_timer, new_tm) = tm.clone().start_now();
                    self.schedule.push(new_timer);
                    *tm = new_tm;
                } else {
                    tm.set_enabled(enabled);
                }
            }
        }
        n
    }

    pub fn remove(&mut self, name: &str) -> Option<TimerModel> {
        // 无需处理已调度的定时任务，在每次pop时将检验
        self.models.remove(name)
    }

    pub fn finish(&mut self, task: Delay<Timer>) {
        if let Some(tm) = self.models.get(&task.value.name) {
            if let Some(uuid) = tm.uuid {
                if task.value.uuid == uuid {
                    // uuid相同表明定时任务与调度器一致，可进行后续处理
                    let (name, tm) = self.models.remove_entry(&task.value.name).unwrap();
                    if tm.oneshot() {
                        // 临时任务，直接退出
                        return;
                    }
                    if !tm.enabled() {
                        // 处于禁用状态，插入并退出
                        self.models.insert(name, tm);
                        return;
                    }
                    // 处于启用状态，开启下一次调度
                    let (next_timer, next_tm) = tm.start_at(task.delay_until());
                    self.schedule.push(next_timer);
                    self.models.insert(name, next_tm);
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Timer {
    pub name: String,
    pub uuid: u128,
    tick_time: Duration,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimerModel {
    pub name: String,
    pub group: String,
    pub tick_time: Duration,
    flags: TimerFlags,
    uuid: Option<u128>,
}

impl TimerModel {
    pub fn new(
        name: impl Into<String>,
        group: impl Into<String>,
        tick_time: Duration,
        flags: TimerFlags,
    ) -> Self {
        Self {
            name: name.into(),
            group: group.into(),
            tick_time,
            flags,
            uuid: None,
        }
    }

    pub fn enabled(&self) -> bool {
        self.flags.contains(TimerFlags::ENABLED)
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        if enabled {
            self.flags.insert(TimerFlags::ENABLED);
        } else {
            self.flags.remove(TimerFlags::ENABLED);
        }
    }

    pub fn oneshot(&self) -> bool {
        self.flags.contains(TimerFlags::ONESHOT)
    }

    pub fn set_oneshot(&mut self, oneshot: bool) {
        if oneshot {
            self.flags.insert(TimerFlags::ONESHOT);
        } else {
            self.flags.remove(TimerFlags::ONESHOT);
        }
    } 

    pub fn uuid(&self) -> Option<u128> {
        self.uuid
    }

    pub fn start_now(self) -> (Delay<Timer>, TimerModel) {
        self.start_at(Instant::now())
    }

    pub fn start_at(mut self, start_time: Instant) -> (Delay<Timer>, TimerModel) {
        let next_time = start_time + self.tick_time;
        // uuid将作为检验定时任务是否与当前定时器匹配的依据
        let uuid = Uuid::new_v4().as_u128();
        self.uuid.replace(uuid);
        let timer = Timer {
            name: self.name.to_owned(),
            uuid,
            tick_time: self.tick_time,
        };
        (Delay::until(timer, next_time), self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_timout() {
        let mut timers = Timers::new();
        let schedule = timers.schedule();
        timers.insert(TimerModel::new(
            "t1",
            "timer",
            Duration::from_millis(100),
            TimerFlags::ENABLED,
        ));
        assert!(schedule.pop_timeout(Duration::from_millis(50)).is_none());
        assert_eq!(
            "t1",
            &schedule
                .pop_timeout(Duration::from_millis(51))
                .unwrap()
                .value
                .name
        );
        assert!(timers.get("t1").is_some());
    }

    #[test]
    fn test_timer_oneshot() {
        let mut timers = Timers::new();
        let schedule = timers.schedule();
        timers.insert(TimerModel::new(
            "t2",
            "timer",
            Duration::from_millis(50),
            TimerFlags::ENABLED | TimerFlags::ONESHOT,
        ));
        let task = schedule.pop();
        timers.finish(task);
        assert_eq!(0, timers.len());
    }

    #[test]
    fn test_timer_repeat() {
        let mut timers = Timers::new();
        let schedule = timers.schedule();
        timers.insert(TimerModel::new(
            "t3",
            "timer",
            Duration::from_millis(10),
            TimerFlags::ENABLED,
        ));
        for _ in 0..10 {
            let task = schedule.pop_timeout(Duration::from_millis(11)).unwrap();
            timers.finish(task);
        }
    }

    #[test]
    fn test_timer_enable() {
        let mut timers = Timers::new();
        let schedule = timers.schedule();
        timers.insert(TimerModel::new(
            "t4",
            "timer",
            Duration::from_millis(10),
            TimerFlags::empty(),
        ));
        assert!(schedule.pop_timeout(Duration::from_millis(11)).is_none());
        timers.enable("t4", true);
        assert!(schedule.pop_timeout(Duration::from_millis(11)).is_some());
    }

    #[test]
    fn test_timer_disable() {
        let mut timers = Timers::new();
        let schedule = timers.schedule();
        timers.insert(TimerModel::new(
            "t5",
            "timer",
            Duration::from_millis(10),
            TimerFlags::ENABLED,
        ));
        timers.enable("t5", false);
        let task = schedule.pop_timeout(Duration::from_millis(11)).unwrap();
        assert!(!timers.is_enabled(&task.value.name));
        timers.finish(task);
        assert!(schedule.pop_timeout(Duration::from_millis(11)).is_none());
    }
}

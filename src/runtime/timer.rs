use std::time::{Duration, Instant};
use std::collections::HashMap;
use uuid::Uuid;
use crate::runtime::delay_queue::{DelayQueue, Delay};

#[derive(Debug)]
pub struct Timers {
    schedule: DelayQueue<Delay<Timer>>,
    models: HashMap<String, TimerModel>,
}

impl Timers {

    pub fn new() -> Self {
        Self{
            schedule: DelayQueue::new(),
            models: HashMap::new(),
        }
    }

    /// 获取调度队列，调度队列为线程安全
    pub fn schedule(&self) -> DelayQueue<Delay<Timer>> {
        self.schedule.clone()
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

    /// 提取调度任务，若无满足条件者，将阻塞当前线程
    pub fn pop_timeout(&mut self, duration: Duration) -> Option<Timer> {
        let deadline = Instant::now() + duration;
        while let Some(task) = self.schedule.pop_until(deadline) {
            let timer = task.value;
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
        None
    }
}

#[derive(Debug, Clone)]
pub struct Timer {
    pub name: String,
    pub uuid: u128,
    tick_time: Duration,
}

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

    pub fn start(mut self) -> (Delay<Timer>, TimerModel) {
        let next_time = Instant::now() + self.tick_time;
        // uuid将作为检验定时任务是否与当前定时器匹配的依据
        let uuid = Uuid::new_v4().as_u128();
        self.uuid.replace(uuid);
        let timer = Timer{
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
    fn test_instant_timer() {
        let inst = Instant::now() + Duration::from_secs(3);
        println!("{:?}", inst.checked_duration_since(Instant::now()));
        // std::thread::sleep(Duration::from_secs(1));
        // println!("{:?}", inst.saturating_duration_since(Instant::now())); 
        // println!("{:?}", Duration::from_secs(3) - Duration::from_secs(5));
    }
}
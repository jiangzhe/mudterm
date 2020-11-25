use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct DelayQueue<T> {
    data: Arc<Data<T>>,
}

impl<T: Delayed> DelayQueue<T> {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Data {
                queue: Mutex::new(BinaryHeap::new()),
                new_head: Condvar::new(),
            }),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Arc::new(Data {
                queue: Mutex::new(BinaryHeap::with_capacity(capacity)),
                new_head: Condvar::new(),
            }),
        }
    }

    pub fn pop(&self) -> T {
        let mut queue = self.data.queue.lock().unwrap();
        loop {
            let now = Instant::now();
            if let Some(head) = queue.peek() {
                if now >= head.delay_until() {
                    return queue.pop().unwrap();
                }
                let wait_time = head.delay_until().saturating_duration_since(now);
                // 尚未到达调度时间
                queue = self.data.new_head.wait_timeout(queue, wait_time).unwrap().0;
            } else {
                queue = self.data.new_head.wait(queue).unwrap();
            }
        }
    }

    pub fn pop_timeout(&self, timeout: Duration) -> Option<T> {
        let deadline = Instant::now() + timeout;
        self.pop_until(deadline)
    }

    pub fn pop_until(&self, deadline: Instant) -> Option<T> {
        let mut queue = self.data.queue.lock().unwrap();
        loop {
            let now = Instant::now();
            if let Some(head) = queue.peek() {
                if now >= head.delay_until() {
                    return queue.pop();
                }
                if now >= deadline {
                    return None;
                }
                let wait_time =
                    Instant::min(deadline, head.delay_until()).saturating_duration_since(now);
                queue = self.data.new_head.wait_timeout(queue, wait_time).unwrap().0;
            } else {
                if now >= deadline {
                    return None;
                }
                let wait_time = deadline.saturating_duration_since(now);
                queue = self.data.new_head.wait_timeout(queue, wait_time).unwrap().0;
            }
        }
    }

    pub fn push(&self, value: T) {
        let mut queue = self.data.queue.lock().unwrap();
        if let Some(head) = queue.peek() {
            if value.delay_until() < head.delay_until() {
                queue.push(value);
                self.data.new_head.notify_one();
                return;
            }
            queue.push(value);
            return;
        }
        queue.push(value);
        self.data.new_head.notify_one();
    }
}

#[derive(Debug)]
struct Data<T> {
    queue: Mutex<BinaryHeap<T>>,
    new_head: Condvar,
}

pub trait Delayed: Ord {
    fn delay_until(&self) -> Instant;
}

#[derive(Debug, Clone)]
pub struct Delay<T> {
    pub value: T,
    until: Instant,
}

impl<T> Delay<T> {
    pub fn until(value: T, until: Instant) -> Self {
        Self { value, until }
    }

    pub fn delay(value: T, duration: Duration) -> Self {
        Self {
            value,
            until: Instant::now() + duration,
        }
    }
}

impl<T> Delayed for Delay<T> {
    fn delay_until(&self) -> Instant {
        self.until
    }
}

impl<T> PartialEq for Delay<T> {
    fn eq(&self, other: &Self) -> bool {
        self.delay_until() == other.delay_until()
    }
}

impl<T> Eq for Delay<T> {}

impl<T> Ord for Delay<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        other.delay_until().cmp(&self.delay_until())
    }
}

impl<T> PartialOrd for Delay<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_delay_queue_pop() {
        let queue = DelayQueue::new();
        queue.push(Delay::until(
            100,
            Instant::now() + Duration::from_millis(100),
        ));
        queue.push(Delay::until(50, Instant::now() + Duration::from_millis(50)));
        assert_eq!(50, queue.pop().value);
        assert_eq!(100, queue.pop().value);
    }

    #[test]
    fn test_delay_queue_timeout() {
        let queue = DelayQueue::new();
        queue.push(Delay::until(
            500,
            Instant::now() + Duration::from_millis(500),
        ));
        queue.push(Delay::until(50, Instant::now() + Duration::from_millis(50)));
        assert_eq!(50, queue.pop().value);
        assert!(queue.pop_timeout(Duration::from_millis(200)).is_none());
    }
}

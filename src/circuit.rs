use std::fmt::Debug;
use std::hash::Hash;
use std::slice::SliceIndex;
use std::sync::atomic::{AtomicU64, Ordering};

pub type Tick = u64;
pub type Ticks = u64;

#[derive(Debug)]
pub enum RunResult {
    Finished { after_ticks: Ticks },
    ReachedMaxTicks { max_ticks: Ticks },
}

pub trait Circuit {
    type NodeId: Clone + Copy + Eq + Hash;

    fn new() -> Self;

    fn update(&mut self);
    fn work_left(&self) -> bool;

    fn wire(&mut self) -> Self::NodeId;
    fn inverter(&mut self) -> Self::NodeId;
    fn trigger(&mut self) -> Self::NodeId;

    fn connect(&mut self, input: Self::NodeId, output: Self::NodeId);
    fn trigger_node(&mut self, node_id: Self::NodeId, val: bool);

    fn is_active(&self, node_id: Self::NodeId) -> bool;

    fn run(&mut self, max_ticks: Ticks) -> RunResult {
        for ticks in 0..max_ticks {
            if self.work_left() {
                self.update();
            } else {
                return RunResult::Finished { after_ticks: ticks };
            };
        }
        RunResult::ReachedMaxTicks { max_ticks }
    }
    fn run_until_done(&mut self) {
        while self.work_left() {
            self.update();
        }
    }
}

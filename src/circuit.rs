use std::fmt::Debug;
use std::hash::Hash;
use std::slice::SliceIndex;
use std::sync::atomic::{AtomicU64, Ordering};

pub type Tick = u64;
pub type Ticks = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(u64);

impl NodeId {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(NEXT.fetch_add(1, Ordering::SeqCst))
    }
}

impl Into<usize> for NodeId {
    fn into(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug)]
pub enum RunResult {
    Finished { after_ticks: Ticks },
    ReachedMaxTicks { max_ticks: Ticks },
}

pub trait Circuit {
    fn new() -> Self;

    fn update(&mut self);
    fn work_left(&self) -> bool;

    fn wire(&mut self) -> NodeId;
    fn inverter(&mut self) -> NodeId;
    fn trigger(&mut self) -> NodeId;

    fn connect(&mut self, input: NodeId, output: NodeId);
    fn trigger_node(&mut self, node_id: NodeId, val: bool);

    fn is_active(&self, node_id: NodeId) -> bool;

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

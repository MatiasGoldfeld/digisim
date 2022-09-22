use std::fmt::Debug;
use std::hash::Hash;

pub type Tick = u64;
pub type Ticks = u64;

#[derive(Debug)]
pub enum RunResult {
    Finished { after_ticks: Ticks },
    ReachedMaxTicks { max_ticks: Ticks },
}

pub trait Circuit {
    type NodeId: Clone + Copy + Eq + Hash + From<Self::InputId>;
    type InputId: Clone + Copy + Eq + Hash;

    fn new() -> Self;
    fn tick(&self) -> Tick;

    fn update(&mut self);
    fn work_left(&self) -> bool;

    fn or(&mut self) -> Self::NodeId;
    fn nor(&mut self) -> Self::NodeId;
    fn and(&mut self) -> Self::NodeId;
    fn nand(&mut self) -> Self::NodeId;
    fn xor(&mut self) -> Self::NodeId;
    fn xnor(&mut self) -> Self::NodeId;

    fn input(&mut self) -> Self::InputId;
    fn set_input(&mut self, node_id: Self::InputId, val: bool);

    fn connect(&mut self, input: Self::NodeId, output: Self::NodeId);

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

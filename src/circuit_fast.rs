use std::{
    fmt::Debug,
    num::Wrapping,
    ops::{Index, IndexMut},
    sync::atomic::{AtomicU32, Ordering},
};

use crate::circuit::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(u32);

impl NodeId {
    const NULL: Self = NodeId(u32::MAX);
}

impl Default for NodeId {
    fn default() -> Self {
        Self::NULL
    }
}

#[derive(Debug, Default)]
struct NodeIdBuilder {
    next: AtomicU32,
    unused: Vec<NodeId>,
}

impl NodeIdBuilder {
    fn get_id(&mut self) -> NodeId {
        let node_id = match self.unused.pop() {
            Some(node_id) => node_id,
            None => NodeId(self.next.fetch_add(1, Ordering::SeqCst)),
        };
        if node_id == NodeId::NULL {
            panic!("Reached null node id");
        }
        node_id
    }

    fn _destroy_id(&mut self, node_id: NodeId) {
        self.unused.push(node_id);
    }
}

impl<T> Index<NodeId> for Vec<T> {
    type Output = T;

    fn index(&self, index: NodeId) -> &Self::Output {
        unsafe { self.get_unchecked(index.0 as usize) }
    }
}

impl<T: Clone + Default> IndexMut<NodeId> for Vec<T> {
    fn index_mut(&mut self, index: NodeId) -> &mut Self::Output {
        // if index >= self.len() {
        //     self.resize(index + 1, T::default())
        // }
        unsafe { self.get_unchecked_mut(index.0 as usize) }
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
enum GateType {
    #[default]
    OrNor,
    AndNand,
    XorXnor,
}

// #[repr(align(8))]
#[derive(Clone, Debug, Default)]
struct NodeData {
    next_update: NodeId, // Modified in change, read in update

    // Technically not necessary to store, but perhaps caching it is good?
    // Read in both phases, modified in change
    output: bool,
    inputs: Wrapping<u8>, // Modified in change
    inverted: bool,       // Read all over
    gate_type: GateType,  // Read in change
}

// Separated from [NodeData] because this is the data that is accessed and
// written to in other nodes when an update is occuring. This ensures better
// locality and gives a modest performance boost.
#[derive(Clone, Debug, Default)]
struct UpdateData {
    next_changed: NodeId,
    inputs_delta: Wrapping<u8>,
}

#[derive(Default, Debug)]
pub struct CircuitFast {
    tick: Tick,
    node_id_builder: NodeIdBuilder,
    node_children: Vec<Vec<NodeId>>,
    node_data: Vec<NodeData>,
    node_update_data: Vec<UpdateData>,
    update_head: NodeId,
    changed_head: NodeId,
}

macro_rules! enqueue {
    ( $head:expr, $node_next:expr, $node_id:ident ) => {{
        let node_next = &mut $node_next;
        if *node_next == NodeId::NULL {
            *node_next = $head;
            $head = $node_id;
        }
    }};
}

impl CircuitFast {
    // Can also make this function not take [&mut self] like [modify] but for
    // some reason that hurts performance by a noticeable amount.
    //
    // Actually it might just be changing the [next_*] ordering in [update]
    // is causing the slow-down. I have no clue why though...
    fn enqueue_update(&mut self, node_id: NodeId) {
        enqueue!(
            self.update_head,
            self.node_data[node_id].next_update,
            node_id
        );
    }

    fn modify(
        node_update_data: &mut Vec<UpdateData>,
        changed_head: &mut NodeId,
        node_id: NodeId,
        increment: bool,
    ) {
        let update_data = &mut node_update_data[node_id];
        if increment {
            update_data.inputs_delta += 1;
        } else {
            update_data.inputs_delta -= 1;
        }
        enqueue!(*changed_head, update_data.next_changed, node_id);
    }

    fn add_node(&mut self, gate_type: GateType, inverted: bool) -> NodeId {
        let node_id = self.node_id_builder.get_id();
        let index = node_id.0 as usize;
        if index >= self.node_data.len() {
            self.node_children.resize(index + 1, Vec::new());
            self.node_data.resize(index + 1, NodeData::default());
            self.node_update_data
                .resize(index + 1, UpdateData::default());
        }
        self.node_data[index].inverted = inverted;
        self.node_data[index].output = inverted;
        self.node_data[index].gate_type = gate_type;
        node_id
    }
}

impl Circuit for CircuitFast {
    type NodeId = NodeId;
    type InputId = NodeId;

    fn new() -> Self {
        Self::default()
    }

    fn tick(&self) -> Tick {
        self.tick
    }

    fn update(&mut self) {
        let mut node_id = self.update_head;
        self.update_head = NodeId::NULL;
        while node_id != NodeId::NULL {
            let node_data = &mut self.node_data[node_id];
            let node_output = node_data.output;
            let next_update = node_data.next_update;
            node_data.next_update = NodeId::NULL;
            for child in self.node_children[node_id].iter().cloned() {
                Self::modify(
                    &mut self.node_update_data,
                    &mut self.changed_head,
                    child,
                    node_output,
                );
            }
            node_id = next_update;
        }

        let mut node_id = self.changed_head;
        self.changed_head = NodeId::NULL;
        while node_id != NodeId::NULL {
            let node_update_data = &mut self.node_update_data[node_id];
            let next_changed = node_update_data.next_changed;
            node_update_data.next_changed = NodeId::NULL;
            if node_update_data.inputs_delta.0 != 0 {
                let node_data = &mut self.node_data[node_id];
                match node_data.gate_type {
                    GateType::OrNor | GateType::AndNand => {
                        node_data.inputs += node_update_data.inputs_delta
                    }
                    GateType::XorXnor => node_data.inputs ^= node_update_data.inputs_delta.0 & 1,
                }
                node_update_data.inputs_delta = Wrapping(0);
                let new_output = node_data.inverted ^ (node_data.inputs.0 != 0);
                if node_data.output != new_output {
                    node_data.output = new_output;
                    self.enqueue_update(node_id);
                }
            }
            node_id = next_changed;
        }

        self.tick += 1;
    }

    fn work_left(&self) -> bool {
        self.update_head != NodeId::NULL || self.changed_head != NodeId::NULL
    }

    fn or(&mut self) -> NodeId {
        self.add_node(GateType::OrNor, false)
    }

    fn nor(&mut self) -> NodeId {
        self.add_node(GateType::OrNor, true)
    }

    fn and(&mut self) -> NodeId {
        self.add_node(GateType::AndNand, true)
    }

    fn nand(&mut self) -> NodeId {
        self.add_node(GateType::AndNand, false)
    }

    fn xor(&mut self) -> NodeId {
        self.add_node(GateType::XorXnor, false)
    }

    fn xnor(&mut self) -> NodeId {
        self.add_node(GateType::XorXnor, true)
    }

    fn input(&mut self) -> NodeId {
        self.add_node(GateType::OrNor, false)
    }

    fn set_input(&mut self, node_id: NodeId, val: bool) {
        let output = &mut self.node_data[node_id].output;
        if *output != val {
            *output = val;
            self.enqueue_update(node_id);
        }
    }

    fn connect(&mut self, input: NodeId, output: NodeId) {
        self.node_children[input].push(output);
        let is_and_nand = match self.node_data[output].gate_type {
            GateType::OrNor | GateType::XorXnor => false,
            GateType::AndNand => true,
        };
        if self.is_active(input) ^ is_and_nand {
            Self::modify(
                &mut self.node_update_data,
                &mut self.changed_head,
                output,
                !is_and_nand,
            );
        }
    }

    fn is_active(&self, node_id: NodeId) -> bool {
        self.node_data[node_id].output
    }
}

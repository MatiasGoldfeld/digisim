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
    // next_changed: NodeId, // Modified in update, read in change

    // Technically not necessary to store, but perhaps caching it is good?
    // Read in both phases, modified in change
    output: bool,
    inputs: Wrapping<u8>, // Modified in change
    // inputs_delta: Wrapping<u8>, // Modified in update, cleared in change
    inverted: bool,      // Read all over
    gate_type: GateType, // Read in change
}

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

impl CircuitFast {
    fn enqueue_update(&mut self, node_id: NodeId) {
        let node_data = &mut self.node_data[node_id];
        if node_data.next_update == NodeId::NULL {
            node_data.next_update = self.update_head;
            self.update_head = node_id;
        }
    }

    fn mark_changed(&mut self, node_id: NodeId) {
        // let node_data = &mut self.node_data[node_id];
        let next_changed = &mut self.node_update_data[node_id].next_changed;
        if *next_changed == NodeId::NULL {
            *next_changed = self.changed_head;
            self.changed_head = node_id;
        }
    }

    // Returns true if value changed
    fn modify(&mut self, node_id: NodeId, increment: bool) {
        // let node_data = &mut self.node_data[node_id];
        let inputs_delta = &mut self.node_update_data[node_id].inputs_delta;
        if increment {
            *inputs_delta += 1;
        } else {
            *inputs_delta -= 1;
        }
        self.mark_changed(node_id);
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
            let danger: *const Self = self; // TODO: Undanger this
            for child in unsafe { &(*danger).node_children[node_id] } {
                self.modify(*child, node_output);
            }
            node_id = next_update;
        }

        let mut node_id = self.changed_head;
        self.changed_head = NodeId::NULL;
        while node_id != NodeId::NULL {
            let node_data = &mut self.node_data[node_id];
            let node_update_data = &mut self.node_update_data[node_id];
            match node_data.gate_type {
                GateType::OrNor | GateType::AndNand => {
                    node_data.inputs += node_update_data.inputs_delta
                }
                GateType::XorXnor => node_data.inputs ^= node_update_data.inputs_delta.0 & 1,
            }
            node_update_data.inputs_delta = Wrapping(0);
            let new_output = node_data.inverted ^ (node_data.inputs.0 != 0);
            let next_changed = node_update_data.next_changed;
            node_update_data.next_changed = NodeId::NULL;
            if node_data.output != new_output {
                node_data.output = new_output;
                self.enqueue_update(node_id);
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
        self.node_data[node_id].inputs = if val { Wrapping(1) } else { Wrapping(0) };
        self.mark_changed(node_id); // or enqueue?
    }

    fn connect(&mut self, input: NodeId, output: NodeId) {
        self.node_children[input].push(output);
        let is_and_nand = match self.node_data[output].gate_type {
            GateType::OrNor | GateType::XorXnor => false,
            GateType::AndNand => true,
        };
        if self.is_active(input) ^ is_and_nand {
            self.modify(output, !is_and_nand);
        }
    }

    fn is_active(&self, node_id: NodeId) -> bool {
        self.node_data[node_id].output
    }
}

use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    num::Wrapping,
    ops::{Index, IndexMut},
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
    },
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

    fn destroy_id(&mut self, node_id: NodeId) {
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

enum ValueChange {
    None,
    LowToHigh,
    HighToLow,
}

// #[repr(align(8))]
#[derive(Clone, Debug, Default)]
struct NodeData {
    next_update: NodeId,
    next_changed: NodeId,
    output: bool,
    inputs: Wrapping<u8>,
    inputs_delta: Wrapping<u8>,
    inverted: bool,
}

#[derive(Default, Debug)]
pub struct CircuitFast {
    tick: Tick,
    node_id_builder: NodeIdBuilder,
    node_children: Vec<Vec<NodeId>>,
    node_data: Vec<NodeData>,
    update_head: NodeId,
    changed_head: NodeId,
}

impl CircuitFast {
    fn get(&self, node_id: NodeId) -> bool {
        self.node_data[node_id].output
    }

    fn enqueue_update(&mut self, node_id: NodeId) {
        let node_data = &mut self.node_data[node_id];
        if node_data.next_update == NodeId::NULL {
            node_data.next_update = self.update_head;
            self.update_head = node_id;
        }
    }

    fn mark_changed(&mut self, node_id: NodeId) {
        let node_data = &mut self.node_data[node_id];
        if node_data.next_changed == NodeId::NULL {
            node_data.next_changed = self.changed_head;
            self.changed_head = node_id;
        }
    }

    // Only safe to do on node with no inputs
    fn set(&mut self, node_id: NodeId, val: bool) {
        self.node_data[node_id].inputs = if val { Wrapping(1) } else { Wrapping(0) };
        self.mark_changed(node_id); // or enqueue?
                                    // self.next.insert(node_id);
    }

    // Returns true if value changed
    fn modify(&mut self, node_id: NodeId, increment: bool) {
        let node_data = &mut self.node_data[node_id];
        if increment {
            node_data.inputs_delta += 1;
        } else {
            node_data.inputs_delta -= 1;
        }
        self.mark_changed(node_id);
    }

    fn add_node(&mut self, inverted: bool) -> NodeId {
        let node_id = self.node_id_builder.get_id();
        let index = node_id.0 as usize;
        if index >= self.node_data.len() {
            self.node_children.resize(index + 1, Vec::new());
            self.node_data.resize(index + 1, NodeData::default());
        }
        self.node_data[index].inverted = inverted;
        self.node_data[index].output = inverted;
        node_id
    }
}

impl Circuit for CircuitFast {
    type NodeId = NodeId;

    fn new() -> Self {
        Self::default()
    }

    fn update(&mut self) {
        let mut node_id = self.update_head;
        self.update_head = NodeId::NULL;
        while node_id != NodeId::NULL {
            let node_data = &mut self.node_data[node_id];
            let node_output = node_data.output;
            let next_update = node_data.next_update;
            node_data.next_update = NodeId::NULL;
            for child in self.node_children[node_id].clone() {
                self.modify(child, node_output);
            }
            node_id = next_update;
        }

        let mut node_id = self.changed_head;
        self.changed_head = NodeId::NULL;
        while node_id != NodeId::NULL {
            let node_data = &mut self.node_data[node_id];
            node_data.inputs += node_data.inputs_delta;
            node_data.inputs_delta = Wrapping(0);
            let new_output = node_data.inverted ^ (node_data.inputs.0 != 0);
            let next_changed = node_data.next_changed;
            node_data.next_changed = NodeId::NULL;
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

    fn wire(&mut self) -> NodeId {
        self.add_node(false)
    }

    fn inverter(&mut self) -> NodeId {
        self.add_node(true)
    }

    fn trigger(&mut self) -> NodeId {
        self.add_node(false)
    }

    fn connect(&mut self, input: NodeId, output: NodeId) {
        self.node_children[input].push(output);
        if self.get(input) {
            self.modify(output, true);
        }
    }

    fn trigger_node(&mut self, node_id: NodeId, val: bool) {
        self.set(node_id, val);
    }

    fn is_active(&self, node_id: NodeId) -> bool {
        self.get(node_id)
    }
}

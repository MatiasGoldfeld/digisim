pub mod circuit_builder;
pub mod circuit_sim;
pub mod components;

mod circuit;
pub use circuit::Circuit;
pub type NodeId = <Circuit as circuit_sim::CircuitSim>::NodeId;
pub type InputId = <Circuit as circuit_sim::CircuitSim>::InputId;

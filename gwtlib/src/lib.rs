use std::collections::{HashMap, hash_map::Entry};
use std::fmt::Display;
use std::rc::Rc;

use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeIndexable;
use petgraph::Graph;
use petgraph::Directed;

#[derive(Eq,Hash,PartialEq)]
enum NodeWeight{
    Place(Rc<str>),
    Transition(Transition),
}

#[derive(Eq,Hash,PartialEq, Clone)]
enum Transition{
    Named(Rc<str>),
    Empty(usize)
}

impl Display for NodeWeight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            NodeWeight::Place(name) => name,
            NodeWeight::Transition(Transition::Named(name)) => name,
            NodeWeight::Transition(Transition::Empty(_)) => "",
        };
        write!(f, "{name}")
    }    
}

type EdgeWeight = &'static str;

pub struct Model{
    graph: Graph<NodeWeight, EdgeWeight, Directed>,
    place_table: HashMap<Rc<str>, NodeIndex>,
    transition_table: HashMap<Transition, NodeIndex>,
    empty_transition: usize,
}


impl Model{
    pub fn new() -> Self{
        Self{
            graph: Graph::new(),
            place_table: HashMap::new(),
            transition_table: HashMap::new(),
            empty_transition: 0,
        }
    }

    fn add_place(&mut self, place_name: &str) -> NodeIndex{
        let node_name: Rc<str> = place_name.into();
        match self.place_table.entry(node_name.clone()){
            Entry::Occupied(e) => {
                *e.get()
            },
            Entry::Vacant(e) => {
                let node = NodeWeight::Place(node_name);
                let value = self.graph.add_node(node);
                e.insert(value);
                value
            }
        }
    }
    fn add_transition(&mut self, transition: &Transition) -> NodeIndex{
        match self.transition_table.entry(transition.clone()){
            Entry::Occupied(e) => {
                *e.get()
            },
            Entry::Vacant(e) => {
                let node = NodeWeight::Transition(transition.clone());
                let value = self.graph.add_node(node);
                e.insert(value);
                value
            }
        }
    }

    fn connect_to_transition(&mut self, given: &str, when: &Transition){
        let from = self.add_place(given);
        let to = self.add_transition(when);
        self.graph.add_edge(from, to, "");
    }

    fn connect_to_place(&mut self, when: &Transition, then: &str){
        let from = self.add_transition(when);
        let to = self.add_place(then);
        self.graph.add_edge(from, to, "");
    }
    pub fn add_step(&mut self, given: &[String], when: &str, then: &[String]){
        let when = if when.is_empty(){
            self.empty_transition += 1;
            Transition::Empty(self.empty_transition)
        } else {
            Transition::Named(when.into())
        };
        for g in given {
             self.connect_to_transition(g, &when);
        }
        for t in then {
             self.connect_to_place(&when, t);
        }
    }

    fn get_node(&self, node_name: &str) -> Option<NodeIndex> {
        self.place_table.get(node_name).copied()
    }

}

impl Default for Model {
    fn default() -> Self {
        Self::new()
    }
}

fn get_node_attr(_g: &Graph<NodeWeight, EdgeWeight, Directed>, (_, node): (NodeIndex, &NodeWeight)) -> String{
    match node {
        NodeWeight::Place(_name) => "".to_string(),
        NodeWeight::Transition(_transition) =>
        {   
            format!("shape=box,style=filled,fillcolor=black,fontcolor=white")
        },
    }
}
fn get_edge_attr(_g: &Graph<NodeWeight, EdgeWeight, Directed>, _: petgraph::graph::EdgeReference<'_, &str>) -> String{
    "".to_string()
}

fn split_label(s:&str, width: usize) -> String {
    let mut label = String::with_capacity(s.len());
    let mut char_count = 0;
    for word in s.split_whitespace() {
        char_count += word.chars().count() + 1;
        label.push_str(word);
        if char_count > width {
            label.push('\n');
            char_count =0;
        } else {
            label.push(' ');
        }
    }
    label.pop();
    label
}

impl Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use graphviz_rust::dot_generator::*;
        use graphviz_rust::dot_structures::*;
        use graphviz_rust::printer::{DotPrinter, PrinterContext};
        let mut dot = graph!(di id!("petri");
                        attr!("rankdir", "TB"),
                        attr!("fontsize", 5),
                        attr!("fontname", esc "Hack Nerd Font")
            );
        let mut transition_sg = subgraph!("transitions");
        transition_sg.add_stmt(stmt!(GraphAttributes::Node(vec![
                        attr!("height", 0.01),
                        attr!("shape", "box"),
                        attr!("style", "filled"),
                        attr!("fillcolor", "black"),
                        attr!("fontname", esc "Hack Nerd Font"),
                        attr!("fontsize", 5),
                        attr!("fontcolor", "white"),
                        attr!("color", "black"),
         ])));
        for (transition, node_id) in self.transition_table.iter() {
            let node_id = node_id.index();
            let lbl = match transition{
                Transition::Named(name) => &name,
                Transition::Empty(_n) => "",
            };
            transition_sg.add_stmt(stmt!(node!(node_id; attr!("label", esc split_label(lbl, 15)))));
        }

        dot.add_stmt(stmt!(transition_sg));
        let mut place_sg = subgraph!("places");
        place_sg.add_stmt(stmt!(GraphAttributes::Node(vec![
                        attr!("fontname", esc "Hack Nerd Font"),
                        attr!("height", 0.1),
                        attr!("shape", "oval"),
                        attr!("fontsize", 8),
                        attr!("color", "black"),
         ])));
        for (place, node_id) in self.place_table.iter() {
            let node_id = node_id.index();
            place_sg.add_stmt(stmt!(node!(node_id; attr!("label", esc split_label(place, 15)))));
        }
        dot.add_stmt(stmt!(place_sg));

        for edge in self.graph.raw_edges(){
            let source = edge.source().index();
            let target  = edge.target().index();
            dot.add_stmt(stmt!(edge!(node_id!(source) => node_id!(target))));
        }

        write!(f, "{}", dot.print(&mut PrinterContext::default()))
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_model() {
        let mut model = Model::new();
        let w = Transition::Named("w".into());
        model.connect_to_transition("A", &w);
        model.connect_to_place(&w, "B");
        let a = model.get_node("A");
        let b = model.get_node("B");
    }
}

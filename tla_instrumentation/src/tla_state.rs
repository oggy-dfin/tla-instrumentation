use crate::tla_value::TlaValue;
use candid::CandidType;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fmt::{Display, Formatter},
};

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, CandidType, Deserialize)]
pub struct VarAssignment(pub BTreeMap<String, TlaValue>);

impl VarAssignment {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn update(&mut self, locals: Vec<(String, TlaValue)>) {
        self.0.extend(locals)
    }

    pub fn add(&mut self, name: &str, value: TlaValue) {
        self.0.insert(name.to_string(), value);
    }

    pub fn merge(&self, other: VarAssignment) -> VarAssignment {
        assert!(
            self.0
                .keys()
                .collect::<BTreeSet<_>>()
                .is_disjoint(&other.0.keys().collect()),
            "The states have non-disjoint sets of keys"
        );
        let mut new_locals = self.0.clone();
        new_locals.extend(other.0);
        VarAssignment(new_locals)
    }
}

#[derive(Debug)]
pub struct GlobalState(pub VarAssignment);

impl GlobalState {
    pub fn new() -> Self {
        Self(VarAssignment::new())
    }
}

#[derive(Clone, Debug)]
pub struct Label(String);

impl Label {
    pub fn new(name: &str) -> Self {
        Self(name.to_string())
    }

    pub fn merge(&self, other: &Label) -> Label {
        Label(format!("{}_{}", self.0, other.0))
    }
}

#[derive(Debug)]
pub struct LocalState {
    pub locals: VarAssignment,
    pub label: Label,
}

#[derive(Debug)]
pub struct Destination(String);

impl Display for Destination {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Destination {
    pub fn new(name: &str) -> Self {
        Self(name.to_string())
    }
}

#[derive(Debug)]
pub struct RequestBuffer {
    pub to: Destination,
    pub message: TlaValue,
}

#[derive(Debug)]
pub struct ResponseBuffer {
    pub from: Destination,
    pub message: TlaValue,
}

#[derive(Debug)]
pub struct StartState {
    pub global: GlobalState,
    pub local: LocalState,
    pub responses: Vec<ResponseBuffer>,
}

#[derive(Debug)]
pub struct EndState {
    pub global: GlobalState,
    pub local: LocalState,
    pub requests: Vec<RequestBuffer>,
}

#[derive(Debug)]
pub struct StatePair {
    pub start: StartState,
    pub end: EndState,
}

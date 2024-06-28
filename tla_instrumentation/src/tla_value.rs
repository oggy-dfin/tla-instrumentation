use candid::{CandidType, Nat, Principal};
use itertools::join;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::{
    fmt,
    fmt::{Display, Formatter},
};

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, CandidType, Deserialize)]
pub enum TlaValue {
    Set(BTreeSet<TlaValue>),
    Record(BTreeMap<String, TlaValue>),
    Function(BTreeMap<TlaValue, TlaValue>),
    Seq(Vec<TlaValue>),
    Literal(String),
    Constant(String),
    Bool(bool),
    Int(Nat),
    Variant { tag: String, value: Box<TlaValue> },
}

impl Display for TlaValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TlaValue::Set(set) => {
                let elements = set.iter().map(|x| format!("{}", x));
                write!(f, "{{{}}}", join(elements, ", "))
            }
            TlaValue::Record(map) => {
                let elements = map.iter().map(|(k, v)| format!("{} |-> {}", k, v));
                write!(f, "[{}]", join(elements, ", "))
            }
            TlaValue::Function(map) => {
                let elements = map.iter().map(|(k, v)| format!("{} :> {}", k, v));
                write!(f, "{}", join(elements, " @@ "))
            }
            TlaValue::Seq(vec) => {
                let elements = vec.iter().map(|x| format!("{}", x));
                write!(f, "<<{}>>", join(elements, ", "))
            }
            TlaValue::Literal(s) => write!(f, "\"{}\"", s),
            TlaValue::Constant(s) => write!(f, "{}", s),
            TlaValue::Bool(b) => write!(f, "{}", if *b { "TRUE" } else { "FALSE" }),
            // Candid likes to pretty print its numbers
            TlaValue::Int(i) => write!(f, "{}", format!("{}", i).replace("_", "")),
            TlaValue::Variant { tag, value } => write!(f, "Variant(\"{}\", {})", tag, value),
        }
    }
}

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord, CandidType, Deserialize)]
pub struct TlaConstantAssignment {
    pub constants: BTreeMap<String, TlaValue>,
}

impl TlaConstantAssignment {
    pub fn to_map(&self) -> HashMap<String, String> {
        self.constants
            .iter()
            .map(|(k, v)| (k.clone(), v.to_string()))
            .collect()
    }
}

pub trait ToTla {
    fn to_tla_value(&self) -> TlaValue;
}

impl ToTla for Principal {
    fn to_tla_value(&self) -> TlaValue {
        TlaValue::Literal(self.to_string())
    }
}

impl ToTla for u32 {
    fn to_tla_value(&self) -> TlaValue {
        TlaValue::Int((*self).into())
    }
}

impl ToTla for u64 {
    fn to_tla_value(&self) -> TlaValue {
        TlaValue::Int((*self).into())
    }
}

impl<K: ToTla, V: ToTla> ToTla for BTreeMap<K, V> {
    fn to_tla_value(&self) -> TlaValue {
        TlaValue::Function(
            self.iter()
                .map(|(k, v)| (k.to_tla_value(), v.to_tla_value()))
                .collect(),
        )
    }
}

impl<V: ToTla> ToTla for BTreeSet<V> {
    fn to_tla_value(&self) -> TlaValue {
        TlaValue::Set(self.iter().map(|v| v.to_tla_value()).collect())
    }
}

impl<V: ToTla> ToTla for Vec<V> {
    fn to_tla_value(&self) -> TlaValue {
        TlaValue::Seq(self.iter().map(|v| v.to_tla_value()).collect())
    }
}

/*
impl<V: ToTla> ToTla for Vec<&V> {
    fn to_tla_value(&self) -> TlaValue {
        TlaValue::Seq(self.iter().map(|v| v.to_tla_value()).collect())
    }
}
 */

impl ToTla for &str {
    fn to_tla_value(&self) -> TlaValue {
        TlaValue::Literal(self.to_string())
    }
}

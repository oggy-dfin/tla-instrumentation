pub mod tla_value;
use std::{cell::RefCell, collections::BTreeMap, mem};

use tla_value::*;

#[derive(Clone, Debug)]
pub struct VarAssignment(pub BTreeMap<String, TlaValue>);

impl VarAssignment {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn update(&self, locals: Vec<(String, TlaValue)>) -> VarAssignment {
        let mut new_locals = self.0.clone();
        new_locals.extend(locals);
        VarAssignment(new_locals)
    }

    pub fn add(&mut self, name: &str, value: TlaValue) {
        self.0.insert(name.to_string(), value);
    }

    pub fn merge(&self, other: VarAssignment) -> VarAssignment {
        let mut new_locals = self.0.clone();
        new_locals.extend(other.0);
        VarAssignment(new_locals)
    }
}

#[derive(Clone, Debug)]
pub struct Label(String);

impl Label {
    pub fn new(name: &str) -> Self {
        Self(name.to_string())
    }

    fn merge(&self, other: &Label) -> Label {
        Label(format!("{}_{}", self.0, other.0))
    }
}

#[derive(Debug)]
pub struct Function {
    // TODO: do we want checks that only declared variables are set?
    // vars: BTreeSet<String>,
    pub default_start_locals: VarAssignment,
    pub default_end_locals: VarAssignment,
    // Only for top-level methods
    pub start_end_labels: Option<(Label, Label)>,
    // TODO: do we want checks that all labels come from an allowed set?
    // labels: BTreeSet<Label>,
}

#[derive(Debug)]
struct StackElem {
    function: Function,
    label: Option<Label>,
    locals: VarAssignment,
}

#[derive(Debug)]
struct CallStack(Vec<StackElem>);

#[derive(Debug)]
pub struct LocalState {
    pub locals: VarAssignment,
    pub label: Label,
}

impl CallStack {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn call_function(&mut self, function: Function) {
        assert!(
            function.start_end_labels.is_none() || self.0.is_empty(),
            "Calling a top-level function"
        );
        let locals = function.default_start_locals.clone();
        let label = function.start_end_labels.clone().map(|(start, _)| start);
        self.0.push(StackElem {
            function,
            label,
            locals,
        });
    }

    fn return_from_function(&mut self) -> Option<LocalState> {
        let f = self.0.pop().expect("No function in call stack");
        match f.function.start_end_labels {
            Some((_, end)) => {
                let locals = f.function.default_end_locals.clone();
                // TODO: do we really want to overwrite all the current values? I guess it's fine
                Some(LocalState { locals, label: end })
            }
            None => None,
        }
    }

    fn get_state(&self) -> LocalState {
        let elem = self.0.first().expect("No function in call stack");
        let mut state = LocalState {
            locals: elem.locals.clone(),
            label: elem
                .label
                .clone()
                .expect("No label at the start of the call stack"),
        };
        self.0.iter().skip(1).for_each(|elem| {
            state.locals = state.locals.merge(elem.locals.clone());
            state.label = state.label.merge(
                elem.label
                    .as_ref()
                    .expect("No label in the middle of the call stack"),
            );
        });
        state
    }

    // TODO: handle passing &mut locals to called functions somehow; what if they're called differently?
    fn log_locals(&mut self, locals: VarAssignment) {
        let elem = self.0.last_mut().expect("No function in call stack");
        elem.locals = elem.locals.merge(locals);
    }
}

#[derive(Debug)]
pub struct GlobalState(pub VarAssignment);

impl GlobalState {
    pub fn new() -> Self {
        Self(VarAssignment::new())
    }
}

#[derive(Debug)]
pub struct Destination(String);

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

#[derive(Debug)]
enum Stage {
    Start,
    End(StartState),
}

#[derive(Debug)]
pub struct InstrumentationState {
    call_stack: CallStack,
    stage: Stage,
}

impl InstrumentationState {
    pub fn new() -> Self {
        Self {
            call_stack: CallStack::new(),
            stage: Stage::Start,
        }
    }
}

thread_local! {
    static TLA_CALL_STACK: RefCell<CallStack> = RefCell::new(CallStack::new());
    static TLA_STAGE: RefCell<Stage> = RefCell::new(Stage::Start);
    static TLA_STATES: RefCell<Vec<StatePair>> = RefCell::new(Vec::new());
}

pub fn log_locals(state: &mut InstrumentationState, locals: Vec<(&str, TlaValue)>) {
    let mut assignment = VarAssignment::new();
    for (name, value) in locals {
        assignment.add(name, value);
    }
    state.call_stack.log_locals(assignment);
}

pub fn log_tla_request<F>(
    state: &mut InstrumentationState,
    to: Destination,
    message: TlaValue,
    get_globals: F,
) -> StatePair
where
    F: FnOnce() -> GlobalState,
{
    let global = get_globals();
    let old_stage = mem::replace(&mut state.stage, Stage::Start);
    let start_state = match old_stage {
        Stage::End(start) => start,
        _ => panic!(
            "Issuing request {} to {}, but stage is start",
            message, to.0
        ),
    };
    StatePair {
        start: start_state,
        end: EndState {
            global,
            local: state.call_stack.get_state(),
            requests: vec![RequestBuffer { to, message }],
        },
    }
}

pub fn log_tla_response<F>(
    state: &mut InstrumentationState,
    from: Destination,
    message: TlaValue,
    get_globals: F,
) where
    F: FnOnce() -> GlobalState,
{
    let local = state.call_stack.get_state();
    let global = get_globals();
    let stage = &mut state.stage;
    assert!(
        matches!(stage, Stage::Start),
        "Receiving response {} from {} in end stage",
        message,
        from.0
    );
    *stage = Stage::End(StartState {
        global,
        local,
        responses: vec![ResponseBuffer { from, message }],
    });
}

pub fn log_fn_call(state: &mut InstrumentationState, function: Function) {
    state.call_stack.call_function(function);
}

pub fn log_fn_return(state: &mut InstrumentationState) {
    match state.call_stack.return_from_function() {
        Some(_) => panic!(
            "Seems we are left with an empty stack, but not returning from a top-level method"
        ),
        None => (),
    }
}

// TODO: Does this work for modeling arguments as non-deterministically chosen locals?
pub fn log_method_call<F>(state: &mut InstrumentationState, function: Function, get_globals: F)
where
    F: FnOnce() -> GlobalState,
{
    state.call_stack.call_function(function);
    state.stage = Stage::End(StartState {
        global: get_globals(),
        local: state.call_stack.get_state(),
        responses: Vec::new(),
    });
}

pub fn log_method_return<F>(state: &mut InstrumentationState, get_globals: F) -> StatePair
where
    F: FnOnce() -> GlobalState,
{
    let local = state
        .call_stack
        .return_from_function()
        .expect("Returning from a method, but didn't end up with an empty call stack");

    let start_state = match mem::replace(&mut state.stage, Stage::Start) {
        Stage::End(start) => start,
        _ => panic!("Returning from method, but not in an end state"),
    };
    StatePair {
        start: start_state,
        end: EndState {
            global: get_globals(),
            local,
            requests: Vec::new(),
        },
    }
}

#[macro_export]
macro_rules! tla_log_locals {
    (($($name:ident : $value:expr),*)) => {
        {
            let mut locals = Vec::new();
            $(
                locals.push((stringify!($name), $value.to_tla_value()));
            )*
            with_tla_state(|state| {
                $crate::log_locals(state, locals);
            });
        }
    };
}

#[macro_export]
macro_rules! tla_log_request {
    ($to:expr, $message:expr) => {{
        let message = $message.to_tla_value();

        with_tla_state(|state| {
            with_tla_state_pairs(|state_pairs| {
                let new_state_pair = $crate::log_tla_request(state, $to, message, get_tla_globals);
                state_pairs.push(new_state_pair);
            });
        });
    }};
}

#[macro_export]
macro_rules! tla_log_response {
    ($from:expr, $message:expr) => {{
        let message = $message.to_tla_value();
        with_tla_state(|state| {
            $crate::log_tla_response(state, $from, message, get_tla_globals);
        });
    }};
}

#[macro_export]
macro_rules! tla_log_method_call {
    ($function:expr) => {{
        println!("Logging method call");
        with_tla_state(|state| {
            $crate::log_method_call(state, $function, get_tla_globals);
        });
        println!("Finished logging method call");
    }};
}

#[macro_export]
macro_rules! tla_log_method_return {
    () => {{
        println!("Logging method return");
        with_tla_state(|state| {
            with_tla_state_pairs(|state_pairs| {
                let state_pair = $crate::log_method_return(state, get_tla_globals);
                state_pairs.push(state_pair);
            });
        });
    }};
}

pub mod tla_value;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    mem,
};

use tla_value::*;

static mut TLA: Vec<u32> = Vec::new();

pub fn tla_mut() -> &'static mut Vec<u32> {
    unsafe { &mut TLA }
}

#[derive(Clone)]
pub struct VarAssignment(BTreeMap<String, TlaValue>);

impl VarAssignment {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn update(&self, locals: Vec<(String, TlaValue)>) -> VarAssignment {
        let mut new_locals = self.0.clone();
        new_locals.extend(locals);
        VarAssignment(new_locals)
    }

    pub fn merge(&self, other: VarAssignment) -> VarAssignment {
        let mut new_locals = self.0.clone();
        new_locals.extend(other.0);
        VarAssignment(new_locals)
    }
}

#[derive(Clone)]
pub struct Label(String);

impl Label {
    fn new(name: &str) -> Self {
        Self(name.to_string())
    }

    fn merge(&self, other: &Label) -> Label {
        Label(format!("{}_{}", self.0, other.0))
    }
}

pub struct Function {
    // TODO: do we want checks that only declared variables are set?
    // vars: BTreeSet<String>,
    default_start_locals: VarAssignment,
    default_end_locals: VarAssignment,
    // Only for top-level methods
    start_end_labels: Option<(Label, Label)>,
    // TODO: do we want checks that all labels come from an allowed set?
    // labels: BTreeSet<Label>,
}

struct StackElem {
    function: Function,
    label: Option<Label>,
    locals: VarAssignment,
}

struct CallStack(Vec<StackElem>);

pub struct LocalState {
    locals: VarAssignment,
    label: Label,
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

pub struct GlobalState(VarAssignment);

pub struct Destination(String);
pub struct RequestBuffer {
    to: Destination,
    message: TlaValue,
}

pub struct ResponseBuffer {
    from: Destination,
    message: TlaValue,
}

pub struct StartState {
    pub global: GlobalState,
    pub local: LocalState,
    pub responses: Vec<ResponseBuffer>,
}

struct EndState {
    global: GlobalState,
    local: LocalState,
    requests: Vec<RequestBuffer>,
}

struct StatePair {
    start: StartState,
    end: EndState,
}

enum Stage {
    Start,
    End(StartState),
}

thread_local! {
    static TLA_CALL_STACK: RefCell<CallStack> = RefCell::new(CallStack::new());
    static TLA_STAGE: RefCell<Stage> = RefCell::new(Stage::Start);
    static TLA_STATES: RefCell<Vec<StatePair>> = RefCell::new(Vec::new());
    static TLA_RESPONSE : RefCell<Option<ResponseBuffer>> = RefCell::new(None);
}

pub fn log_start_locals(locals: VarAssignment) {
    assert!(
        TLA_STAGE.with_borrow(|stage| matches!(*stage, Stage::Start)),
        "Logging start locals in end stage"
    );
    TLA_CALL_STACK.with_borrow_mut(|stack| {
        stack.log_locals(locals);
    })
}

pub fn log_end_locals<F>(locals: VarAssignment, get_globals: F)
where
    F: FnOnce() -> GlobalState,
{
    TLA_STAGE.with(|sstage| {
        let mut stage = sstage.borrow_mut();
        if matches!(*stage, Stage::Start) {
            let local = TLA_CALL_STACK.with(|stack| stack.borrow().get_state());
            let global = get_globals();
            TLA_RESPONSE.with(|rresp| {
                let resp = rresp.borrow_mut().take();
                *stage = Stage::End(StartState {
                    local,
                    global,
                    responses: resp.into_iter().collect(),
                });
            });
        }
    });
    TLA_CALL_STACK.with(|stack| {
        stack.borrow_mut().log_locals(locals);
    })
}

pub fn log_tla_request<F>(to: Destination, message: TlaValue, get_globals: F)
where
    F: FnOnce() -> GlobalState,
{
    // TODO: what if we don't change any locals before issuing a request?
    TLA_STAGE.with(|stage| {
        assert!(
            matches!(*stage.borrow(), Stage::End(_)),
            "Logging request in start stage"
        );
    });
    let global = get_globals();
    TLA_STATES.with_borrow_mut(|states| {
        TLA_STAGE.with_borrow_mut(|stage| {
            let old_stage = mem::replace(&mut *stage, Stage::Start);
            let start_state = match old_stage {
                Stage::End(start) => start,
                _ => panic!("Issuing request, but not in an end state"),
            };
            states.push(StatePair {
                start: start_state,
                end: EndState {
                    global,
                    local: TLA_CALL_STACK.with(|stack| stack.borrow().get_state()),
                    requests: vec![RequestBuffer { to, message }],
                },
            });
        })
    });
}

pub fn log_tla_response<F>(from: Destination, message: TlaValue, get_globals: F)
where
    F: FnOnce() -> GlobalState,
{
    TLA_STAGE.with_borrow_mut(|stage| {
        *stage = Stage::Start;
    });
    let global = get_globals();
    TLA_RESPONSE.with(|resp| {
        resp.borrow_mut().replace(ResponseBuffer { from, message });
    });
}

pub fn log_fn_call<F>(function: Function) {
    TLA_CALL_STACK.with_borrow_mut(|stack| {
        stack.call_function(function);
    });
}

pub fn log_fn_return() {
    TLA_CALL_STACK.with_borrow_mut(|stack| {
        stack.return_from_function();
        if stack.0.is_empty() {
            TLA_STATES.with_borrow_mut(|states| TLA_STAGE.with_borrow_mut(f));
        }
    });
}

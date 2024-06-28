pub mod checker;
pub mod tla_state;
pub mod tla_value;
use std::mem;

pub use tla_state::*;
pub use tla_value::*;

#[derive(Debug)]
pub struct Update {
    // TODO: do we want checks that only declared variables are set?
    // vars: BTreeSet<String>,
    pub default_start_locals: VarAssignment,
    pub default_end_locals: VarAssignment,
    // Only for top-level methods
    pub start_label: Label,
    pub end_label: Label,
    // TODO: do we want checks that all labels come from an allowed set?
    // labels: BTreeSet<Label>,
    pub process_id: String,
}

#[derive(Debug)]
enum LocationStackElem {
    Label(Label),
    Placeholder,
}
#[derive(Debug)]
struct LocationStack(Vec<LocationStackElem>);

impl LocationStack {
    pub fn merge_labels(&self) -> Label {
        let mut label = Label::new("");
        for elem in self.0.iter() {
            match elem {
                LocationStackElem::Label(l) => label = label.merge(l),
                LocationStackElem::Placeholder => {
                    panic!("Placeholder found in the location stack while trying to merge labels")
                }
            }
        }
        label
    }
}

#[derive(Debug)]
struct Context {
    update: Update,
    locals: VarAssignment,
    location: LocationStack,
}

impl Context {
    fn new(update: Update) -> Self {
        let location = LocationStack(vec![LocationStackElem::Label(update.start_label.clone())]);
        let locals = update.default_start_locals.clone();
        Self {
            update,
            locals,
            location,
        }
    }

    fn call_function(&mut self) {
        self.location.0.push(LocationStackElem::Placeholder);
    }

    fn return_from_function(&mut self) -> () {
        let _f = self.location.0.pop().expect("No function in call stack");
    }

    fn end_update(&mut self) -> LocalState {
        LocalState {
            // TODO: do we really want to overwrite all the current values? I guess it's fine
            locals: self.update.default_end_locals.clone(),
            label: self.update.end_label.clone(),
        }
    }

    fn get_state(&self) -> LocalState {
        let label = self.location.merge_labels();
        LocalState {
            locals: self.locals.clone(),
            label,
        }
    }

    // TODO: handle passing &mut locals to called functions somehow; what if they're called differently?
    fn log_locals(&mut self, locals: VarAssignment) {
        self.locals = self.locals.merge(locals);
    }
}

#[derive(Debug)]
enum Stage {
    Start,
    End(StartState),
}

#[derive(Debug)]
pub struct InstrumentationState {
    context: Context,
    stage: Stage,
}

impl InstrumentationState {
    pub fn new(update: Update, global: GlobalState) -> Self {
        let locals = update.default_start_locals.clone();
        let label = update.start_label.clone();
        Self {
            context: Context::new(update),
            stage: Stage::End(StartState {
                global,
                local: LocalState { locals, label },
                responses: Vec::new(),
            }),
        }
    }
}

pub fn log_locals(state: &mut InstrumentationState, locals: Vec<(&str, TlaValue)>) {
    let mut assignment = VarAssignment::new();
    for (name, value) in locals {
        assignment.add(name, value);
    }
    state.context.log_locals(assignment);
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
        _ => panic!("Issuing request {} to {}, but stage is start", message, to),
    };
    StatePair {
        start: start_state,
        end: EndState {
            global,
            local: state.context.get_state(),
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
    let local = state.context.get_state();
    let global = get_globals();
    let stage = &mut state.stage;
    assert!(
        matches!(stage, Stage::Start),
        "Receiving response {} from {} in end stage",
        message,
        from
    );
    *stage = Stage::End(StartState {
        global,
        local,
        responses: vec![ResponseBuffer { from, message }],
    });
}

pub fn log_fn_call(state: &mut InstrumentationState) {
    state.context.call_function();
}

pub fn log_fn_return(state: &mut InstrumentationState) {
    state.context.return_from_function()
}

// TODO: Does this work for modeling arguments as non-deterministically chosen locals?
pub fn log_method_call<F>(function: Update, get_globals: F) -> InstrumentationState
where
    F: FnOnce() -> GlobalState,
{
    InstrumentationState::new(function, get_globals())
}

pub fn log_method_return<F>(state: &mut InstrumentationState, get_globals: F) -> StatePair
where
    F: FnOnce() -> GlobalState,
{
    let local = state.context.end_update();

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

/// Logs the value of local variables at the end of the current message handler.
/// This might be called multiple times in a single message handler, in particular
/// if the message handler is implemented through several functions, each of which
/// has local variables that are reflected in the TLA model.
/// It assumes that there is a function:
/// `with_tla_state<F>(f: F) where F: FnOnce(&mut InstrumentationState) -> ()`
/// in scope (typically providing a way to mutate some global canister variable).
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

/// Logs the sending of a request (ending a message handler).
/// It assumes that there are the following three functions in scope:
/// 1. `get_tla_globals() -> GlobalState
/// 2. `with_tla_state<F>(f: F) where F: FnOnce(&mut InstrumentationState) -> ()
/// 3. `with_tla_state_pairs<F>(f: F) where F: FnOnce(&mut Vec<StatePair>) -> ()
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

/// Logs the receipt of a response (that starts a new message handler).
/// It assumes that there are the following two functions in scope:
/// 1. `get_tla_globals() -> GlobalState`
/// 2. with_tla_state<F>(f: F) where F: FnOnce(&mut InstrumentationState) -> ()
#[macro_export]
macro_rules! tla_log_response {
    ($from:expr, $message:expr) => {{
        let message = $message.to_tla_value();
        with_tla_state(|state| {
            $crate::log_tla_response(state, $from, message, get_tla_globals);
        });
    }};
}

/// Logs the start of a method (top-level update)
/// It assumes that there are the following two functions in scope:
/// 1. `get_tla_globals() -> GlobalState`
/// 2. with_tla_state<F>(f: F) where F: FnOnce(&mut InstrumentationState) -> ()
/// This macro is normally not called directly; rather, the attribute proc macro tla_update
/// is used instead.
#[macro_export]
macro_rules! tla_log_method_call {
    ($update:expr) => {{
        init_tla_state($crate::log_method_call($update, get_tla_globals))
    }};
}

/// Logs the start of a method (top-level update)
/// This assumes that there are the following three functions in scope:
/// 1. `get_tla_globals() -> GlobalState`
/// 2. `with_tla_state<F>(f: F) where F: FnOnce(&mut InstrumentationState) -> ()`
/// 3. `with_tla_state_pairs<F>(f: F) where F: FnOnce(&mut Vec<StatePair>) -> ()`
/// This macro is normally not called directly; rather, the attribute proc macro tla_update
/// is used instead.
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

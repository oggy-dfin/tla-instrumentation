use tla_instrumentation::{
    tla_log_locals, tla_log_request, tla_log_response, tla_value::ToTla, Destination, Function,
    GlobalState, InstrumentationState, Label, StatePair, VarAssignment,
};
use tla_instrumentation_proc_macros::tla_update;

static mut STATE_PAIRS: Vec<StatePair> = Vec::new();

static mut STATE: Option<InstrumentationState> = None;

fn with_tla_state<F>(f: F)
where
    F: FnOnce(&mut InstrumentationState) -> (),
{
    unsafe {
        if let Some(ref mut state) = STATE {
            // print!("State before with_tla_state: {:?}", state);
            f(state);
            // print!("State after with_tla_state: {:?}", state)
        } else {
            panic!("Instrumentation state not initialized");
        }
    }
}

fn with_tla_state_pairs<F>(f: F)
where
    F: FnOnce(&mut Vec<StatePair>) -> (),
{
    unsafe {
        f(&mut STATE_PAIRS);
        // print!("State after with_tla_state_pairs: {:?}", STATE_PAIRS);
    }
}

fn get_tla_globals() -> GlobalState {
    GlobalState::new()
}

fn fn_desc() -> Function {
    Function {
        default_start_locals: VarAssignment::new(),
        default_end_locals: VarAssignment::new(),
        start_end_labels: Some((Label::new("Start_Label"), Label::new("End_Label"))),
    }
}

#[tla_update(fn_desc())]
fn my_f() {
    tla_log_locals!((x : 1_u64));
    tla_log_request!(Destination::new("Destination"), 1_u64);
    tla_log_response!(Destination::new("Destination"), 1_u64);
}

fn main() {
    unsafe {
        STATE = Some(InstrumentationState::new());
    }
    // tla!(1);
    // tla!(3);
    my_f();
    with_tla_state_pairs(|pairs| {
        println!("----------------");
        print!("State pairs:");
        for pair in pairs.iter() {
            println!("{:?}", pair.start);
            println!("{:?}", pair.end);
        }
        println!("----------------");
    })
}

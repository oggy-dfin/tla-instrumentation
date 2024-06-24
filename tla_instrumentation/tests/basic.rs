use tla_instrumentation::{
    tla_log_locals, tla_log_request, tla_log_response, tla_value::ToTla, Destination,
};
use tla_instrumentation_proc_macros::tla_update;

// Example of how to separate as much of the instrumentation code as possible from the main code
mod tla_stuff {
    use tla_instrumentation::{
        Function, GlobalState, InstrumentationState, Label, StatePair, VarAssignment,
    };
    static mut STATE_PAIRS: Vec<StatePair> = Vec::new();

    static mut STATE: Option<InstrumentationState> = None;

    pub fn with_tla_state<F>(f: F)
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

    pub fn with_tla_state_pairs<F>(f: F)
    where
        F: FnOnce(&mut Vec<StatePair>) -> (),
    {
        unsafe {
            f(&mut STATE_PAIRS);
            // print!("State after with_tla_state_pairs: {:?}", STATE_PAIRS);
        }
    }

    pub fn get_tla_globals() -> GlobalState {
        GlobalState::new()
    }

    pub fn my_f_desc() -> Function {
        Function {
            default_start_locals: VarAssignment::new(),
            default_end_locals: VarAssignment::new(),
            start_end_labels: Some((Label::new("Start_Label"), Label::new("End_Label"))),
        }
    }

    pub fn init_instrumentation() {
        unsafe {
            STATE = Some(InstrumentationState::new());
        }
    }
}

use tla_stuff::{
    get_tla_globals, init_instrumentation, my_f_desc, with_tla_state, with_tla_state_pairs,
};

#[tla_update(my_f_desc())]
fn my_f() {
    tla_log_locals!((x : 1_u64));
    tla_log_request!(Destination::new("Destination"), 1_u64);
    tla_log_response!(Destination::new("Destination"), 1_u64);
}

#[test]
fn basic_test() {
    init_instrumentation();
    my_f();
    with_tla_state_pairs(|pairs| {
        println!("----------------");
        print!("State pairs:");
        for pair in pairs.iter() {
            println!("{:?}", pair.start);
            println!("{:?}", pair.end);
        }
        println!("----------------");
        assert_eq!(pairs.len(), 2);
        let first = &pairs[0];
        assert_eq!(
            first.end.local.locals.0.get("x"),
            Some(&1_u64.to_tla_value())
        );
    })
}

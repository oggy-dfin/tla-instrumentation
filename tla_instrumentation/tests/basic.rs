use std::collections::{BTreeMap, BTreeSet};

use tla_instrumentation::{
    tla_log_locals, tla_log_request, tla_log_response, tla_value::ToTla, Destination,
    ResolvedStatePair, TlaValue,
};
use tla_instrumentation_proc_macros::tla_update;

const PID: &str = "My_F_PID";
const CAN_NAME: &str = "mycan";

static mut GLOBAL: u64 = 0;

fn init_global() {
    unsafe {
        GLOBAL = 0;
    }
}
// Example of how to separate as much of the instrumentation code as possible from the main code
#[macro_use]
mod tla_stuff {
    use super::{CAN_NAME, PID};
    use tla_instrumentation::{
        GlobalState, InstrumentationState, Label, ResolvedStatePair, ToTla, Update, VarAssignment,
    };

    static mut STATE_PAIRS: Vec<ResolvedStatePair> = Vec::new();

    static mut STATE: Option<InstrumentationState> = None;

    pub fn init_tla_state(s: InstrumentationState) -> () {
        unsafe {
            assert!(
                STATE.is_none(),
                "Starting a new update with state {:?} without finishing the previous one",
                s
            );
            STATE = Some(s);
        }
    }

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
        F: FnOnce(&mut Vec<ResolvedStatePair>) -> (),
    {
        unsafe {
            // As unsafe as anything else, but see
            // https://github.com/rust-lang/rust/issues/114447
            // for why this particular syntax here
            f(&mut *std::ptr::addr_of_mut!(STATE_PAIRS));
        }
    }

    pub fn get_tla_globals() -> GlobalState {
        let mut state = GlobalState::new();
        unsafe {
            state.add("global", super::GLOBAL.to_tla_value());
        }
        state
    }

    // #[macro_export]
    macro_rules! get_tla_globals {
        () => {
            tla_stuff::get_tla_globals
        };
    }

    pub fn my_f_desc() -> Update {
        Update {
            default_start_locals: VarAssignment::new(),
            default_end_locals: VarAssignment::new(),
            start_label: Label::new("Start_Label"),
            end_label: Label::new("End_Label"),
            process_id: PID.to_string(),
            canister_name: CAN_NAME.to_string(),
        }
    }
}

use tla_stuff::{init_tla_state, my_f_desc, with_tla_state, with_tla_state_pairs};

#[tla_update(my_f_desc())]
fn my_f() {
    tla_log_locals!((x : 1_u64));
    unsafe {
        GLOBAL = 1;
    }
    tla_log_request!(Destination::new("othercan"), 2_u64);
    tla_log_response!(Destination::new("othercan"), 3_u64);
    unsafe {
        GLOBAL = 2;
    }
}

async fn awaited_f() {
    println!("Being awaited!")
}

#[tla_update(my_f_desc())]
async fn my_f_async() {
    awaited_f().await
}

#[test]
fn basic_test() {
    init_global();
    my_f();
    with_tla_state_pairs(|pairs: &mut Vec<ResolvedStatePair>| {
        println!("----------------");
        print!("State pairs:");
        for pair in pairs.iter() {
            println!("{:?}", pair.start);
            println!("{:?}", pair.end);
        }
        println!("----------------");
        assert_eq!(pairs.len(), 2);
        let first = &pairs[0];
        assert_eq!(first.start.get("global"), Some(&0_u64.to_tla_value()));
        assert_eq!(first.end.get("global"), Some(&1_u64.to_tla_value()));
        assert_eq!(
            first.end.get("x"),
            Some(TlaValue::Function(BTreeMap::from([(
                TlaValue::Literal(PID.to_string()),
                1_u64.to_tla_value()
            ),])))
            .as_ref()
        );
        assert_eq!(
            first
                .end
                .get(format!("{}_to_{}", CAN_NAME, "othercan").as_str()),
            Some(&TlaValue::Seq(vec![2_u64.to_tla_value()]))
        );
        let second = &pairs[1];
        assert_eq!(second.start.get("global"), Some(&1_u64.to_tla_value()));
        assert_eq!(
            second.start.get("x"),
            Some(&TlaValue::Function(BTreeMap::from([(
                TlaValue::Literal(PID.to_string()),
                1_u64.to_tla_value()
            )])))
        );
        assert_eq!(
            second
                .start
                .get(format!("{}_to_{}", "othercan", CAN_NAME).as_str()),
            Some(&BTreeSet::from([3_u64]).to_tla_value())
        );
        assert_eq!(second.end.get("global"), Some(&2_u64.to_tla_value()));
    })
}

#[test]
fn async_test() {
    init_global();
    let _res = tokio_test::block_on(my_f_async());
    with_tla_state_pairs(|pairs: &mut Vec<ResolvedStatePair>| {
        println!("----------------");
        print!("State pairs:");
        for pair in pairs.iter() {
            println!("{:?}", pair.start);
            println!("{:?}", pair.end);
        }
        println!("----------------");
        assert_eq!(pairs.len(), 1);
        let first = &pairs[0];
        assert_eq!(first.start.get("global"), Some(&0_u64.to_tla_value()));
        assert_eq!(first.end.get("global"), Some(&0_u64.to_tla_value()));
    })
}

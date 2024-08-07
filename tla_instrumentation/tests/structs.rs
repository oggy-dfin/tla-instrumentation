use std::{
    collections::{BTreeMap, BTreeSet},
    ptr::addr_of_mut,
};

use tla_instrumentation::{
    tla_log_locals, tla_log_request, tla_log_response, tla_value::ToTla, Destination,
    InstrumentationState,
};
use tla_instrumentation_proc_macros::tla_update_method;

// Example of how to separate as much of the instrumentation code as possible from the main code
#[macro_use]
mod tla_stuff {
    use crate::StructCanister;

    pub const PID: &str = "My_F_PID";
    pub const CAN_NAME: &str = "mycan";

    use std::sync::RwLock;
    use tla_instrumentation::{
        GlobalState, InstrumentationState, Label, ResolvedStatePair, ToTla, Update, VarAssignment,
    };
    use tokio::task_local;

    task_local! {
        pub static TLA_INSTRUMENTATION_STATE: InstrumentationState;
    }

    pub static TLA_TRACES: RwLock<Vec<ResolvedStatePair>> = RwLock::new(Vec::new());

    pub fn tla_get_globals(c: &StructCanister) -> GlobalState {
        let mut state = GlobalState::new();
        state.add("counter", c.counter.to_tla_value());
        state
    }

    // #[macro_export]
    macro_rules! tla_get_globals {
        ($self:expr) => {
            tla_stuff::tla_get_globals($self)
        };
    }

    pub fn my_f_desc() -> Update {
        Update {
            default_start_locals: VarAssignment::new().add("my_local", 0_u64.to_tla_value()),
            default_end_locals: VarAssignment::new(),
            start_label: Label::new("Start_Label"),
            end_label: Label::new("End_Label"),
            process_id: PID.to_string(),
            canister_name: CAN_NAME.to_string(),
        }
    }
}

use tla_stuff::{my_f_desc, CAN_NAME, PID, TLA_INSTRUMENTATION_STATE, TLA_TRACES};

struct StructCanister {
    pub counter: u64,
}

static mut GLOBAL: StructCanister = StructCanister { counter: 0 };

fn call_maker() {
    tla_log_request!(Destination::new("othercan"), 2_u64);
    tla_log_response!(Destination::new("othercan"), 3_u64);
}

impl StructCanister {
    #[tla_update_method(my_f_desc())]
    pub async fn my_method(&mut self) -> () {
        self.counter += 1;
        let mut my_local: u64 = self.counter;
        tla_log_locals! {my_local: my_local};
        call_maker();
        self.counter += 1;
        my_local = self.counter;
        // Note that this would not be necessary (and would be an error) if
        // we defined my_local in default_end_locals in my_f_desc
        tla_log_locals! {my_local: my_local};
        return ();
    }
}

#[test]
fn struct_test() {
    unsafe {
        let canister = &mut *addr_of_mut!(GLOBAL);
        let _res = tokio_test::block_on(canister.my_method());
    }
    let pairs = TLA_TRACES.read().unwrap();
    println!("----------------");
    println!("State pairs:");
    for pair in pairs.iter() {
        println!("{:?}", pair.start);
        println!("{:?}", pair.end);
    }
    println!("----------------");
    assert_eq!(pairs.len(), 2);
    let first = &pairs[0];
    assert_eq!(first.start.get("counter"), Some(&0_u64.to_tla_value()));
    assert_eq!(first.end.get("counter"), Some(&1_u64.to_tla_value()));

    assert_eq!(
        first.start.get("my_local"),
        Some(BTreeMap::from([(PID, 0_u64)]).to_tla_value()).as_ref()
    );
    assert_eq!(
        first.end.get("my_local"),
        Some(BTreeMap::from([(PID, 1_u64)]).to_tla_value()).as_ref()
    );

    assert_eq!(
        first
            .end
            .get(format!("{}_to_{}", CAN_NAME, "othercan").as_str()),
        Some(&vec![2_u64].to_tla_value())
    );

    let second = &pairs[1];

    assert_eq!(second.start.get("counter"), Some(&1_u64.to_tla_value()));
    assert_eq!(second.end.get("counter"), Some(&2_u64.to_tla_value()));

    assert_eq!(
        second.start.get("my_local"),
        Some(BTreeMap::from([(PID, 1_u64)]).to_tla_value()).as_ref()
    );
    assert_eq!(
        second.end.get("my_local"),
        Some(BTreeMap::from([(PID, 2_u64)]).to_tla_value()).as_ref()
    );

    assert_eq!(
        second
            .start
            .get(format!("{}_to_{}", "othercan", CAN_NAME).as_str()),
        Some(&BTreeSet::from([3_u64]).to_tla_value())
    );
}

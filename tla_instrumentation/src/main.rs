use macros::*;
use canister::tla_mut;
use proc_macros::tla_update;

#[tla_update(5)]
fn my_f() { }


fn main() {
    tla!(1);
    tla!(3);
    my_f();
    println!("{:?}", tla_mut());
}

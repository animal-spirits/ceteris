//! A run that cannot start publishes a `load` halt and nothing else (ADR 0214).

use ceteris_app::sim::{Halt, Runner};
use ceteris_econ::EconWorld;

#[test]
fn failed_build_is_a_load_halt() {
    let runner = Runner::spawn::<EconWorld>(2, || Err("no pack".into()), |_| (), || {});
    assert_eq!(
        runner.wait().unwrap().unwrap_err(),
        Halt {
            tick: 0,
            kind: "load".into(),
            detail: "no pack".into()
        }
    );
    assert!(runner.wait().is_none());
}

//! `ceteris gravity`: argument errors stop before any source is read.

use std::process::Command;

#[test]
fn gravity_rejects_bad_arguments() {
    for (args, message) in [
        (vec!["gravity", "--pack"], "--pack needs a directory"),
        (vec!["gravity", "--out"], "--out needs a file"),
        (
            vec!["gravity", "--write", "--ticks"],
            "unknown argument '--ticks'",
        ),
    ] {
        let run = Command::new(env!("CARGO_BIN_EXE_ceteris"))
            .args(&args)
            .output()
            .unwrap();
        assert!(!run.status.success(), "{args:?}");
        let stderr = String::from_utf8_lossy(&run.stderr);
        assert!(stderr.contains(message), "{args:?}: {stderr}");
    }
}

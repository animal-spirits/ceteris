//! The repository itself passes `rows check`.

use std::path::Path;

#[test]
fn repository_passes_check() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let errors = ceteris_rows::check(&root).unwrap();
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

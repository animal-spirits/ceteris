//! The reference model is written from the spec and must not depend on the crates it checks.

use std::process::Command;

const FORBIDDEN: [&str; 4] = [
    "ceteris-core",
    "ceteris-geo",
    "ceteris-trade",
    "ceteris-econ",
];

#[test]
fn reference_does_not_depend_on_core_crates() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        .args(["metadata", "--format-version", "1"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let nodes = meta["resolve"]["nodes"].as_array().unwrap();
    let id_name = |id: &str| -> String {
        meta["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == id)
            .map(|p| p["name"].as_str().unwrap().to_owned())
            .unwrap()
    };
    let start = meta["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "ceteris-reference")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    // Every transitive dependency, normal, build and dev.
    let mut seen = vec![start.clone()];
    let mut stack = vec![start];
    while let Some(id) = stack.pop() {
        let node = nodes.iter().find(|n| n["id"] == id.as_str()).unwrap();
        for dep in node["dependencies"].as_array().unwrap() {
            let dep = dep.as_str().unwrap().to_owned();
            if !seen.contains(&dep) {
                seen.push(dep.clone());
                stack.push(dep);
            }
        }
    }
    let names: Vec<String> = seen.iter().map(|id| id_name(id)).collect();
    for bad in FORBIDDEN {
        assert!(
            !names.iter().any(|n| n == bad),
            "ceteris-reference depends on {bad}"
        );
    }
}

use ceteris_rows as rows;
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "usage: rows check | next [N] | next-code FAMILY | render";

fn run(args: &[String]) -> Result<ExitCode, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["check"] => {
            let errors = rows::check(&root)?;
            for e in &errors {
                eprintln!("{e}");
            }
            if errors.is_empty() {
                println!("rows: ok");
                Ok(ExitCode::SUCCESS)
            } else {
                eprintln!("rows: {} problem(s)", errors.len());
                Ok(ExitCode::FAILURE)
            }
        }
        ["next"] | ["next", _] => {
            let n = match args.get(1) {
                Some(s) => s.parse().map_err(|_| USAGE.to_string())?,
                None => 1,
            };
            let (owner, _) = rows::allocations(&rows::load(&root)?);
            for id in rows::next_ids(&owner, n) {
                println!("{}", rows::fmt_id(id));
            }
            Ok(ExitCode::SUCCESS)
        }
        ["next-code", family] if rows::CODE_FAMILIES.contains(&family) => {
            println!("{}", rows::next_spec_code(&root, family)?);
            Ok(ExitCode::SUCCESS)
        }
        ["render"] => {
            if rows::render_in_place(&root)? {
                println!("rows: rewrote {}", rows::APPENDIX);
            }
            Ok(ExitCode::SUCCESS)
        }
        _ => Err(USAGE.into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run(&args).unwrap_or_else(|e| {
        eprintln!("{e}");
        ExitCode::from(2)
    })
}

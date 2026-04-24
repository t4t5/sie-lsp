//! Covers every diagnostic code the parser can emit.

use std::collections::HashSet;

use sie_parser::{diagnostics as dc, parse, read_file};

#[test]
fn every_diagnostic_code_fires_at_least_once() {
    let path = std::path::Path::new("tests/fixtures/broken.se");
    let src = read_file(path).expect("read broken.se");
    let out = parse(&src);
    let seen: HashSet<&str> = out.diagnostics.iter().map(|d| d.code).collect();

    let expected: &[&str] = dc::ALL;
    let mut missing: Vec<&&str> = expected.iter().filter(|c| !seen.contains(*c)).collect();
    missing.sort();
    assert!(
        missing.is_empty(),
        "missing diagnostic codes in broken.se fixture: {missing:?}\nsaw: {seen:?}\nall diags: {:#?}",
        out.diagnostics
    );
}

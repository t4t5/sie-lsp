//! Integration test: parse the real SIE4 export from Visma Administration 2000
//! and assert it produces no Error-severity diagnostics.

use sie_lsp::{decode_cp437, parse, Severity};

#[test]
fn sample_parses_without_errors() {
    let bytes = std::fs::read("tests/fixtures/sample.se")
        .expect("tests/fixtures/sample.se must exist");
    let text = decode_cp437(&bytes);
    let out = parse(&text);
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "expected no Error-severity diagnostics, got: {errors:#?}"
    );
    assert!(
        out.items.len() > 2000,
        "expected a large number of items from the 4k-line sample, got {}",
        out.items.len()
    );
    assert_eq!(out.items[0].label, "#FLAGGA", "first item should be #FLAGGA");
}

#[test]
fn sample_company_name_decoded_from_cp437() {
    let bytes = std::fs::read("tests/fixtures/sample.se").unwrap();
    let text = decode_cp437(&bytes);
    let out = parse(&text);
    let fnamn = out
        .items
        .iter()
        .find(|it| it.label == "#FNAMN")
        .expect("sample has #FNAMN");
    // Expect the Swedish "Övningsbolaget AB" round-tripped from CP437.
    let first_field = &fnamn.fields[0];
    let text = match &first_field.value {
        sie_lsp::FieldValue::Quoted { text } => text.as_str(),
        sie_lsp::FieldValue::Bare { text } => text.as_str(),
        _ => panic!("unexpected field value"),
    };
    assert_eq!(text, "Övningsbolaget AB");
}

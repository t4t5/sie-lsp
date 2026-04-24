//! Typed domain model over a parsed SIE file.
//!
//! `parse` returns a loosely-typed `Item` tree. `read` walks that tree once
//! into a `SieDocument` whose fields are the shapes consumers actually want:
//! a `Company`, a list of `FiscalYear`s, accounts keyed by number, and
//! `(year_idx, account_no)`-indexed balance maps for IB / UB / RES.
//!
//! Unknown or irrelevant labels (`#DIM`, `#PSALDO`, `#BTRANS`, `#VER`,
//! `#KSUMMA`, `#FNR`, `#FLAGGA`, …) are ignored. Parse errors from the
//! underlying parser are discarded here; call `parse` directly if you need
//! diagnostics.

use crate::parser;
use crate::types::{FieldValue, Item, ParseOutput};
use anyhow::{Context, Result, anyhow, bail};
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use std::str::FromStr;

/// Year index as used in SIE `#RAR`/`#IB`/`#UB`/`#RES`: 0 = current fiscal
/// year, -1 = previous, etc.
pub type YearIdx = i32;

/// SIE account number (1000..9999 range in BAS charts).
pub type AccountNo = u32;

/// Skatteverket SRU reporting code (e.g. 7302).
pub type SruCode = u32;

#[derive(Debug, Clone)]
pub struct Company {
    pub name: String,
    /// Stored without dash: `"559174-1383"` → `"5591741383"`. Consumers that
    /// need a legal-form prefix (`"16"` for Aktiebolag) add it themselves.
    pub orgnr_raw: String,
    /// Extracted from `#ADRESS` token 2 (e.g. `"106 31 STOCKHOLM"`); may be
    /// `None` if the address didn't parse cleanly.
    pub postnr: Option<String>,
    pub postort: Option<String>,
}

impl Default for Company {
    fn default() -> Self {
        Self {
            name: String::new(),
            orgnr_raw: String::new(),
            postnr: None,
            postort: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FiscalYear {
    pub idx: YearIdx,
    /// YYYYMMDD as an 8-digit string.
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone)]
pub struct Account {
    pub no: AccountNo,
    pub name: String,
    pub sru: Option<SruCode>,
}

#[derive(Debug, Clone, Default)]
pub struct SieDocument {
    pub company: Company,
    pub years: Vec<FiscalYear>,
    pub accounts: BTreeMap<AccountNo, Account>,
    /// `(year_idx, account_no) → closing balance (UB)`
    pub ub: BTreeMap<(YearIdx, AccountNo), Decimal>,
    /// `(year_idx, account_no) → opening balance (IB)`
    pub ib: BTreeMap<(YearIdx, AccountNo), Decimal>,
    /// `(year_idx, account_no) → P&L period result (RES)`
    pub res: BTreeMap<(YearIdx, AccountNo), Decimal>,
}

impl SieDocument {
    pub fn current_year(&self) -> Option<&FiscalYear> {
        self.years.iter().find(|y| y.idx == 0)
    }
}

/// Parse SIE source and walk the resulting AST into a typed `SieDocument`.
pub fn read(src: &str) -> Result<SieDocument> {
    let ParseOutput { items, diagnostics: _ } = parser::parse(src);

    let mut doc = SieDocument::default();
    let mut pending_sru: Vec<(AccountNo, SruCode)> = Vec::new();

    for item in &items {
        match item.label.as_str() {
            "#FNAMN" => doc.company.name = string_field(item, 0)?.to_string(),
            "#ORGNR" => {
                let raw = string_field(item, 0)?.to_string();
                doc.company.orgnr_raw = raw.replace('-', "");
            }
            "#ADRESS" => {
                // Token index 2 is the postal address, e.g. "106 31 STOCKHOLM".
                if let Some(addr) = item.fields.get(2).and_then(field_str) {
                    let (postnr, postort) = parse_postal_address(addr);
                    doc.company.postnr = postnr;
                    doc.company.postort = postort;
                }
            }
            "#RAR" => {
                let idx: i32 = int_field(item, 0)?;
                let start = string_field(item, 1)?.to_string();
                let end = string_field(item, 2)?.to_string();
                doc.years.push(FiscalYear { idx, start, end });
            }
            "#KONTO" => {
                let no: AccountNo = int_field(item, 0)?;
                let name = string_field(item, 1)?.to_string();
                doc.accounts.insert(
                    no,
                    Account {
                        no,
                        name,
                        sru: None,
                    },
                );
            }
            "#SRU" => {
                let account: AccountNo = int_field(item, 0)?;
                let sru_str = string_field(item, 1)?;
                let sru: SruCode = sru_str
                    .parse()
                    .with_context(|| format!("invalid SRU code {sru_str:?}"))?;
                pending_sru.push((account, sru));
            }
            "#IB" => {
                let (yr, acct, amt) = balance_fields(item)?;
                doc.ib.insert((yr, acct), amt);
            }
            "#UB" => {
                let (yr, acct, amt) = balance_fields(item)?;
                doc.ub.insert((yr, acct), amt);
            }
            "#RES" => {
                let (yr, acct, amt) = balance_fields(item)?;
                doc.res.insert((yr, acct), amt);
            }
            _ => {}
        }
    }

    for (acct, sru) in pending_sru {
        if let Some(a) = doc.accounts.get_mut(&acct) {
            a.sru = Some(sru);
        }
    }

    if doc.company.orgnr_raw.is_empty() {
        bail!("#ORGNR missing from SIE file");
    }
    if doc.company.name.is_empty() {
        bail!("#FNAMN missing from SIE file");
    }
    if doc.current_year().is_none() {
        bail!("#RAR 0 (current fiscal year) missing from SIE file");
    }

    Ok(doc)
}

fn field_str(f: &crate::types::Field) -> Option<&str> {
    match &f.value {
        FieldValue::Bare { text } | FieldValue::Quoted { text } => Some(text.as_str()),
        FieldValue::ObjectList { .. } => None,
    }
}

fn string_field(item: &Item, idx: usize) -> Result<&str> {
    item.fields
        .get(idx)
        .and_then(field_str)
        .ok_or_else(|| anyhow!("{} field #{} missing or non-string", item.label, idx))
}

fn int_field<T: FromStr>(item: &Item, idx: usize) -> Result<T>
where
    T::Err: std::fmt::Display,
{
    let s = string_field(item, idx)?;
    s.parse::<T>()
        .map_err(|e| anyhow!("{} field #{} {:?}: {e}", item.label, idx, s))
}

fn decimal_field(item: &Item, idx: usize) -> Result<Decimal> {
    let s = string_field(item, idx)?;
    Decimal::from_str(s).map_err(|e| anyhow!("{} field #{} {:?}: {e}", item.label, idx, s))
}

fn balance_fields(item: &Item) -> Result<(YearIdx, AccountNo, Decimal)> {
    let yr: YearIdx = int_field(item, 0)?;
    let acct: AccountNo = int_field(item, 1)?;
    let amt = decimal_field(item, 2)?;
    Ok((yr, acct, amt))
}

/// Split e.g. `"106 31 STOCKHOLM"` into (`"10631"`, `"STOCKHOLM"`).
/// Postal address format per SIE spec and Swedish convention:
///   `"XXX XX CITY"` or `"XXXXX CITY"`.
fn parse_postal_address(addr: &str) -> (Option<String>, Option<String>) {
    let trimmed = addr.trim();
    let mut chars = trimmed.chars().peekable();
    let mut digits = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            digits.push(c);
            chars.next();
        } else if c == ' ' && digits.len() < 5 {
            // Skip the space inside the postal code, e.g. "106 31".
            chars.next();
        } else {
            break;
        }
    }
    if digits.len() != 5 {
        return (None, None);
    }
    let rest: String = chars.collect();
    let city = rest.trim().to_string();
    if city.is_empty() {
        (Some(digits), None)
    } else {
        (Some(digits), Some(city))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postal_spaced() {
        assert_eq!(
            parse_postal_address("106 31 STOCKHOLM"),
            (Some("10631".into()), Some("STOCKHOLM".into()))
        );
    }

    #[test]
    fn postal_unspaced() {
        assert_eq!(
            parse_postal_address("12345 GÖTEBORG"),
            (Some("12345".into()), Some("GÖTEBORG".into()))
        );
    }

    #[test]
    fn postal_junk() {
        assert_eq!(parse_postal_address("no postnr here"), (None, None));
    }
}

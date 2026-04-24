//! The SIE 4B label schema. Transcribed from `docs/spec.md` §11 and §12.
//!
//! The parser, hover provider, and completion provider all drive off this
//! table. Keep `LABELS` the single source of truth.

pub struct LabelSpec {
    /// Label including the leading `#`, e.g. `"#KONTO"`.
    pub label: &'static str,
    /// Paragraph description rendered as hover markdown.
    pub description: &'static str,
    /// Human-readable field layout, e.g. `"#KONTO account_no account_name"`.
    pub format: &'static str,
    pub fields: &'static [FieldSpec],
    /// True iff the item owns a `{ ... }` brace block with child items.
    /// Currently only `#VER` is a container per spec §5.4.
    pub container: bool,
}

pub struct FieldSpec {
    pub name: &'static str,
    pub required: bool,
    pub kind: FieldKind,
}

pub enum FieldKind {
    /// Any string. No validation beyond the common lexer checks.
    String,
    /// Optional leading `-`, digits only.
    Integer,
    /// Decimal amount. Matches `-?\d+(\.\d{1,2})?`.
    Decimal,
    /// `YYYYMMDD`. 8 digits; month 01..=12, day 01..=31.
    Date,
    /// Exact match against one of the listed variants (ASCII case-insensitive).
    Enum(&'static [&'static str]),
    /// `{ dim_no obj_no dim_no obj_no ... }` inline.
    ObjectList,
    /// Catch-all. Accepted as-is.
    Raw,
}

// ---------- enum variant lists ----------

const FTYP_VARIANTS: &[&str] = &[
    "AB", "E", "HB", "KB", "EK", "KHF", "BRF", "BF", "SF", "I", "S", "FL",
    "BAB", "MB", "SB", "BFL", "FAB", "OFB", "SE", "SCE", "TSF", "X",
];

const KTYP_VARIANTS: &[&str] = &["T", "S", "K", "I"];
const FLAGGA_VARIANTS: &[&str] = &["0", "1"];
const FORMAT_VARIANTS: &[&str] = &["PC8"];
const KPTYP_VARIANTS: &[&str] = &["BAS95", "BAS96", "EUBAS97", "NE2007"];
const SIETYP_VARIANTS: &[&str] = &["1", "2", "3", "4"];

pub static LABELS: &[LabelSpec] = &[
    // ---- Identification items ----
    LabelSpec {
        label: "#FLAGGA",
        description: "Import flag. `0` = not yet imported, `1` = imported. \
                      Must be the first item in the file.",
        format: "#FLAGGA x",
        fields: &[FieldSpec { name: "x", required: true, kind: FieldKind::Enum(FLAGGA_VARIANTS) }],
        container: false,
    },
    LabelSpec {
        label: "#PROGRAM",
        description: "Identifies the program that exported the file.",
        format: "#PROGRAM program_name version",
        fields: &[
            FieldSpec { name: "program_name", required: true, kind: FieldKind::String },
            FieldSpec { name: "version",      required: true, kind: FieldKind::String },
        ],
        container: false,
    },
    LabelSpec {
        label: "#FORMAT",
        description: "Character set used in the file. Only `PC8` (IBM Extended 8-bit ASCII, codepage 437) is permitted.",
        format: "#FORMAT PC8",
        fields: &[FieldSpec { name: "charset", required: true, kind: FieldKind::Enum(FORMAT_VARIANTS) }],
        container: false,
    },
    LabelSpec {
        label: "#GEN",
        description: "When and by whom the file was generated.",
        format: "#GEN date sign",
        fields: &[
            FieldSpec { name: "date", required: true,  kind: FieldKind::Date },
            FieldSpec { name: "sign", required: false, kind: FieldKind::String },
        ],
        container: false,
    },
    LabelSpec {
        label: "#SIETYP",
        description: "Which SIE file type this file conforms to (1..=4).",
        format: "#SIETYP type_no",
        fields: &[FieldSpec { name: "type_no", required: true, kind: FieldKind::Enum(SIETYP_VARIANTS) }],
        container: false,
    },
    LabelSpec {
        label: "#PROSA",
        description: "Free comment text about the file contents.",
        format: "#PROSA text",
        fields: &[FieldSpec { name: "text", required: true, kind: FieldKind::String }],
        container: false,
    },
    LabelSpec {
        label: "#FTYP",
        description: "Company type. Used e.g. to pick the right SRU code set. See spec §11 #FTYP for the list.",
        format: "#FTYP company_type",
        fields: &[FieldSpec { name: "company_type", required: true, kind: FieldKind::Enum(FTYP_VARIANTS) }],
        container: false,
    },
    LabelSpec {
        label: "#FNR",
        description: "The exporting program's internal identifier for the company.",
        format: "#FNR company_id",
        fields: &[FieldSpec { name: "company_id", required: true, kind: FieldKind::String }],
        container: false,
    },
    LabelSpec {
        label: "#ORGNR",
        description: "Corporate identification number. Should contain a hyphen after the sixth digit. \
                      Acquisition and activity numbers are optional.",
        format: "#ORGNR CIN acq_no act_no",
        fields: &[
            FieldSpec { name: "CIN",    required: true,  kind: FieldKind::String },
            FieldSpec { name: "acq_no", required: false, kind: FieldKind::String },
            FieldSpec { name: "act_no", required: false, kind: FieldKind::String },
        ],
        container: false,
    },
    LabelSpec {
        label: "#BKOD",
        description: "Industry code (SNI) for the exported company.",
        format: "#BKOD SNI_code",
        fields: &[FieldSpec { name: "SNI_code", required: true, kind: FieldKind::String }],
        container: false,
    },
    LabelSpec {
        label: "#ADRESS",
        description: "Address information for the exported company: contact, distribution address, \
                      postal address, and telephone number.",
        format: "#ADRESS contact distribution_address postal_address tel",
        fields: &[
            FieldSpec { name: "contact",              required: true, kind: FieldKind::String },
            FieldSpec { name: "distribution_address", required: true, kind: FieldKind::String },
            FieldSpec { name: "postal_address",       required: true, kind: FieldKind::String },
            FieldSpec { name: "tel",                  required: true, kind: FieldKind::String },
        ],
        container: false,
    },
    LabelSpec {
        label: "#FNAMN",
        description: "Complete legal name of the exported company.",
        format: "#FNAMN company_name",
        fields: &[FieldSpec { name: "company_name", required: true, kind: FieldKind::String }],
        container: false,
    },
    LabelSpec {
        label: "#RAR",
        description: "Financial year range. `year_no` is `0` for the current year, `-1` for the previous year, and so on.",
        format: "#RAR year_no start end",
        fields: &[
            FieldSpec { name: "year_no", required: true, kind: FieldKind::Integer },
            FieldSpec { name: "start",   required: true, kind: FieldKind::Date },
            FieldSpec { name: "end",     required: true, kind: FieldKind::Date },
        ],
        container: false,
    },
    LabelSpec {
        label: "#TAXAR",
        description: "Taxation year that the SRU codes apply to.",
        format: "#TAXAR year",
        fields: &[FieldSpec { name: "year", required: true, kind: FieldKind::Integer }],
        container: false,
    },
    LabelSpec {
        label: "#OMFATTN",
        description: "Period end date for period balances (YYYYMMDD).",
        format: "#OMFATTN date",
        fields: &[FieldSpec { name: "date", required: true, kind: FieldKind::Date }],
        container: false,
    },
    LabelSpec {
        label: "#KPTYP",
        description: "Chart of accounts type: BAS95, BAS96, EUBAS97, NE2007. \
                      If missing, BAS 95 is assumed. BAS2xxx is handled as EUBAS97.",
        format: "#KPTYP type",
        fields: &[FieldSpec { name: "type", required: true, kind: FieldKind::Enum(KPTYP_VARIANTS) }],
        container: false,
    },
    LabelSpec {
        label: "#VALUTA",
        description: "Reporting currency (ISO 4217). Defaults to SEK if absent.",
        format: "#VALUTA currency_code",
        fields: &[FieldSpec { name: "currency_code", required: true, kind: FieldKind::String }],
        container: false,
    },

    // ---- Chart of accounts information ----
    LabelSpec {
        label: "#KONTO",
        description: "Account information. Declares the name of an account. Account number must be numeric.",
        format: "#KONTO account_no account_name",
        fields: &[
            FieldSpec { name: "account_no",   required: true, kind: FieldKind::Integer },
            FieldSpec { name: "account_name", required: true, kind: FieldKind::String },
        ],
        container: false,
    },
    LabelSpec {
        label: "#KTYP",
        description: "Account type. `T` = asset, `S` = debt, `K` = cost, `I` = income.",
        format: "#KTYP account_no account_type",
        fields: &[
            FieldSpec { name: "account_no",   required: true, kind: FieldKind::Integer },
            FieldSpec { name: "account_type", required: true, kind: FieldKind::Enum(KTYP_VARIANTS) },
        ],
        container: false,
    },
    LabelSpec {
        label: "#ENHET",
        description: "Unit used for quantity reporting on the account (e.g. `litre`, `kg`).",
        format: "#ENHET account_no unit",
        fields: &[
            FieldSpec { name: "account_no", required: true, kind: FieldKind::Integer },
            FieldSpec { name: "unit",       required: true, kind: FieldKind::String },
        ],
        container: false,
    },
    LabelSpec {
        label: "#SRU",
        description: "SRU code for transferring account balances to a standardised accounts extract.",
        format: "#SRU account SRU_code",
        fields: &[
            FieldSpec { name: "account",  required: true, kind: FieldKind::Integer },
            FieldSpec { name: "SRU_code", required: true, kind: FieldKind::String },
        ],
        container: false,
    },
    LabelSpec {
        label: "#DIM",
        description: "Declares a single dimension. Dimension numbers 1..=19 are reserved for standard \
                      dimensions (cost centre, cost bearer, project, employee, customer, supplier, invoice, …).",
        format: "#DIM dimension_no name",
        fields: &[
            FieldSpec { name: "dimension_no", required: true, kind: FieldKind::Integer },
            FieldSpec { name: "name",         required: true, kind: FieldKind::String },
        ],
        container: false,
    },
    LabelSpec {
        label: "#UNDERDIM",
        description: "Declares a sub-dimension of another dimension. `superdimension` identifies the parent.",
        format: "#UNDERDIM dimension_no name superdimension",
        fields: &[
            FieldSpec { name: "dimension_no",   required: true, kind: FieldKind::Integer },
            FieldSpec { name: "name",           required: true, kind: FieldKind::String },
            FieldSpec { name: "superdimension", required: true, kind: FieldKind::Integer },
        ],
        container: false,
    },
    LabelSpec {
        label: "#OBJEKT",
        description: "Declares an object (value) inside a given dimension.",
        format: "#OBJEKT dimension_no object_no object_name",
        fields: &[
            FieldSpec { name: "dimension_no", required: true, kind: FieldKind::Integer },
            FieldSpec { name: "object_no",    required: true, kind: FieldKind::String },
            FieldSpec { name: "object_name",  required: true, kind: FieldKind::String },
        ],
        container: false,
    },

    // ---- Balance items / Verification items ----
    LabelSpec {
        label: "#IB",
        description: "Opening balance for a balance sheet account. Credit balance is expressed as a negative amount.",
        format: "#IB year_no account balance quantity",
        fields: &[
            FieldSpec { name: "year_no",  required: true,  kind: FieldKind::Integer },
            FieldSpec { name: "account",  required: true,  kind: FieldKind::Integer },
            FieldSpec { name: "balance",  required: true,  kind: FieldKind::Decimal },
            FieldSpec { name: "quantity", required: false, kind: FieldKind::Decimal },
        ],
        container: false,
    },
    LabelSpec {
        label: "#UB",
        description: "Closing balance for a balance sheet account. Credit balance is expressed as a negative amount.",
        format: "#UB year_no account balance quantity",
        fields: &[
            FieldSpec { name: "year_no",  required: true,  kind: FieldKind::Integer },
            FieldSpec { name: "account",  required: true,  kind: FieldKind::Integer },
            FieldSpec { name: "balance",  required: true,  kind: FieldKind::Decimal },
            FieldSpec { name: "quantity", required: false, kind: FieldKind::Decimal },
        ],
        container: false,
    },
    LabelSpec {
        label: "#OIB",
        description: "Opening balance for a balance sheet account, specified at the object level.",
        format: "#OIB year_no account {object_list} balance quantity",
        fields: &[
            FieldSpec { name: "year_no",     required: true,  kind: FieldKind::Integer },
            FieldSpec { name: "account",     required: true,  kind: FieldKind::Integer },
            FieldSpec { name: "object_list", required: true,  kind: FieldKind::ObjectList },
            FieldSpec { name: "balance",     required: true,  kind: FieldKind::Decimal },
            FieldSpec { name: "quantity",    required: false, kind: FieldKind::Decimal },
        ],
        container: false,
    },
    LabelSpec {
        label: "#OUB",
        description: "Closing balance for a balance sheet account, specified at the object level.",
        format: "#OUB year_no account {object_list} balance quantity",
        fields: &[
            FieldSpec { name: "year_no",     required: true,  kind: FieldKind::Integer },
            FieldSpec { name: "account",     required: true,  kind: FieldKind::Integer },
            FieldSpec { name: "object_list", required: true,  kind: FieldKind::ObjectList },
            FieldSpec { name: "balance",     required: true,  kind: FieldKind::Decimal },
            FieldSpec { name: "quantity",    required: false, kind: FieldKind::Decimal },
        ],
        container: false,
    },
    LabelSpec {
        label: "#RES",
        description: "Balance for a profit-and-loss account. Credit balance is expressed as a negative amount.",
        format: "#RES year_no account balance quantity",
        fields: &[
            FieldSpec { name: "year_no",  required: true,  kind: FieldKind::Integer },
            FieldSpec { name: "account",  required: true,  kind: FieldKind::Integer },
            FieldSpec { name: "balance",  required: true,  kind: FieldKind::Decimal },
            FieldSpec { name: "quantity", required: false, kind: FieldKind::Decimal },
        ],
        container: false,
    },
    LabelSpec {
        label: "#PSALDO",
        description: "Period end balance for an account (change during the period). \
                      `period` is `YYYYMM`. The object list is empty `{}` at the account-wide level.",
        format: "#PSALDO year_no period account {object_list} balance quantity",
        fields: &[
            FieldSpec { name: "year_no",     required: true,  kind: FieldKind::Integer },
            FieldSpec { name: "period",      required: true,  kind: FieldKind::Integer },
            FieldSpec { name: "account",     required: true,  kind: FieldKind::Integer },
            FieldSpec { name: "object_list", required: true,  kind: FieldKind::ObjectList },
            FieldSpec { name: "balance",     required: true,  kind: FieldKind::Decimal },
            FieldSpec { name: "quantity",    required: false, kind: FieldKind::Decimal },
        ],
        container: false,
    },
    LabelSpec {
        label: "#PBUDGET",
        description: "Period budget for an account (budgeted change during the period). \
                      `period` is `YYYYMM`.",
        format: "#PBUDGET year_no period account {object_list} balance quantity",
        fields: &[
            FieldSpec { name: "year_no",     required: true,  kind: FieldKind::Integer },
            FieldSpec { name: "period",      required: true,  kind: FieldKind::Integer },
            FieldSpec { name: "account",     required: true,  kind: FieldKind::Integer },
            FieldSpec { name: "object_list", required: true,  kind: FieldKind::ObjectList },
            FieldSpec { name: "balance",     required: true,  kind: FieldKind::Decimal },
            FieldSpec { name: "quantity",    required: false, kind: FieldKind::Decimal },
        ],
        container: false,
    },
    LabelSpec {
        label: "#VER",
        description: "Verification (journal entry). Followed by a `{ ... }` brace block of `#TRANS` / `#RTRANS` / `#BTRANS` lines. \
                      The sum of transaction amounts inside the block should be zero.",
        format: "#VER series verno verdate vertext regdate sign",
        fields: &[
            FieldSpec { name: "series",  required: false, kind: FieldKind::String },
            FieldSpec { name: "verno",   required: false, kind: FieldKind::String },
            FieldSpec { name: "verdate", required: true,  kind: FieldKind::Date },
            FieldSpec { name: "vertext", required: false, kind: FieldKind::String },
            FieldSpec { name: "regdate", required: false, kind: FieldKind::Date },
            FieldSpec { name: "sign",    required: false, kind: FieldKind::String },
        ],
        container: true,
    },
    LabelSpec {
        label: "#TRANS",
        description: "Transaction inside a `#VER` block. `object_list` may be empty (`{}`).",
        format: "#TRANS account_no {object_list} amount transdate transtext quantity sign",
        fields: &[
            FieldSpec { name: "account_no",  required: true,  kind: FieldKind::Integer },
            FieldSpec { name: "object_list", required: true,  kind: FieldKind::ObjectList },
            FieldSpec { name: "amount",      required: true,  kind: FieldKind::Decimal },
            FieldSpec { name: "transdate",   required: false, kind: FieldKind::Date },
            FieldSpec { name: "transtext",   required: false, kind: FieldKind::String },
            FieldSpec { name: "quantity",    required: false, kind: FieldKind::Decimal },
            FieldSpec { name: "sign",        required: false, kind: FieldKind::String },
        ],
        container: false,
    },
    LabelSpec {
        label: "#RTRANS",
        description: "Supplementary transaction. Must appear immediately before an identical `#TRANS` line for backward compatibility.",
        format: "#RTRANS account_no {object_list} amount transdate transtext quantity sign",
        fields: &[
            FieldSpec { name: "account_no",  required: true,  kind: FieldKind::Integer },
            FieldSpec { name: "object_list", required: true,  kind: FieldKind::ObjectList },
            FieldSpec { name: "amount",      required: true,  kind: FieldKind::Decimal },
            FieldSpec { name: "transdate",   required: false, kind: FieldKind::Date },
            FieldSpec { name: "transtext",   required: false, kind: FieldKind::String },
            FieldSpec { name: "quantity",    required: false, kind: FieldKind::Decimal },
            FieldSpec { name: "sign",        required: false, kind: FieldKind::String },
        ],
        container: false,
    },
    LabelSpec {
        label: "#BTRANS",
        description: "Removed (cancelled) transaction. Only valid inside a `#VER` block.",
        format: "#BTRANS account_no {object_list} amount transdate transtext quantity sign",
        fields: &[
            FieldSpec { name: "account_no",  required: true,  kind: FieldKind::Integer },
            FieldSpec { name: "object_list", required: true,  kind: FieldKind::ObjectList },
            FieldSpec { name: "amount",      required: true,  kind: FieldKind::Decimal },
            FieldSpec { name: "transdate",   required: false, kind: FieldKind::Date },
            FieldSpec { name: "transtext",   required: false, kind: FieldKind::String },
            FieldSpec { name: "quantity",    required: false, kind: FieldKind::Decimal },
            FieldSpec { name: "sign",        required: false, kind: FieldKind::String },
        ],
        container: false,
    },

    // ---- Control totals ----
    LabelSpec {
        label: "#KSUMMA",
        description: "Control summation marker. The opening `#KSUMMA` has no argument and signals \
                      that a CRC-32 control total will appear at the end of the file. The closing \
                      `#KSUMMA` carries the control total value.",
        format: "#KSUMMA [checksum]",
        fields: &[FieldSpec { name: "checksum", required: false, kind: FieldKind::Raw }],
        container: false,
    },
];

pub fn label_info(name: &str) -> Option<&'static LabelSpec> {
    LABELS.iter().find(|s| s.label.eq_ignore_ascii_case(name))
}

pub fn all_labels() -> &'static [LabelSpec] {
    LABELS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_thirty_six_labels() {
        assert_eq!(LABELS.len(), 36);
    }

    #[test]
    fn all_labels_start_with_hash() {
        for l in LABELS {
            assert!(l.label.starts_with('#'), "{} does not start with #", l.label);
        }
    }

    #[test]
    fn label_lookup_case_insensitive() {
        assert!(label_info("#konto").is_some());
        assert!(label_info("#KONTO").is_some());
        assert!(label_info("#Konto").is_some());
        assert!(label_info("#BOGUS").is_none());
    }

    #[test]
    fn ver_is_only_container() {
        let containers: Vec<&str> = LABELS.iter().filter(|l| l.container).map(|l| l.label).collect();
        assert_eq!(containers, vec!["#VER"]);
    }
}

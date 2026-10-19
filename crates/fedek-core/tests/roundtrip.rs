//! Export → import must be lossless, and the cached values we write into the
//! exported workbook must be the engine's own numbers.

use std::path::PathBuf;

use calamine::{open_workbook_auto, Data, Reader};
use fedek_core::{evaluate, export_workbook, import_workbook};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/golden_anon.xlsx")
}

#[test]
fn export_then_import_is_lossless() {
    let course = import_workbook(&fixture()).unwrap();
    let out = std::env::temp_dir().join(format!("fedek-roundtrip-{}.xlsx", std::process::id()));
    export_workbook(&course, &out).unwrap();
    let again = import_workbook(&out).unwrap();
    std::fs::remove_file(&out).ok();
    assert_eq!(course, again);
}

#[test]
fn exported_cached_values_match_the_engine() {
    let course = import_workbook(&fixture()).unwrap();
    let res = evaluate(&course);
    let out = std::env::temp_dir().join(format!("fedek-cached-{}.xlsx", std::process::id()));
    export_workbook(&course, &out).unwrap();

    let mut wb = open_workbook_auto(&out).unwrap();
    let s1 = wb.worksheet_range("Sayfa1").unwrap();
    let num = |rg: &calamine::Range<Data>, r: u32, c: u32| match rg.get_value((r, c)) {
        Some(Data::Float(f)) => Some(*f),
        Some(Data::Int(i)) => Some(*i as f64),
        Some(Data::String(s)) => s.parse::<f64>().ok(),
        _ => None,
    };
    for (i, po) in res.program_outcomes.iter().enumerate() {
        assert_eq!(num(&s1, 1, 7 + i as u32), po.ratio, "PÇ{}", i + 1);
    }
    for (i, o) in res.outcomes.iter().enumerate() {
        assert_eq!(num(&s1, 21 + i as u32, 6), o.average, "G{}", 22 + i);
    }
    let s2 = wb.worksheet_range("Sayfa2").unwrap();
    assert_eq!(num(&s2, 5, 10), Some(36.0));
    assert_eq!(num(&s2, 8, 3), res.assessments[0].outcome_ratios[1]);
    std::fs::remove_file(&out).ok();
}

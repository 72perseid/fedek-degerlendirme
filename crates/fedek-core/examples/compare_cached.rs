//! Compare the engine against the cached formula values of any template
//! workbook (e.g. one recalculated by Excel or LibreOffice):
//! `cargo run --example compare_cached -- file.xlsx`
use calamine::{open_workbook_auto, Data, Range, Reader};
use fedek_core::{evaluate, import_workbook};
use std::path::Path;

fn cached(rg: &Range<Data>, row: u32, col: u32) -> Option<f64> {
    match rg.get_value((row, col))? {
        Data::Float(f) => Some(*f),
        Data::Int(i) => Some(*i as f64),
        _ => None,
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: compare_cached <file.xlsx>");
    let path = Path::new(&path);
    let course = import_workbook(path).expect("import");
    let res = evaluate(&course);
    let mut wb = open_workbook_auto(path).expect("open");
    let s1 = wb.worksheet_range("Sayfa1").expect("Sayfa1");
    let mut checked = 0;
    let mut bad = 0;
    let mut check = |what: String, ours: Option<f64>, excel: Option<f64>| {
        checked += 1;
        let same = match (ours, excel) {
            (None, None) => true,
            (Some(a), Some(b)) => (a - b).abs() < 1e-9,
            _ => false,
        };
        if !same {
            bad += 1;
            println!("MISMATCH {what}: engine {ours:?} vs file {excel:?}");
        }
    };
    for (i, po) in res.program_outcomes.iter().enumerate() {
        check(format!("Sayfa1 PÇ {}", po.code), po.ratio, cached(&s1, 1, 7 + i as u32));
    }
    for (i, o) in res.outcomes.iter().enumerate() {
        let r = 21 + i as u32;
        check(format!("Sayfa1 F{}", r + 1), o.weighted, cached(&s1, r, 5));
        check(format!("Sayfa1 G{}", r + 1), o.average, cached(&s1, r, 6));
        for (a, v) in o.per_assessment.iter().enumerate() {
            check(format!("Sayfa1 {}{}", fedek_core::export::col_letter(7 + a as u32), r + 1), *v, cached(&s1, r, 7 + a as u32));
        }
    }
    for (ai, ar) in res.assessments.iter().enumerate() {
        let sheet = fedek_core::export::assessment_sheet_name(ai);
        let rg = wb.worksheet_range(&sheet).expect("sheet");
        for (qi, q) in ar.questions.iter().enumerate() {
            let c = 10 + qi as u32;
            check(format!("{sheet} row4 q{}", qi + 1), q.ratio, cached(&rg, 3, c));
            if course.assessments[ai].questions[qi].outcome.is_some() {
                check(format!("{sheet} row5 q{}", qi + 1), Some(q.achieved as f64), cached(&rg, 4, c));
                check(format!("{sheet} row6 q{}", qi + 1), Some(ar.counted_students as f64), cached(&rg, 5, c));
            }
        }
        for oi in 0..course.outcomes.len() {
            check(format!("{sheet} D{}", 8 + oi), ar.outcome_ratios[oi], cached(&rg, 7 + oi as u32, 3));
        }
    }
    println!("{checked} cells checked, {bad} mismatches");
    if bad > 0 {
        std::process::exit(1);
    }
}

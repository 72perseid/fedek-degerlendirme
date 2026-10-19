//! Golden test: import the department's real (anonymised) workbook, recompute
//! everything, and require equality with the values Excel itself cached.

use std::path::PathBuf;

use calamine::{open_workbook_auto, Data, Range, Reader};
use fedek_core::{evaluate, import_workbook, CourseKind, StudentStatus};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/golden_anon.xlsx")
}

/// Excel's cached value of a formula cell; "" (from IFERROR) reads as `None`.
fn cached(rg: &Range<Data>, row: u32, col: u32) -> Option<f64> {
    match rg.get_value((row, col))? {
        Data::Float(f) => Some(*f),
        Data::Int(i) => Some(*i as f64),
        _ => None,
    }
}

#[track_caller]
fn assert_close(ours: Option<f64>, excel: Option<f64>, what: &str) {
    match (ours, excel) {
        (None, None) => {}
        (Some(a), Some(b)) => assert!((a - b).abs() < 1e-9, "{what}: ours {a} vs Excel {b}"),
        _ => panic!("{what}: ours {ours:?} vs Excel {excel:?}"),
    }
}

#[test]
fn import_reads_the_template_faithfully() {
    let c = import_workbook(&fixture()).expect("import");
    assert_eq!(c.code_and_name, "CHA001 ÇOCUK HAKLARI VE AİLE EĞİTİMİ");
    assert_eq!(c.term, "2023-2024 BAHAR");
    assert_eq!(c.kind, CourseKind::Compulsory);
    assert!(c.program.name.starts_with("EGE ÜNİVERSİTESİ"));
    assert_eq!(c.program.outcomes.len(), 8);
    assert_eq!(c.program.outcomes[7].code, "PÇ8");
    assert_eq!(c.outcomes.len(), 8);
    assert_eq!(c.outcomes[2].program_outcome.as_deref(), Some("PÇ4"));
    assert_eq!(c.outcomes[7].strength, Some(4));
    assert!(c.evaluation.starts_with("GENEL OLARAK"));

    assert_eq!(c.assessments.len(), 2, "only Sayfa2 and Sayfa3 are in use");
    let vize = &c.assessments[0];
    assert_eq!(vize.name, "ARA SINAV");
    assert_eq!(vize.questions.len(), 2);
    assert_eq!(vize.questions[0].outcome, Some(3), "SORU1 -> ÖÇ4");
    assert_eq!(vize.questions[1].outcome, Some(1), "SORU2 -> ÖÇ2");
    assert_eq!(vize.questions[0].threshold, Some(25.0));
    assert_eq!(vize.students.len(), 60);
    assert_eq!(vize.students[0].number.len(), 11, "leading zero preserved");
    assert_eq!(vize.students[27].status, Some(StudentStatus::NotSampled));
    assert_eq!(vize.students[0].scores, vec![Some(50.0), Some(45.0)]);
    assert_eq!(c.assessments[1].name, "DÖNEM SONU SINAVI");
}

#[test]
fn engine_matches_every_cached_excel_value() {
    let path = fixture();
    let course = import_workbook(&path).expect("import");
    let res = evaluate(&course);

    let mut wb = open_workbook_auto(&path).expect("open");
    let s1 = wb.worksheet_range("Sayfa1").expect("Sayfa1");

    // Row 2: PÇ ratios in H2..P2.
    for (i, po) in res.program_outcomes.iter().enumerate() {
        assert_close(po.ratio, cached(&s1, 1, 7 + i as u32), &format!("PÇ{}", i + 1));
    }
    // Rows 22..: F (weighted), G (average), H/I (per assessment).
    for (i, o) in res.outcomes.iter().enumerate() {
        let r = 21 + i as u32;
        assert_close(o.weighted, cached(&s1, r, 5), &format!("F{}", r + 1));
        assert_close(o.average, cached(&s1, r, 6), &format!("G{}", r + 1));
        assert_close(o.per_assessment[0], cached(&s1, r, 7), &format!("H{}", r + 1));
        assert_close(o.per_assessment[1], cached(&s1, r, 8), &format!("I{}", r + 1));
    }
    // Assessment sheets: rows 4/5/6 per question and column D per ÖÇ.
    for (ai, sheet) in ["Sayfa2", "Sayfa3"].iter().enumerate() {
        let rg = wb.worksheet_range(sheet).expect("sheet");
        let ar = &res.assessments[ai];
        assert_eq!(Some(ar.counted_students as f64), cached(&rg, 5, 10), "{sheet} K6");
        for (qi, q) in ar.questions.iter().enumerate() {
            let c = 10 + qi as u32;
            assert_close(q.ratio, cached(&rg, 3, c), &format!("{sheet} row4 q{}", qi + 1));
            assert_eq!(Some(q.achieved as f64), cached(&rg, 4, c), "{sheet} row5 q{}", qi + 1);
        }
        for oi in 0..course.outcomes.len() {
            assert_close(ar.outcome_ratios[oi], cached(&rg, 7 + oi as u32, 3), &format!("{sheet} D{}", 8 + oi));
        }
    }

    // Sanity on the headline numbers the department reads off Sayfa1.
    assert!((res.program_outcomes[1].ratio.unwrap() - 0.986111111111).abs() < 1e-9);
    assert_eq!(res.program_outcomes[3].ratio, Some(0.5));
    assert_eq!(res.program_outcomes[0].ratio, Some(0.0));
    assert_eq!(res.program_outcomes[2].ratio, None);
}

//! Read a workbook that follows the department's Excel template into a [`Course`].
//!
//! Cell addresses below are the template's; rows/columns in code are 0-based.

use std::path::Path;

use calamine::{open_workbook_auto, Data, Range, Reader};

use crate::model::*;

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("Excel dosyası okunamadı: {0}")]
    Xlsx(#[from] calamine::Error),
    #[error("'Sayfa1' sayfası bulunamadı; bu dosya ÖÇ-PÇ şablonuna uymuyor")]
    NoSummarySheet,
}

/// Template geometry (0-based).
pub mod layout {
    /// Sayfa1 row of "ÖÇ1" (Excel row 22).
    pub const FIRST_OUTCOME_ROW: u32 = 21;
    pub const MAX_OUTCOMES: u32 = 20;
    /// Sayfa1 row of "PÇ1" in the lookup table (Excel row 48).
    pub const FIRST_PC_ROW: u32 = 47;
    pub const MAX_PCS: u32 = 11;
    /// Assessment sheet: first question column (K) and last (BH).
    pub const FIRST_Q_COL: u32 = 10;
    pub const LAST_Q_COL: u32 = 59;
    /// Assessment sheet: student rows 15..=214.
    pub const FIRST_STUDENT_ROW: u32 = 14;
    pub const LAST_STUDENT_ROW: u32 = 213;
    pub const ROW_TAG: u32 = 1;
    pub const ROW_POINTS: u32 = 6;
    pub const ROW_THRESHOLD: u32 = 9;
    pub const COL_NUMBER: u32 = 6;
    pub const COL_FIRST: u32 = 7;
    pub const COL_LAST: u32 = 8;
    pub const COL_STATUS: u32 = 9;
}
use layout::*;

/// Trimmed, non-empty text of a cell; numbers are rendered without a
/// trailing ".0" so student numbers survive.
fn text(rg: &Range<Data>, row: u32, col: u32) -> Option<String> {
    let s = match rg.get_value((row, col))? {
        Data::String(s) => s.trim().to_string(),
        Data::Float(f) if f.fract() == 0.0 => format!("{}", *f as i64),
        Data::Float(f) => f.to_string(),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        _ => return None,
    };
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn number(rg: &Range<Data>, row: u32, col: u32) -> Option<f64> {
    match rg.get_value((row, col))? {
        Data::Float(f) => Some(*f),
        Data::Int(i) => Some(*i as f64),
        Data::String(s) => s.trim().replace(',', ".").parse().ok(),
        _ => None,
    }
}

fn normalize_code(code: &str) -> String {
    code.split_whitespace().collect::<String>().to_uppercase()
}

pub fn import_workbook(path: &Path) -> Result<Course, ImportError> {
    let mut wb = open_workbook_auto(path)?;
    let names: Vec<String> = wb.sheet_names().to_vec();
    if !names.iter().any(|n| n == "Sayfa1") {
        return Err(ImportError::NoSummarySheet);
    }
    let s1 = wb.worksheet_range("Sayfa1")?;
    let mut course = read_summary(&s1);

    for name in names.iter().filter(|n| n.as_str() != "Sayfa1") {
        let rg = wb.worksheet_range(name)?;
        // C5 holds the assessment's name; an empty C5 means the sheet is unused.
        if let Some(title) = text(&rg, 4, 2) {
            course.assessments.push(read_assessment(&rg, title));
        }
    }
    Ok(course)
}

fn read_summary(s1: &Range<Data>) -> Course {
    let mut course = Course {
        code_and_name: text(s1, 2, 3).unwrap_or_default(), // D3
        instructor: text(s1, 3, 3).unwrap_or_default(),    // D4
        term: text(s1, 4, 3).unwrap_or_default(),          // D5
        kind: match text(s1, 5, 3) {
            // D6
            Some(t) if t.to_uppercase().contains("SEÇ") => CourseKind::Elective,
            _ => CourseKind::Compulsory,
        },
        evaluation: text(s1, 7, 7).unwrap_or_default(),  // H8
        improvement: text(s1, 9, 7).unwrap_or_default(), // H10
        other_notes: text(s1, 13, 7).unwrap_or_default(), // H14
        ..Course::default()
    };
    course.program.name = text(s1, 45, 3).unwrap_or_default(); // D46
    for r in FIRST_PC_ROW..FIRST_PC_ROW + MAX_PCS {
        if let Some(code) = text(s1, r, 2) {
            course.program.outcomes.push(ProgramOutcome {
                code: normalize_code(&code),
                text: text(s1, r, 3).unwrap_or_default(),
            });
        }
    }
    // Learning outcomes are positional; keep gaps, drop trailing empties.
    let mut outcomes: Vec<LearningOutcome> = (0..MAX_OUTCOMES)
        .map(|i| {
            let r = FIRST_OUTCOME_ROW + i;
            LearningOutcome {
                text: text(s1, r, 1).unwrap_or_default(),
                program_outcome: text(s1, r, 2).map(|c| normalize_code(&c)),
                strength: number(s1, r, 4)
                    .filter(|v| (1.0..=5.0).contains(v))
                    .map(|v| v.round() as u8),
            }
        })
        .collect();
    while matches!(outcomes.last(), Some(lo) if lo.text.is_empty() && lo.program_outcome.is_none() && lo.strength.is_none())
    {
        outcomes.pop();
    }
    course.outcomes = outcomes;
    course
}

fn read_assessment(rg: &Range<Data>, name: String) -> Assessment {
    // Number of questions = last column with a tag, points or threshold.
    let question_count = (FIRST_Q_COL..=LAST_Q_COL)
        .rev()
        .find(|&c| {
            text(rg, ROW_TAG, c).is_some()
                || number(rg, ROW_POINTS, c).is_some()
                || number(rg, ROW_THRESHOLD, c).is_some()
        })
        .map_or(0, |c| (c - FIRST_Q_COL + 1) as usize);

    let questions = (0..question_count)
        .map(|q| {
            let c = FIRST_Q_COL + q as u32;
            Question {
                outcome: text(rg, ROW_TAG, c).and_then(|t| parse_outcome_code(&t)),
                points: number(rg, ROW_POINTS, c),
                threshold: number(rg, ROW_THRESHOLD, c),
            }
        })
        .collect();

    let mut students = Vec::new();
    for r in FIRST_STUDENT_ROW..=LAST_STUDENT_ROW {
        let status = text(rg, r, COL_STATUS).map(|s| s.to_uppercase());
        let s = Student {
            number: text(rg, r, COL_NUMBER).unwrap_or_default(),
            first_name: text(rg, r, COL_FIRST).unwrap_or_default(),
            last_name: text(rg, r, COL_LAST).unwrap_or_default(),
            status: match status.as_deref() {
                Some("F2") => Some(StudentStatus::Absent),
                Some("S-") => Some(StudentStatus::NotSampled),
                _ => None,
            },
            scores: (0..question_count)
                .map(|q| number(rg, r, FIRST_Q_COL + q as u32))
                .collect(),
        };
        let empty = s.number.is_empty()
            && s.first_name.is_empty()
            && s.last_name.is_empty()
            && s.status.is_none()
            && s.scores.iter().all(Option::is_none);
        if !empty {
            students.push(s);
        }
    }

    Assessment {
        name,
        questions,
        students,
    }
}

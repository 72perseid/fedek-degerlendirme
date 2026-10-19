//! Data model. One [`Course`] is one workbook: course info, the programme's
//! outcome list, the course's learning outcomes with their PÇ mapping, and
//! any number of assessments (exams, assignments, ...).

use serde::{Deserialize, Serialize};

/// A programme outcome ("PÇ1", text) as published in the Bologna package.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProgramOutcome {
    pub code: String,
    pub text: String,
}

/// The degree programme the course belongs to.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Program {
    /// Institution and programme name as printed in the template (Sayfa1!D46).
    pub name: String,
    pub outcomes: Vec<ProgramOutcome>,
    /// Bologna package page the list was taken from, if any.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bologna_url: String,
}

/// A course learning outcome. Its code is positional: the i-th entry is "ÖÇ{i+1}".
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LearningOutcome {
    pub text: String,
    /// Code of the single programme outcome this ÖÇ feeds (template rule 8).
    pub program_outcome: Option<String>,
    /// Contribution strength on the 1..5 scale (template rule 9).
    pub strength: Option<u8>,
}

/// Why a student is excluded from an assessment's denominator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StudentStatus {
    /// "F2": did not attend.
    Absent,
    /// "S-": not part of the random sample (template rule 6).
    NotSampled,
}

impl StudentStatus {
    pub fn template_code(self) -> &'static str {
        match self {
            StudentStatus::Absent => "F2",
            StudentStatus::NotSampled => "S-",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Student {
    pub number: String,
    pub first_name: String,
    pub last_name: String,
    pub status: Option<StudentStatus>,
    /// One entry per question of the assessment; `None` = no score entered.
    pub scores: Vec<Option<f64>>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Question {
    /// Index into `Course::outcomes` of the ÖÇ this question measures.
    pub outcome: Option<usize>,
    /// Points the question is worth. Informational only, like the template.
    pub points: Option<f64>,
    /// Lowest score that counts as "ÖÇ achieved". Required for the question to count.
    pub threshold: Option<f64>,
}

/// One assessment method: a sheet "SayfaN" in the template.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Assessment {
    /// "Ara Sınav", "Dönem Sonu Sınavı", "Ödev", ...
    pub name: String,
    pub questions: Vec<Question>,
    pub students: Vec<Student>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CourseKind {
    #[default]
    Compulsory,
    Elective,
}

impl CourseKind {
    pub fn template_text(self) -> &'static str {
        match self {
            CourseKind::Compulsory => "ZORUNLU",
            CourseKind::Elective => "SEÇMELİ",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Course {
    /// "CHA001 ÇOCUK HAKLARI VE AİLE EĞİTİMİ"
    pub code_and_name: String,
    pub instructor: String,
    /// "2023-2024 BAHAR"
    pub term: String,
    pub kind: CourseKind,
    pub program: Program,
    pub outcomes: Vec<LearningOutcome>,
    pub assessments: Vec<Assessment>,
    /// Free text: "DEĞERLENDİRMENİZ"
    pub evaluation: String,
    /// Free text: "İYİLEŞTİRME ÖNERİNİZ"
    pub improvement: String,
    /// Free text: "DİĞER DÜŞÜNCELERİNİZ"
    pub other_notes: String,
    /// Bologna package course page the ÖÇ list was taken from, if any.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bologna_url: String,
}

/// Positional code of a learning outcome: "ÖÇ1", "ÖÇ2", ...
pub fn outcome_code(index: usize) -> String {
    format!("ÖÇ{}", index + 1)
}

/// Parse "ÖÇ4" (any case, surrounding spaces) into index 3.
pub fn parse_outcome_code(text: &str) -> Option<usize> {
    let t = text.trim().to_uppercase();
    let digits = t.strip_prefix("ÖÇ")?;
    let n: usize = digits.trim().parse().ok()?;
    n.checked_sub(1)
}

/// Default points per question when `n` questions share 100 points.
pub fn default_points(n: usize) -> f64 {
    if n == 0 {
        0.0
    } else {
        100.0 / n as f64
    }
}

impl Assessment {
    /// Resize to `n` questions. New questions get `100/n` points and a
    /// threshold of `pass_percent` of that; existing ones keep their values.
    pub fn set_question_count(&mut self, n: usize, pass_percent: f64) {
        let points = default_points(n);
        self.questions.resize_with(n, || Question {
            outcome: None,
            points: Some(points),
            threshold: Some(points * pass_percent / 100.0),
        });
        for s in &mut self.students {
            s.scores.resize(n, None);
        }
    }

    /// Re-derive every question's points and threshold from the question
    /// count and a pass percentage (the department's usual case: 50 %).
    pub fn reset_points_and_thresholds(&mut self, pass_percent: f64) {
        let points = default_points(self.questions.len());
        for q in &mut self.questions {
            q.points = Some(points);
            q.threshold = Some(points * pass_percent / 100.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_codes_round_trip() {
        assert_eq!(outcome_code(0), "ÖÇ1");
        assert_eq!(parse_outcome_code(" öç4 "), Some(3));
        assert_eq!(parse_outcome_code("ÖK4"), None);
        assert_eq!(parse_outcome_code("ÖÇ0"), None);
    }

    #[test]
    fn question_defaults_follow_count_and_pass_percent() {
        let mut a = Assessment::default();
        a.set_question_count(4, 50.0);
        assert_eq!(a.questions[3].points, Some(25.0));
        assert_eq!(a.questions[3].threshold, Some(12.5));
        a.set_question_count(3, 60.0);
        assert_eq!(a.questions.len(), 3);
        assert_eq!(a.questions[0].points, Some(25.0), "existing questions keep their values");
        a.reset_points_and_thresholds(60.0);
        assert!((a.questions[0].points.unwrap() - 100.0 / 3.0).abs() < 1e-12);
        assert!((a.questions[0].threshold.unwrap() - 20.0).abs() < 1e-12);
    }
}

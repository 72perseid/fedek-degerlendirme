//! The calculation engine. Every rule here mirrors a formula in the Excel
//! template; the cell references are given so the two can be compared.

use crate::model::{Assessment, Course};

/// Result for one question column of an assessment sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct QuestionResult {
    /// Students scoring at or above the threshold (row 5: COUNTIF(K15:K214,">="&K10)).
    pub achieved: usize,
    /// `achieved / counted_students` (row 4), `None` when the question has no
    /// ÖÇ tag or nobody is counted.
    pub ratio: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssessmentResult {
    /// Row 6: students with a surname minus those flagged F2 or S-.
    pub counted_students: usize,
    pub questions: Vec<QuestionResult>,
    /// Column D rows 8..27: mean ratio of the questions tagged with each ÖÇ.
    pub outcome_ratios: Vec<Option<f64>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OutcomeResult {
    /// Sayfa1 columns H..P: the ÖÇ's ratio in each assessment.
    pub per_assessment: Vec<Option<f64>>,
    /// Sayfa1 column G: AVERAGE over the assessments that measured it.
    pub average: Option<f64>,
    /// Sayfa1 column F: strength × average (blank strength counts as 0).
    pub weighted: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgramOutcomeResult {
    pub code: String,
    /// Sayfa1 row 2: SUMIFS(F)/SUMIFS(E) over the ÖÇs mapped to this PÇ.
    pub ratio: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CourseResult {
    pub assessments: Vec<AssessmentResult>,
    pub outcomes: Vec<OutcomeResult>,
    pub program_outcomes: Vec<ProgramOutcomeResult>,
}

fn mean(values: impl IntoIterator<Item = f64>) -> Option<f64> {
    let (sum, n) = values
        .into_iter()
        .fold((0.0, 0usize), |(s, n), v| (s + v, n + 1));
    if n == 0 {
        None
    } else {
        Some(sum / n as f64)
    }
}

/// Row 6 of an assessment sheet:
/// `COUNTIF(Soyadı,"*") - COUNTIF(Durum,"F2") - COUNTIF(Durum,"S-")`.
pub fn counted_students(a: &Assessment) -> usize {
    let with_surname = a
        .students
        .iter()
        .filter(|s| !s.last_name.trim().is_empty())
        .count();
    let flagged = a.students.iter().filter(|s| s.status.is_some()).count();
    with_surname.saturating_sub(flagged)
}

pub fn evaluate_assessment(a: &Assessment, outcome_count: usize) -> AssessmentResult {
    let counted = counted_students(a);
    let questions: Vec<QuestionResult> = a
        .questions
        .iter()
        .enumerate()
        .map(|(qi, q)| {
            // COUNTIF with an empty threshold matches nothing.
            let achieved = match q.threshold {
                Some(t) => a
                    .students
                    .iter()
                    .filter(|s| matches!(s.scores.get(qi), Some(Some(v)) if *v >= t))
                    .count(),
                None => 0,
            };
            let ratio = if q.outcome.is_none() || counted == 0 {
                None
            } else {
                Some(achieved as f64 / counted as f64)
            };
            QuestionResult { achieved, ratio }
        })
        .collect();

    let outcome_ratios = (0..outcome_count)
        .map(|oi| {
            mean(
                a.questions
                    .iter()
                    .zip(&questions)
                    .filter(|(q, _)| q.outcome == Some(oi))
                    .filter_map(|(_, r)| r.ratio),
            )
        })
        .collect();

    AssessmentResult {
        counted_students: counted,
        questions,
        outcome_ratios,
    }
}

pub fn evaluate(course: &Course) -> CourseResult {
    let n = course.outcomes.len();
    let assessments: Vec<AssessmentResult> = course
        .assessments
        .iter()
        .map(|a| evaluate_assessment(a, n))
        .collect();

    let outcomes: Vec<OutcomeResult> = course
        .outcomes
        .iter()
        .enumerate()
        .map(|(oi, lo)| {
            let per_assessment: Vec<Option<f64>> =
                assessments.iter().map(|ar| ar.outcome_ratios[oi]).collect();
            let average = mean(per_assessment.iter().flatten().copied());
            let weighted = average.map(|g| f64::from(lo.strength.unwrap_or(0)) * g);
            OutcomeResult {
                per_assessment,
                average,
                weighted,
            }
        })
        .collect();

    let program_outcomes = course
        .program
        .outcomes
        .iter()
        .map(|po| {
            let (mut num, mut den) = (0.0, 0.0);
            for (lo, r) in course.outcomes.iter().zip(&outcomes) {
                if lo.program_outcome.as_deref() == Some(po.code.as_str()) {
                    den += f64::from(lo.strength.unwrap_or(0));
                    num += r.weighted.unwrap_or(0.0);
                }
            }
            ProgramOutcomeResult {
                code: po.code.clone(),
                ratio: if den == 0.0 { None } else { Some(num / den) },
            }
        })
        .collect();

    CourseResult {
        assessments,
        outcomes,
        program_outcomes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn student(last: &str, status: Option<StudentStatus>, scores: &[Option<f64>]) -> Student {
        Student {
            number: String::new(),
            first_name: String::new(),
            last_name: last.into(),
            status,
            scores: scores.to_vec(),
        }
    }

    fn course() -> Course {
        let mut c = Course::default();
        c.program.outcomes = vec![
            ProgramOutcome { code: "PÇ1".into(), text: String::new() },
            ProgramOutcome { code: "PÇ2".into(), text: String::new() },
        ];
        c.outcomes = vec![
            LearningOutcome { text: "a".into(), program_outcome: Some("PÇ1".into()), strength: Some(2) },
            LearningOutcome { text: "b".into(), program_outcome: Some("PÇ1".into()), strength: Some(3) },
            LearningOutcome { text: "c".into(), program_outcome: Some("PÇ2".into()), strength: Some(4) },
        ];
        c.assessments = vec![Assessment {
            name: "Vize".into(),
            questions: vec![
                Question { outcome: Some(0), points: Some(50.0), threshold: Some(25.0) },
                Question { outcome: Some(0), points: Some(50.0), threshold: Some(25.0) },
            ],
            students: vec![
                student("X", None, &[Some(30.0), Some(10.0)]),
                student("Y", None, &[Some(20.0), Some(40.0)]),
                student("Z", None, &[Some(50.0), Some(50.0)]),
                student("W", Some(StudentStatus::NotSampled), &[None, None]),
                student("V", Some(StudentStatus::Absent), &[None, None]),
            ],
        }];
        c
    }

    #[test]
    fn flagged_students_leave_the_denominator() {
        let r = evaluate(&course());
        assert_eq!(r.assessments[0].counted_students, 3);
        assert_eq!(r.assessments[0].questions[0].achieved, 2);
        assert_eq!(r.assessments[0].questions[0].ratio, Some(2.0 / 3.0));
    }

    #[test]
    fn outcome_ratio_is_the_mean_of_its_questions() {
        let r = evaluate(&course());
        // q1: 2/3, q2: 2/3 -> ÖÇ1 = 2/3
        assert!((r.outcomes[0].average.unwrap() - 2.0 / 3.0).abs() < 1e-12);
        assert!((r.outcomes[0].weighted.unwrap() - 4.0 / 3.0).abs() < 1e-12);
        assert_eq!(r.outcomes[1].average, None, "ÖÇ2 has no question");
    }

    #[test]
    fn unmeasured_outcomes_still_count_in_the_pc_denominator() {
        // Template quirk, reproduced on purpose: PÇ1 = (2·⅔ + nothing) / (2 + 3).
        let r = evaluate(&course());
        assert!((r.program_outcomes[0].ratio.unwrap() - (4.0 / 3.0) / 5.0).abs() < 1e-12);
        assert_eq!(r.program_outcomes[1].ratio, Some(0.0), "mapped but never measured -> 0");
    }

    #[test]
    fn empty_roster_yields_no_ratios() {
        let mut c = course();
        c.assessments[0].students.clear();
        let r = evaluate(&c);
        assert_eq!(r.assessments[0].questions[0].ratio, None);
        assert_eq!(r.outcomes[0].average, None);
        assert_eq!(r.program_outcomes[0].ratio, Some(0.0));
    }

    #[test]
    fn blank_strength_contributes_zero() {
        let mut c = course();
        c.outcomes[0].strength = None;
        let r = evaluate(&c);
        assert_eq!(r.outcomes[0].weighted, Some(0.0));
        assert_eq!(r.program_outcomes[0].ratio, Some(0.0 / 3.0));
    }
}

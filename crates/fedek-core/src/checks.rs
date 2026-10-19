//! Plain-language warnings about a course's data, shown on the results page.
//! None of these change the numbers; they explain them.

use crate::calc::evaluate;
use crate::model::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// Something the engine will silently treat as zero or ignore.
    Warning,
    /// Worth knowing, not necessarily wrong.
    Info,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub severity: Severity,
    pub message: String,
}

fn warn(v: &mut Vec<Check>, message: String) {
    v.push(Check { severity: Severity::Warning, message });
}

fn info(v: &mut Vec<Check>, message: String) {
    v.push(Check { severity: Severity::Info, message });
}

pub fn run_checks(course: &Course) -> Vec<Check> {
    let mut out = Vec::new();
    let res = evaluate(course);

    if course.outcomes.is_empty() {
        warn(&mut out, "Henüz öğrenme çıktısı (ÖÇ) girilmedi.".into());
    }
    for (i, lo) in course.outcomes.iter().enumerate() {
        let code = outcome_code(i);
        if lo.text.trim().is_empty() {
            warn(&mut out, format!("{code}: tanım metni boş."));
        }
        match &lo.program_outcome {
            None => warn(&mut out, format!("{code}: program çıktısı (PÇ) seçilmedi; PÇ hesabına girmez.")),
            Some(pc) if !course.program.outcomes.iter().any(|p| &p.code == pc) => {
                warn(&mut out, format!("{code}: seçili {pc} bu programın listesinde yok."))
            }
            _ => {}
        }
        if lo.strength.is_none() {
            warn(&mut out, format!("{code}: ÖÇ-PÇ ilişki düzeyi (1-5) girilmedi; 0 sayılır."));
        }
        let measured = course
            .assessments
            .iter()
            .any(|a| a.questions.iter().any(|q| q.outcome == Some(i)));
        if !measured {
            warn(&mut out, format!("{code}: hiçbir soruyla ölçülmedi; bağlı olduğu PÇ'nin hesabında 0 sayılır."));
        }
    }

    for (ai, a) in course.assessments.iter().enumerate() {
        let name = if a.name.trim().is_empty() { format!("{}. değerlendirme", ai + 1) } else { a.name.clone() };
        if a.name.trim().is_empty() {
            warn(&mut out, format!("{name}: ad girilmedi."));
        }
        if a.questions.is_empty() {
            warn(&mut out, format!("{name}: soru sayısı 0."));
            continue;
        }
        let ar = &res.assessments[ai];
        if ar.counted_students == 0 {
            warn(&mut out, format!("{name}: sayılan öğrenci yok (liste boş veya herkes F2/S- işaretli)."));
        }
        for (qi, q) in a.questions.iter().enumerate() {
            let qn = qi + 1;
            if q.outcome.is_none() {
                warn(&mut out, format!("{name}, Soru {qn}: ÖÇ seçilmedi; bu soru hesaba katılmaz."));
                continue;
            }
            if q.threshold.is_none() {
                warn(&mut out, format!("{name}, Soru {qn}: eşik puanı boş; hiçbir öğrenci sağlamış sayılmaz."));
            }
            let missing = a
                .students
                .iter()
                .filter(|s| s.status.is_none() && !s.last_name.trim().is_empty())
                .filter(|s| !matches!(s.scores.get(qi), Some(Some(_))))
                .count();
            if missing > 0 {
                warn(&mut out, format!("{name}, Soru {qn}: sayılan {missing} öğrencinin puanı boş (sağlamamış sayılır)."));
            }
            if let Some(p) = q.points {
                let over = a.students.iter().filter(|s| matches!(s.scores.get(qi), Some(Some(v)) if *v > p)).count();
                if over > 0 {
                    info(&mut out, format!("{name}, Soru {qn}: {over} öğrencinin puanı soru puanından ({p}) yüksek."));
                }
            }
        }
    }

    // Was the same sample used in every assessment?
    let samples: Vec<Vec<String>> = course
        .assessments
        .iter()
        .filter(|a| !a.students.is_empty())
        .map(|a| {
            let mut v: Vec<String> = a
                .students
                .iter()
                .filter(|s| s.status == Some(StudentStatus::NotSampled))
                .map(|s| s.number.clone())
                .collect();
            v.sort();
            v
        })
        .collect();
    if samples.len() > 1 && samples.windows(2).any(|w| w[0] != w[1]) {
        info(&mut out, "Değerlendirmelerde 'S- seçilmedi' işaretli öğrenciler aynı değil; örneklem tutarlı değil.".into());
    }

    for po in &course.program.outcomes {
        if !course.outcomes.iter().any(|lo| lo.program_outcome.as_deref() == Some(po.code.as_str())) {
            info(&mut out, format!("{}: bu derste hiçbir ÖÇ ile eşleştirilmedi.", po.code));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_course_gets_the_expected_warnings() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/golden_anon.xlsx");
        let course = crate::import_workbook(&path).unwrap();
        let checks = run_checks(&course);
        let msgs: Vec<&str> = checks.iter().map(|c| c.message.as_str()).collect();
        assert!(msgs.iter().any(|m| m.starts_with("ÖÇ1: hiçbir soruyla")));
        assert!(msgs.iter().any(|m| m.starts_with("ÖÇ3: hiçbir soruyla")));
        assert!(!msgs.iter().any(|m| m.starts_with("ÖÇ2: hiçbir soruyla")));
        assert!(msgs.iter().any(|m| m.contains("örneklem tutarlı değil")));
        assert!(msgs.iter().any(|m| m.starts_with("PÇ3:")));
    }
}

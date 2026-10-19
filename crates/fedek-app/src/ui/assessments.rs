//! Step 3: one tab per assessment; questions with their ÖÇ tag, points and
//! threshold; the student roster with scores.

use egui::{Color32, Id, RichText};
use egui_extras::{Column, TableBuilder};
use fedek_core::calc::counted_students;
use fedek_core::{outcome_code, Assessment, Student, StudentStatus};

use crate::app::{bold, App, Dialog, ACCENT};
use crate::ui::{card, danger_button, hint, num_cell, secondary_button, section, text_field};

const MAX_QUESTIONS: usize = 50;
const MAX_STUDENTS: usize = 200;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let App { open, num_edits, dialog, .. } = app;
    let Some(open) = open.as_mut() else { return };
    let pass = open.file.pass_percent;
    let course = &mut open.file.course;

    ui.heading("3. Sınavlar ve Puanlar");
    hint(ui, "Her değerlendirme (ara sınav, final, ödev…) için soruları ve öğrencilerin sorulardan aldığı puanları girin. Soru puanı 100'ün soru sayısına bölümüdür; eşik puanı, ÖÇ'yi sağlamış sayılmak için gereken en düşük puandır.");
    ui.add_space(8.0);

    // Tabs.
    ui.horizontal_wrapped(|ui| {
        for (i, a) in course.assessments.iter().enumerate() {
            let selected = i == open.assessment_tab;
            let name = if a.name.trim().is_empty() { format!("{}. değerlendirme", i + 1) } else { a.name.clone() };
            let text = if selected { RichText::new(name).color(Color32::WHITE).size(16.0) } else { RichText::new(name).size(16.0) };
            let btn = egui::Button::new(text).fill(if selected { ACCENT } else { Color32::from_gray(0xee) }).min_size(egui::vec2(0.0, 36.0));
            if ui.add(btn).clicked() {
                open.assessment_tab = i;
            }
        }
        if secondary_button(ui, "＋ Değerlendirme ekle").clicked() {
            let n = course.assessments.len();
            course.assessments.push(Assessment { name: format!("Değerlendirme {}", n + 1), ..Default::default() });
            open.assessment_tab = n;
        }
    });
    if course.assessments.is_empty() {
        ui.label(RichText::new("Henüz değerlendirme yok. “Değerlendirme ekle” ile başlayın.").color(Color32::GRAY));
        return;
    }
    let tab = open.assessment_tab.min(course.assessments.len() - 1);
    open.assessment_tab = tab;
    let outcome_texts: Vec<String> = course.outcomes.iter().map(|o| o.text.clone()).collect();
    let prev_roster: Option<Vec<Student>> = (tab > 0).then(|| course.assessments[tab - 1].students.clone());
    let a = &mut course.assessments[tab];
    ui.add_space(8.0);

    // Settings.
    card(ui, |ui| {
        egui::Grid::new(("a-grid", tab)).num_columns(2).spacing([16.0, 10.0]).show(ui, |ui| {
            ui.label("Değerlendirmenin adı");
            text_field(ui, ("a-name", tab), &mut a.name, 360.0).on_hover_text("Ara Sınav, Dönem Sonu Sınavı, Ödev, Sunum…");
            ui.end_row();
            ui.label("Soru sayısı");
            ui.horizontal(|ui| {
                let mut n = a.questions.len();
                if ui.add_enabled(n > 0, egui::Button::new("－")).clicked() { n -= 1; }
                ui.label(bold(n.to_string()).size(18.0));
                if ui.add_enabled(n < MAX_QUESTIONS, egui::Button::new("＋")).clicked() { n += 1; }
                if n != a.questions.len() {
                    a.set_question_count(n, pass);
                }
                ui.add_space(12.0);
                if secondary_button(ui, "Puan ve eşikleri yeniden hesapla").on_hover_text(format!("Her soru 100/{} puan, eşik bunun %{}'i olur.", n.max(1), pass)).clicked() {
                    a.reset_points_and_thresholds(pass);
                }
            });
            ui.end_row();
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if danger_button(ui, "Bu değerlendirmeyi sil").clicked() {
                    *dialog = Some(Dialog::ConfirmDeleteAssessment(tab));
                }
            });
        });
    });

    if a.questions.is_empty() {
        ui.add_space(8.0);
        ui.label(RichText::new("Önce soru sayısını belirleyin.").color(Color32::GRAY));
        return;
    }

    // Questions.
    section(ui, "Sorular");
    card(ui, |ui| {
        egui::Grid::new(("q-grid", tab)).num_columns(4).spacing([18.0, 8.0]).striped(true).show(ui, |ui| {
            ui.label(bold("Soru"));
            ui.label(bold("Ölçtüğü öğrenme çıktısı"));
            ui.label(bold("Soru puanı"));
            ui.label(bold("Eşik puanı"));
            ui.end_row();
            for (qi, q) in a.questions.iter_mut().enumerate() {
                ui.label(format!("Soru {}", qi + 1));
                let sel = q.outcome.map(outcome_code).unwrap_or_else(|| "seçin…".into());
                egui::ComboBox::from_id_salt(("q-oc", tab, qi)).selected_text(sel).width(130.0).show_ui(ui, |ui| {
                    if ui.selectable_label(q.outcome.is_none(), "— yok —").clicked() {
                        q.outcome = None;
                    }
                    for (oi, t) in outcome_texts.iter().enumerate() {
                        if ui.selectable_label(q.outcome == Some(oi), outcome_code(oi)).on_hover_text(t).clicked() {
                            q.outcome = Some(oi);
                        }
                    }
                });
                num_cell(ui, num_edits, Id::unique(("q-pts", tab, qi)), &mut q.points, 80.0, None);
                num_cell(ui, num_edits, Id::unique(("q-thr", tab, qi)), &mut q.threshold, 80.0, q.points);
                ui.end_row();
            }
        });
        if a.questions.iter().any(|q| q.outcome.is_none()) {
            ui.label(RichText::new("ÖÇ seçilmeyen sorular hesaba katılmaz.").size(13.0).color(Color32::DARK_GRAY));
        }
    });

    // Roster.
    section(ui, "Öğrenciler ve puanlar");
    let n_q = a.questions.len();
    for s in &mut a.students {
        if s.scores.len() != n_q {
            s.scores.resize(n_q, None);
        }
    }
    ui.horizontal_wrapped(|ui| {
        if a.students.len() < MAX_STUDENTS && secondary_button(ui, "＋ Öğrenci ekle").clicked() {
            a.students.push(Student { scores: vec![None; n_q], ..Default::default() });
        }
        if secondary_button(ui, "Listeyi yapıştır…").on_hover_text("OBYS'den kopyaladığınız numara, ad, soyad listesini yapıştırın.").clicked() {
            *dialog = Some(Dialog::PasteRoster { assessment: tab, text: String::new() });
        }
        if let Some(prev) = &prev_roster {
            if !prev.is_empty() && secondary_button(ui, "Önceki değerlendirmenin listesini al").on_hover_text("Aynı öğrencileri ve F2/S- işaretlerini alır; puanlar boş gelir.").clicked() {
                a.students = prev.iter().map(|s| Student { scores: vec![None; n_q], ..s.clone() }).collect();
            }
        }
    });
    let total = a.students.len();
    let absent = a.students.iter().filter(|s| s.status == Some(StudentStatus::Absent)).count();
    let unsampled = a.students.iter().filter(|s| s.status == Some(StudentStatus::NotSampled)).count();
    ui.label(RichText::new(format!("Listede {total} öğrenci · {unsampled} seçilmedi (S-) · {absent} devamsız (F2) · hesaba katılan: {}", counted_students(a))).size(14.0));
    ui.add_space(4.0);

    let mut remove_student: Option<usize> = None;
    let students = &mut a.students;
    let questions = &a.questions;
    egui::ScrollArea::horizontal().id_salt(("roster-scroll", tab)).show(ui, |ui| {
    TableBuilder::new(ui)
        .id_salt(("roster", tab))
        .striped(true)
        .vscroll(false)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::exact(40.0))
        .column(Column::exact(160.0))
        .column(Column::exact(150.0))
        .column(Column::exact(170.0))
        .column(Column::exact(150.0))
        .columns(Column::exact(78.0), n_q)
        .column(Column::exact(44.0))
        .header(30.0, |mut h| {
            for t in ["Sıra", "Öğrenci no", "Adı", "Soyadı", "Durum"] {
                h.col(|ui| { ui.label(bold(t)); });
            }
            for (qi, q) in questions.iter().enumerate() {
                h.col(|ui| {
                    let tag = q.outcome.map(outcome_code).unwrap_or_else(|| "ÖÇ yok".into());
                    ui.label(bold(format!("S{}", qi + 1))).on_hover_text(format!("{tag} · puan {}", q.points.map(crate::ui::fmt_num).unwrap_or_default()));
                });
            }
            h.col(|_| {});
        })
        .body(|body| {
            body.rows(32.0, students.len(), |mut row| {
                let i = row.index();
                let s = &mut students[i];
                let flagged = s.status.is_some();
                row.col(|ui| { ui.label(RichText::new((i + 1).to_string()).color(Color32::GRAY)); });
                row.col(|ui| { text_field(ui, ("st-no", tab, i), &mut s.number, 148.0); });
                row.col(|ui| { text_field(ui, ("st-first", tab, i), &mut s.first_name, 138.0); });
                row.col(|ui| { text_field(ui, ("st-last", tab, i), &mut s.last_name, 158.0); });
                row.col(|ui| {
                    let label = match s.status {
                        None => "sayılır",
                        Some(StudentStatus::Absent) => "F2 · devamsız",
                        Some(StudentStatus::NotSampled) => "S- · seçilmedi",
                    };
                    let before = s.status;
                    egui::ComboBox::from_id_salt(("st-status", tab, i)).selected_text(label).width(135.0).show_ui(ui, |ui| {
                        ui.selectable_value(&mut s.status, None, "sayılır");
                        ui.selectable_value(&mut s.status, Some(StudentStatus::NotSampled), "S- · seçilmedi (örneklem dışı)");
                        ui.selectable_value(&mut s.status, Some(StudentStatus::Absent), "F2 · devamsız");
                    });
                    if s.status != before && s.status.is_some() {
                        for v in &mut s.scores { *v = None; }
                    }
                });
                for qi in 0..n_q {
                    row.col(|ui| {
                        if flagged {
                            ui.label(RichText::new("—").color(Color32::GRAY));
                        } else {
                            num_cell(ui, num_edits, Id::unique(("score", tab, i, qi)), &mut s.scores[qi], 66.0, questions[qi].points);
                        }
                    });
                }
                row.col(|ui| {
                    if ui.add(egui::Button::new(RichText::new("✕").color(crate::app::WARN_RED)).frame(false)).on_hover_text("Bu satırı sil").clicked() {
                        remove_student = Some(i);
                    }
                });
            });
        });
    });
    if let Some(i) = remove_student {
        a.students.remove(i);
    }
    if a.students.is_empty() {
        ui.label(RichText::new("Liste boş. Öğrenci ekleyin veya listeyi yapıştırın.").color(Color32::GRAY));
    }
}

/// Parse lines like "07230000913\tSUDE\tKİY", "1 07230000913 SELİN SUDE SATIR"
/// or "AYŞE YILMAZ" into students. The last word is the surname.
pub fn parse_roster(text: &str, question_count: usize) -> Vec<Student> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut tokens: Vec<String> = if line.contains('\t') {
            line.split('\t').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
        } else if line.contains(';') {
            line.split(';').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
        } else {
            line.split_whitespace().map(str::to_string).collect()
        };
        // Drop a leading row index like "1" or "1.".
        if tokens.len() > 2 && tokens[0].trim_end_matches('.').chars().all(|c| c.is_ascii_digit()) && tokens[0].len() <= 3 {
            tokens.remove(0);
        }
        let mut number = String::new();
        if let Some(pos) = tokens.iter().position(|t| t.len() >= 4 && t.chars().all(|c| c.is_ascii_digit())) {
            number = tokens.remove(pos);
        }
        let (first, last) = match tokens.len() {
            0 => (String::new(), String::new()),
            1 => (tokens[0].clone(), String::new()),
            n => (tokens[..n - 1].join(" "), tokens[n - 1].clone()),
        };
        if number.is_empty() && first.is_empty() {
            continue;
        }
        out.push(Student { number, first_name: first, last_name: last, status: None, scores: vec![None; question_count] });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tab_separated_obys_export() {
        let s = parse_roster("07230000913\tSUDE\tKİY\n07230000873\tSELİN SUDE\tSATIR\n", 2);
        assert_eq!(s.len(), 2);
        assert_eq!(s[1].first_name, "SELİN SUDE");
        assert_eq!(s[1].last_name, "SATIR");
        assert_eq!(s[1].scores, vec![None, None]);
    }

    #[test]
    fn parses_space_separated_with_row_index() {
        let s = parse_roster("1 07230000913 SUDE KİY\n2. 07230000857 ŞAFAK AYDIN KIRIKTAŞ\nAYŞE YILMAZ", 1);
        assert_eq!(s[0].number, "07230000913");
        assert_eq!(s[1].first_name, "ŞAFAK AYDIN");
        assert_eq!(s[1].last_name, "KIRIKTAŞ");
        assert_eq!(s[2].number, "");
        assert_eq!(s[2].last_name, "YILMAZ");
    }
}

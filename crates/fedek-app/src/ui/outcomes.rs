use egui::{Color32, RichText};
use fedek_core::{outcome_code, LearningOutcome};

use crate::app::{bold, App, WARN_RED};
use crate::ui::{card, danger_button, hint, secondary_button, strength_label, text_area};

const MAX_OUTCOMES: usize = 20;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let Some(open) = app.open.as_mut() else { return };
    let course = &mut open.file.course;

    ui.heading("2. Öğrenme Çıktıları (ÖÇ)");
    hint(ui, "Dersin öğrenme çıktılarını Bologna bilgi paketindeki sırayla yazın. Her ÖÇ için, öğretim elemanının matrisinde en güçlü ilişkiyi verdiği tek bir program çıktısını (PÇ) ve ilişki düzeyini (1-5) seçin.");
    ui.add_space(8.0);

    let pcs = course.program.outcomes.clone();
    let mut remove: Option<usize> = None;
    let mut move_up: Option<usize> = None;

    for i in 0..course.outcomes.len() {
        let lo: &mut LearningOutcome = &mut course.outcomes[i];
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(bold(outcome_code(i)).size(18.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if danger_button(ui, "Sil").clicked() {
                        remove = Some(i);
                    }
                    if i > 0 && secondary_button(ui, "▲ Yukarı").clicked() {
                        move_up = Some(i);
                    }
                });
            });
            ui.label("Tanım");
            text_area(ui, ("oc-text", i), &mut lo.text, 2);
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Program çıktısı (PÇ):");
                let selected = lo.program_outcome.clone().unwrap_or_else(|| "seçin…".into());
                let missing = lo.program_outcome.as_ref().map_or(false, |c| !pcs.iter().any(|p| &p.code == c));
                egui::ComboBox::from_id_salt(("oc-pc", i))
                    .selected_text(if missing { format!("{selected} (listede yok)") } else { selected })
                    .width(150.0)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(lo.program_outcome.is_none(), "— yok —").clicked() {
                            lo.program_outcome = None;
                        }
                        for p in &pcs {
                            let is = lo.program_outcome.as_deref() == Some(p.code.as_str());
                            if ui.selectable_label(is, &p.code).on_hover_text(&p.text).clicked() {
                                lo.program_outcome = Some(p.code.clone());
                            }
                        }
                    });
                ui.add_space(14.0);
                ui.label("İlişki düzeyi:");
                egui::ComboBox::from_id_salt(("oc-strength", i))
                    .selected_text(lo.strength.map_or("seçin…".to_string(), |s| strength_label(s).to_string()))
                    .width(170.0)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(lo.strength.is_none(), "— boş —").clicked() {
                            lo.strength = None;
                        }
                        for s in 1..=5u8 {
                            if ui.selectable_label(lo.strength == Some(s), strength_label(s)).clicked() {
                                lo.strength = Some(s);
                            }
                        }
                    });
            });
            if let Some(p) = lo.program_outcome.as_ref().and_then(|c| pcs.iter().find(|p| &p.code == c)) {
                ui.label(RichText::new(&p.text).size(13.0).color(Color32::DARK_GRAY));
            }
            if lo.strength.map_or(false, |s| s < 3) {
                ui.label(RichText::new("Şablon kuralı 9: ÖÇ'nin 3, 4 veya 5 düzeyinde ilişkili olduğu bir PÇ ile eşleştirilmesi tercih edilir.").size(13.0).color(WARN_RED));
            }
        });
        ui.add_space(6.0);
    }

    if let Some(i) = remove {
        course.outcomes.remove(i);
        // Questions pointing at later outcomes shift down; the removed one is untagged.
        for a in &mut course.assessments {
            for q in &mut a.questions {
                q.outcome = match q.outcome {
                    Some(x) if x == i => None,
                    Some(x) if x > i => Some(x - 1),
                    other => other,
                };
            }
        }
    }
    if let Some(i) = move_up {
        course.outcomes.swap(i, i - 1);
        for a in &mut course.assessments {
            for q in &mut a.questions {
                q.outcome = match q.outcome {
                    Some(x) if x == i => Some(i - 1),
                    Some(x) if x == i - 1 => Some(i),
                    other => other,
                };
            }
        }
    }

    ui.add_space(4.0);
    if course.outcomes.len() < MAX_OUTCOMES {
        if secondary_button(ui, "＋ Öğrenme çıktısı ekle").clicked() {
            course.outcomes.push(LearningOutcome::default());
        }
    } else {
        ui.label(RichText::new("Şablon en fazla 20 öğrenme çıktısı alır.").color(Color32::GRAY));
    }

    ui.add_space(12.0);
    egui::CollapsingHeader::new("Program çıktıları listesi (başvuru için)").show(ui, |ui| {
        for p in &pcs {
            ui.horizontal_wrapped(|ui| {
                ui.label(bold(&p.code));
                ui.label(&p.text);
            });
        }
    });
}

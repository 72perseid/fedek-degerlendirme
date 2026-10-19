//! Step 4: the numbers the template produces, the data warnings, and the
//! instructor's three free-text boxes.

use egui::{Color32, RichText};
use fedek_core::{evaluate, outcome_code, run_checks, Severity};

use crate::app::{bold, App, OK_GREEN, WARN_RED};
use crate::ui::{card, hint, pct, primary_button, section, text_area, wrapped};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let mut export = false;
    let mut pdf = false;
    let mut report_students = app.settings.report_students;
    {
        let Some(open) = app.open.as_mut() else { return };
        let course = &mut open.file.course;
        let res = evaluate(course);
        let checks = run_checks(course);

        ui.heading("4. Sonuçlar ve Rapor");
        hint(ui, "Excel şablonunun Sayfa1'inde çıkan değerler. Yüzdeler, eşiği geçen öğrencilerin hesaba katılan öğrencilere oranıdır.");
        ui.add_space(8.0);

        section(ui, "Kontroller");
        card(ui, |ui| {
            if checks.is_empty() {
                ui.label(RichText::new("✔ Veride sorun görünmüyor.").color(OK_GREEN));
            }
            for c in &checks {
                let (icon, color) = match c.severity {
                    Severity::Warning => ("⚠", WARN_RED),
                    Severity::Info => ("ℹ", Color32::DARK_GRAY),
                };
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(icon).color(color));
                    ui.label(RichText::new(&c.message).color(color));
                });
            }
        });

        section(ui, "Dersin sağladığı program çıktıları (PÇ)");
        card(ui, |ui| {
            egui::Grid::new("pc-grid").num_columns(3).spacing([18.0, 8.0]).striped(true).show(ui, |ui| {
                ui.label(bold("PÇ"));
                ui.label(bold("Tanım"));
                ui.label(bold("Yüzde"));
                ui.end_row();
                for (po, r) in course.program.outcomes.iter().zip(&res.program_outcomes) {
                    ui.label(bold(&po.code));
                    wrapped(ui, 620.0, RichText::new(&po.text).size(14.0));
                    ui.label(bold(pct(r.ratio)).size(17.0));
                    ui.end_row();
                }
            });
            ui.label(RichText::new("PÇ yüzdesi = Σ(ilişki düzeyi × ÖÇ ortalaması) / Σ(ilişki düzeyi), o PÇ'ye bağlı ÖÇ'ler üzerinden. Ölçülmeyen ÖÇ'ler paya 0 katar.").size(13.0).color(Color32::DARK_GRAY));
        });

        section(ui, "Öğrenme çıktıları (ÖÇ)");
        card(ui, |ui| {
            let n_a = course.assessments.len();
            egui::ScrollArea::horizontal().id_salt("oc-grid-scroll").show(ui, |ui| {
            egui::Grid::new("oc-grid").num_columns(6 + n_a).spacing([14.0, 8.0]).striped(true).show(ui, |ui| {
                ui.label(bold("ÖÇ"));
                ui.label(bold("Tanım"));
                ui.label(bold("PÇ"));
                ui.label(bold("Düzey"));
                for a in &course.assessments {
                    wrapped(ui, 110.0, bold(if a.name.is_empty() { "?" } else { a.name.as_str() }).size(14.0));
                }
                ui.label(bold("ÖÇ ort."));
                ui.label(bold("% PÇ"));
                ui.end_row();
                for (i, (lo, r)) in course.outcomes.iter().zip(&res.outcomes).enumerate() {
                    ui.label(bold(outcome_code(i)));
                    wrapped(ui, 320.0, RichText::new(&lo.text).size(14.0));
                    ui.label(lo.program_outcome.clone().unwrap_or_else(|| "–".into()));
                    ui.label(lo.strength.map_or("–".to_string(), |s| s.to_string()));
                    for v in &r.per_assessment {
                        ui.label(pct(*v));
                    }
                    ui.label(bold(pct(r.average)));
                    ui.label(r.weighted.map_or("–".to_string(), |w| format!("{:.2}", w).replace('.', ",")));
                    ui.end_row();
                }
            });
            });
        });

        section(ui, "Değerlendirmeniz");
        card(ui, |ui| {
            ui.label("Dersin başarısı hakkındaki değerlendirmeniz");
            text_area(ui, "evaluation", &mut course.evaluation, 3);
            ui.add_space(6.0);
            ui.label("İyileştirme öneriniz");
            text_area(ui, "improvement", &mut course.improvement, 3);
            ui.add_space(6.0);
            ui.label("Diğer düşünceleriniz");
            text_area(ui, "other", &mut course.other_notes, 3);
        });

        section(ui, "Çıktı al");
        card(ui, |ui| {
            ui.horizontal(|ui| {
                if primary_button(ui, "Rapor (PDF) – yazdırılabilir").clicked() {
                    pdf = true;
                }
                ui.label(RichText::new("Okunaklı A4 rapor: ders bilgileri, PÇ ve ÖÇ tabloları, soru tabloları, değerlendirme metinleri.").size(13.0).color(Color32::DARK_GRAY));
            });
            ui.checkbox(&mut report_students, "Öğrenci puan listelerini rapora ek olarak koy");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if primary_button(ui, "Excel'e Aktar (FEDEK şablonu)").clicked() {
                    export = true;
                }
                ui.label(RichText::new("Şablonla aynı düzende, formülleri çalışır bir .xlsx dosyası üretir.").size(13.0).color(Color32::DARK_GRAY));
            });
        });
    }
    if report_students != app.settings.report_students {
        app.settings.report_students = report_students;
        let _ = app.storage.save_settings(&app.settings);
    }
    if export {
        app.export_excel();
    }
    if pdf {
        app.export_pdf();
    }
}

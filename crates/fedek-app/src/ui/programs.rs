//! Programme outcome (PÇ) lists, one per degree programme.

use egui::{Color32, RichText};
use fedek_core::{Program, ProgramOutcome};

use crate::app::{bold, App, Dialog, FetchTarget, Screen, ACCENT};
use crate::ui::{card, danger_button, hint, primary_button, secondary_button, text_area, text_field};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    egui::Panel::top("programs-top").show(ui, |ui| {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if secondary_button(ui, "◀  Ana Sayfa").clicked() {
                app.screen = Screen::Home;
            }
            ui.add_space(8.0);
            ui.heading("Program Çıktıları (PÇ) Listeleri");
        });
        ui.add_space(6.0);
    });

    let mut fetch: Option<(String, usize)> = None;
    let App { programs, selected_program, dialog, .. } = app;
    if programs.is_empty() {
        programs.push(crate::storage::protohistorya_program());
    }
    if *selected_program >= programs.len() {
        *selected_program = 0;
    }

    egui::Panel::left("programs-list").resizable(false).exact_size(320.0).show(ui, |ui| {
        ui.add_space(10.0);
        hint(ui, "Her bölüm/program için Bologna bilgi paketindeki PÇ listesi. Yeni dersler bu listelerden birini kullanır.");
        ui.add_space(8.0);
        for i in 0..programs.len() {
            let selected = i == *selected_program;
            let name = if programs[i].name.trim().is_empty() { "(adsız program)".to_string() } else { programs[i].name.clone() };
            let text = if selected { RichText::new(name).color(Color32::WHITE) } else { RichText::new(name) };
            if ui.add(egui::Button::new(text).fill(if selected { ACCENT } else { Color32::TRANSPARENT }).min_size(egui::vec2(300.0, 40.0)).wrap()).clicked() {
                *selected_program = i;
            }
        }
        ui.add_space(8.0);
        if secondary_button(ui, "＋ Yeni program").clicked() {
            programs.push(Program { name: "Yeni Program".into(), ..Default::default() });
            *selected_program = programs.len() - 1;
        }
    });

    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let idx = *selected_program;
            let p = &mut programs[idx];
            ui.add_space(6.0);
            card(ui, |ui| {
                ui.label("Kurum ve program adı (şablonda Sayfa1!D46 hücresine yazılır)");
                text_field(ui, ("prog-name", idx), &mut p.name, 700.0);
                ui.add_space(8.0);
                ui.label("Bologna bilgi paketi program sayfası");
                ui.horizontal(|ui| {
                    text_field(ui, ("prog-url", idx), &mut p.bologna_url, 560.0)
                        .on_hover_text("Örnek: https://ebp.ege.edu.tr/DereceProgramlari/Detay/1/114/2709/932001");
                    if primary_button(ui, "PÇ listesini bağlantıdan al").clicked() {
                        fetch = Some((p.bologna_url.clone(), idx));
                    }
                });
                ui.label(RichText::new("Program sayfasının (veya programa ait bir ders sayfasının) bağlantısı yeterlidir; liste uygulanmadan önce gösterilir.").size(13.0).color(Color32::DARK_GRAY));
            });
            ui.add_space(8.0);
            ui.label(bold("Program çıktıları").size(19.0));
            let mut remove: Option<usize> = None;
            for (i, po) in p.outcomes.iter_mut().enumerate() {
                card(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Kod");
                        text_field(ui, ("pc-code", idx, i), &mut po.code, 90.0);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if danger_button(ui, "Sil").clicked() {
                                remove = Some(i);
                            }
                        });
                    });
                    text_area(ui, ("pc-text", idx, i), &mut po.text, 2);
                });
                ui.add_space(4.0);
            }
            if let Some(i) = remove {
                p.outcomes.remove(i);
            }
            if secondary_button(ui, "＋ PÇ ekle").clicked() {
                let n = p.outcomes.len() + 1;
                p.outcomes.push(ProgramOutcome { code: format!("PÇ{n}"), text: String::new() });
            }
            ui.add_space(16.0);
            if programs.len() > 1 && danger_button(ui, "Bu programı sil").clicked() {
                *dialog = Some(Dialog::ConfirmDeleteProgram(idx));
            }
            ui.add_space(8.0);
            ui.label(RichText::new("Not: Bir programı değiştirmek daha önce açılmış dersleri etkilemez; her ders kendi PÇ listesinin kopyasını saklar.").size(13.0).color(Color32::DARK_GRAY));
        });
    });

    if let Some((url, idx)) = fetch {
        app.start_fetch(&url, FetchTarget::Program(idx));
    }
}

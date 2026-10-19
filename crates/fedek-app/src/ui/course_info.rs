use egui::RichText;
use fedek_core::CourseKind;

use crate::app::{App, FetchTarget};
use crate::ui::{card, hint, primary_button, section, text_field};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let programs = app.programs.clone();
    let mut fetch_url: Option<String> = None;
    let Some(open) = app.open.as_mut() else { return };
    let f = &mut open.file;

    ui.heading("1. Ders Bilgileri");
    hint(ui, "Excel şablonunun en üstündeki ders bilgileri. Bologna bilgi paketindeki ders sayfasının bağlantısını yapıştırırsanız ders adı, sorumlusu, ÖÇ'ler ve PÇ ilişki matrisi otomatik doldurulur.");
    ui.add_space(8.0);

    card(ui, |ui| {
        ui.label(bold_label("Bologna bilgi paketi ders sayfası"));
        ui.horizontal(|ui| {
            text_field(ui, "bologna_url", &mut f.course.bologna_url, 640.0)
                .on_hover_text("Örnek: https://ebp.ege.edu.tr/DereceProgramlari/Ders/1/2709/296397/788026/1/2026");
            if primary_button(ui, "Bağlantıdan doldur").clicked() {
                fetch_url = Some(f.course.bologna_url.clone());
            }
        });
        ui.label(RichText::new("Ders kodu/adı, sorumlusu, öğrenme çıktıları ve ÖÇ-PÇ ilişki matrisi indirilir; uygulamadan önce önizleme gösterilir.").size(13.0).color(egui::Color32::DARK_GRAY));
    });
    ui.add_space(8.0);

    card(ui, |ui| {
        egui::Grid::new("info-grid").num_columns(2).spacing([16.0, 12.0]).show(ui, |ui| {
            ui.label("Dersin kodu ve adı");
            text_field(ui, "code_name", &mut f.course.code_and_name, 520.0)
                .on_hover_text("Örnek: CHA001 ÇOCUK HAKLARI VE AİLE EĞİTİMİ");
            ui.end_row();
            ui.label("Dersin sorumlusu");
            text_field(ui, "instructor", &mut f.course.instructor, 520.0);
            ui.end_row();
            ui.label("Dönemi");
            text_field(ui, "term", &mut f.course.term, 520.0).on_hover_text("Örnek: 2023-2024 BAHAR");
            ui.end_row();
            ui.label("Ders türü");
            ui.horizontal(|ui| {
                ui.radio_value(&mut f.course.kind, CourseKind::Compulsory, "Zorunlu");
                ui.radio_value(&mut f.course.kind, CourseKind::Elective, "Seçmeli");
            });
            ui.end_row();
        });
    });

    section(ui, "Program");
    card(ui, |ui| {
        ui.label(RichText::new(&f.course.program.name).strong());
        ui.label(RichText::new(format!("{} program çıktısı (PÇ)", f.course.program.outcomes.len())).small());
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Programı değiştir:");
            let mut chosen: Option<usize> = None;
            egui::ComboBox::from_id_salt("program-pick")
                .selected_text("seçin…")
                .width(420.0)
                .show_ui(ui, |ui| {
                    for (i, p) in programs.iter().enumerate() {
                        if ui.selectable_label(false, &p.name).clicked() {
                            chosen = Some(i);
                        }
                    }
                });
            if let Some(i) = chosen {
                f.course.program = programs[i].clone();
            }
        });
        ui.label(RichText::new("PÇ listeleri ana sayfadaki “Program Çıktıları (PÇ)” ekranından düzenlenir. Programı değiştirmek bu dersin PÇ listesini yeniler; ÖÇ eşleştirmelerini kontrol edin.").small());
    });

    section(ui, "Eşik (geçme) yüzdesi");
    card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("Bir sorudan ÖÇ'yi sağlamış sayılmak için gereken puan yüzdesi:");
            ui.add(egui::DragValue::new(&mut f.pass_percent).range(0.0..=100.0).speed(1.0).suffix(" %"));
        });
        ui.label(RichText::new("Yeni eklenen sorular için eşik puanı bu yüzdeden hesaplanır (ör. 4 soru → 25 puan → eşik 12,5). Her sorunun eşiğini sınav ekranında ayrıca değiştirebilirsiniz.").small());
    });

    if let Some(url) = fetch_url {
        app.start_fetch(&url, FetchTarget::OpenCourse);
    }
}

fn bold_label(text: &str) -> RichText {
    crate::app::bold(text).size(16.0)
}

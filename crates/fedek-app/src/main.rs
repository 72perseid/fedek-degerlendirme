#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! FEDEK Ders Değerlendirme: a desktop tool that replaces the department's
//! ÖÇ-PÇ Excel template with guided screens, and still exports that template.

mod app;
mod storage;
mod ui;

/// Developer switches: `--open-last` opens the most recent course,
/// `--step info|outcomes|assessments|results` picks the step, `--programs`
/// opens the PÇ screen, `--bologna <url>` starts a new course from that
/// Bologna page. Handy for screenshots and bug reports.
fn parse_args() -> app::StartUp {
    let mut s = app::StartUp::default();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--open-last" => s.open_last = true,
            "--programs" => s.programs = true,
            "--bologna" => s.bologna_url = args.next(),
            "--step" => {
                s.step = match args.next().as_deref() {
                    Some("outcomes") => Some(app::Step::Outcomes),
                    Some("assessments") => Some(app::Step::Assessments),
                    Some("results") => Some(app::Step::Results),
                    Some(_) => Some(app::Step::Info),
                    None => None,
                }
            }
            _ => {}
        }
    }
    s
}

fn main() -> eframe::Result {
    let startup = parse_args();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("FEDEK Ders Değerlendirme")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([900.0, 600.0])
            .with_maximized(true),
        ..Default::default()
    };
    eframe::run_native(
        "FEDEK Ders Değerlendirme",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, startup)))),
    )
}

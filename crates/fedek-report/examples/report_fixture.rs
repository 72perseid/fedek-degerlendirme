//! Render a template workbook to PDF: `cargo run -p fedek-report --example report_fixture -- in.xlsx out.pdf`
fn main() {
    let mut args = std::env::args().skip(1);
    let input = args.next().expect("usage: report_fixture <in.xlsx> <out.pdf>");
    let output = args.next().expect("usage: report_fixture <in.xlsx> <out.pdf>");
    let course = fedek_core::import_workbook(std::path::Path::new(&input)).expect("import");
    let opts = fedek_report::ReportOptions { pass_percent: Some(50.0), include_students: true };
    let (pages, pdf) = fedek_report::compile(&course, &opts).expect("report");
    std::fs::write(&output, pdf).expect("write");
    println!("{pages} sayfa → {output}");
}

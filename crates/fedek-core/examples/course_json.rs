//! Print a template workbook as the JSON the app stores: `cargo run --example course_json -- file.xlsx`
fn main() {
    let path = std::env::args().nth(1).expect("usage: course_json <file.xlsx>");
    let course = fedek_core::import_workbook(std::path::Path::new(&path)).expect("import");
    println!("{}", serde_json::to_string_pretty(&course).unwrap());
}

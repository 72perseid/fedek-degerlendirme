//! Export the golden fixture through the engine: `cargo run --example export_fixture -- out.xlsx`
use std::path::PathBuf;

fn main() {
    let out = std::env::args().nth(1).expect("usage: export_fixture <out.xlsx>");
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/golden_anon.xlsx");
    let course = fedek_core::import_workbook(&fixture).expect("import fixture");
    fedek_core::export_workbook(&course, std::path::Path::new(&out)).expect("export");
    println!("wrote {out}");
}

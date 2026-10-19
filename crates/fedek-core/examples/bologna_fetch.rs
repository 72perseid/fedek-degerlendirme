//! Download and parse a Bologna page: `cargo run --example bologna_fetch -- <url>`
fn main() {
    let url = std::env::args().nth(1).expect("usage: bologna_fetch <url>");
    match fedek_core::fetch_page(&url) {
        Ok(fedek_core::Page::Course(c)) => {
            println!("{} {} | {}", c.code, c.name, c.instructor);
            println!("{} ÖÇ, {} PÇ", c.outcomes.len(), c.program_outcomes.len());
            for (i, row) in c.matrix.iter().enumerate() {
                let s = fedek_core::suggest(row);
                println!("ÖÇ{} {:?} -> {:?} düzey {:?} tied {:?}", i + 1, row, s.program_outcome.map(|j| j + 1), s.strength, s.tied);
            }
        }
        Ok(fedek_core::Page::Program(p)) => {
            println!("{}", p.name);
            for po in p.outcomes {
                println!("{} {}", po.code, po.text);
            }
        }
        Err(e) => {
            eprintln!("HATA: {e}");
            std::process::exit(1);
        }
    }
}

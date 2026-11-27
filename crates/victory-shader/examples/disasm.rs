//! Prints the GFX10 code of the native programs in a shader container:
//! `cargo run -p victory-shader --example disasm -- <file.cdxil>`.
fn main() {
    let path = std::env::args().nth(1).expect("a .cdxil file");
    let data = std::fs::read(&path).unwrap();
    let container = victory_shader::Container::parse(&data).unwrap();
    for part in &container.parts {
        let Ok(programs) = victory_shader::native_programs(part.data) else { continue };
        for p in programs {
            println!("; {:?} wave64={} vgprs={} code={} bytes", p.stage, p.wave64, p.vgprs, p.code.len());
            for i in victory_shader::gfx10::decode(&p.code).unwrap() {
                println!("{:6x}: {}", i.offset, i.text(p.wave64));
            }
        }
    }
}

//! Translates a shader container with the stage loop limits and with one of
//! its own, writing the DXIL of each: `cargo run -p victory-shader
//! --example translate_limit -- <file.cdxil> <limit> <out-prefix>`.
fn main() {
    let mut args = std::env::args().skip(1);
    let (path, limit, out) = (args.next().unwrap(), args.next().unwrap().parse::<u32>().unwrap(), args.next().unwrap());
    let data = std::fs::read(path).unwrap();
    for (name, own) in [("default", None), ("own", Some(limit))] {
        let t = victory_shader::emit::with_loop_limit(own, || victory_shader::emit::translate(&data)).unwrap();
        std::fs::write(format!("{out}.{name}.dxil"), &t.dxil).unwrap();
        println!("{name}: {} bytes of DXIL", t.dxil.len());
    }
}

//! Turns sdf_font.h (made by tools/sdf-font.py, the same file the C games
//! include) into Rust.
use std::{env, fs, path::Path};

fn main() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("sdf_font.h");
    println!("cargo:rerun-if-changed={}", src.display());
    let h = fs::read_to_string(&src).expect("read sdf_font.h");
    let define = |name: &str| -> String {
        let line = h.lines().find(|l| l.starts_with(&format!("#define {name} "))).expect(name);
        line.split_whitespace().nth(2).unwrap().trim_end_matches('f').to_string()
    };
    let chars_line = h.lines().find(|l| l.contains("SDF_CHARS[] = ")).expect("SDF_CHARS");
    let chars = &chars_line[chars_line.find('"').unwrap() + 1..chars_line.rfind('"').unwrap()];
    let glyph_start = h.find("SDF_GLYPHS[] = {").expect("SDF_GLYPHS");
    let glyph_end = glyph_start + h[glyph_start..].find("};").unwrap();
    let mut glyphs = String::new();
    for line in h[glyph_start..glyph_end].lines().skip(1) {
        let t = line.trim().trim_start_matches('{').trim_end_matches(',').trim_end_matches('}');
        let v: Vec<&str> = t.split(',').map(|s| s.trim().trim_end_matches('f')).collect();
        if v.len() != 6 {
            continue;
        }
        glyphs.push_str(&format!(
            "    Glyph {{ off: {}, w: {}, h: {}, left: {:?}, adv: {:?}, top: {:?} }},\n",
            v[0], v[1], v[2],
            v[3].parse::<f32>().unwrap(), v[4].parse::<f32>().unwrap(), v[5].parse::<f32>().unwrap()
        ));
    }
    let data_start = h.find("SDF_DATA[").expect("SDF_DATA");
    let body = &h[data_start + h[data_start..].find('{').unwrap() + 1..];
    let body = &body[..body.find("};").unwrap()];
    let data: Vec<u8> = body.split(',').map(str::trim).filter(|s| !s.is_empty()).map(|s| s.parse().unwrap()).collect();
    let out = Path::new(&env::var("OUT_DIR").unwrap()).to_path_buf();
    fs::write(out.join("sdf_data.bin"), &data).unwrap();
    let rs = format!(
        "pub const SDF_EM: f32 = {em:?};\npub const SDF_SPREAD: f32 = {spread:?};\npub const SDF_CHARS: &[u8] = b{chars:?};\npub static SDF_DATA: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/sdf_data.bin\"));\npub static SDF_GLYPHS: &[Glyph] = &[\n{glyphs}];\n",
        em = define("SDF_EM").parse::<f32>().unwrap(),
        spread = define("SDF_SPREAD").parse::<f32>().unwrap(),
        chars = chars,
    );
    fs::write(out.join("sdf_font.rs"), rs).unwrap();
}

//! Read and write: chain STEP, BRep text, and BRep binary round-trips with progressive rotation.

use cadrum::{Solid, Transform};
use glam::DVec3;
use std::f64::consts::FRAC_PI_8;

fn main() -> Result<(), cadrum::Error> {
    let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();
    let step_path = format!("{example_name}.step");
    let text_path = format!("{example_name}_text.brep");
    let brep_path = format!("{example_name}.brep");

    // 0. Original: read colored_box.step
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let original = cadrum::read_step(
        &mut std::fs::File::open(format!("{manifest_dir}/steps/colored_box.step")).expect("open file"),
    )?;

    // 1. STEP round-trip: rotate 30° → write → read
    let a_written = original.clone().rotate_x(FRAC_PI_8);
    cadrum::write_step(&a_written, &mut std::fs::File::create(&step_path).expect("create file"))?;
    let a = cadrum::read_step(&mut std::fs::File::open(&step_path).expect("open file"))?;

    // 2. BRep text round-trip: rotate another 30° → write → read
    let b_written = a.clone().rotate_x(FRAC_PI_8);
    cadrum::write_brep_text(&b_written, &mut std::fs::File::create(&text_path).expect("create file"))?;
    let b = cadrum::read_brep_text(&mut std::fs::File::open(&text_path).expect("open file"))?;

    // 3. BRep binary round-trip: rotate another 30° → write → read
    let c_written = b.clone().rotate_x(FRAC_PI_8);
    cadrum::write_brep_binary(&c_written, &mut std::fs::File::create(&brep_path).expect("create file"))?;
    let c = cadrum::read_brep_binary(&mut std::fs::File::open(&brep_path).expect("open file"))?;

    // 4. Arrange side by side and export SVG + STL
    let [min, max] = original[0].bounding_box();
    let spacing = (max - min).length() * 1.5;
    let all: Vec<Solid> = [original, a, b, c].into_iter()
        .enumerate()
        .flat_map(|(i, solids)| solids.translate(DVec3::X * spacing * i as f64))
        .collect();

    let mut svg = std::fs::File::create(format!("{example_name}.svg")).expect("create file");
    cadrum::mesh(&all, 0.5).and_then(|m| m.write_svg(DVec3::new(1.0, 1.0, 2.0), true, false, &mut svg))?;

    let mut stl = std::fs::File::create(format!("{example_name}.stl")).expect("create file");
    cadrum::mesh(&all, 0.1).and_then(|m| m.write_stl(&mut stl))?;

    // 5. Print summary
    let stl_path = format!("{example_name}.stl");
    for (label, path) in [("STEP", &step_path), ("BRep text", &text_path), ("BRep binary", &brep_path), ("STL", &stl_path)] {
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        println!("{label:12} {path:30} {size:>8} bytes");
    }

    Ok(())
}

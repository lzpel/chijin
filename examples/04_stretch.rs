//! Stretch example: create a cylinder and stretch it along XYZ from its center.
//!
//! ```
//! cargo run --example 02_stretch
//! ```
//!
//! Output: stretched.brep (BRep text format)

use cadrum::Solid;
use cadrum::utils::stretch_vector;
use glam::DVec3;

/// Cut at (cx,cy,cz) and stretch each axis by (dx,dy,dz). Axes with delta <= 0 are skipped.
fn stretch(shape: Vec<Solid>, cx: f64, cy: f64, cz: f64, dx: f64, dy: f64, dz: f64) -> Result<Vec<Solid>, cadrum::Error> {
    let eps = 1e-10;
    let shape = if dx > eps { stretch_vector(&shape, DVec3::new(cx, 0.0, 0.0), DVec3::new(dx, 0.0, 0.0))? } else { shape };
    let shape = if dy > eps { stretch_vector(&shape, DVec3::new(0.0, cy, 0.0), DVec3::new(0.0, dy, 0.0))? } else { shape };
    let shape = if dz > eps { stretch_vector(&shape, DVec3::new(0.0, 0.0, cz), DVec3::new(0.0, 0.0, dz))? } else { shape };
    shape.iter().map(|s| s.clean()).collect()
}

fn main() {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();
    let radius = 20.0_f64;
    let height = 80.0_f64;
    let cylinder: Vec<Solid> = vec![Solid::cylinder(radius, DVec3::Z, height)];
    let center = DVec3::new(0.0, 0.0, height / 2.0);
    let (dx, dy, dz) = (30.0, 20.0, 40.0);

    println!("cylinder: radius={radius}mm, height={height}mm");
    println!("cut at: {center:?} / stretch: X={dx}mm Y={dy}mm Z={dz}mm");

    let result = stretch(cylinder, center.x, center.y, center.z, dx, dy, dz)
        .expect("stretch failed");

    let out_path = format!("{example_name}.brep");
    let mut buf = Vec::new();
    cadrum::io::write_brep_text(&result, &mut buf).expect("failed to write BRep");
    std::fs::write(out_path, &buf).expect("failed to write file");

    let mesh = cadrum::io::mesh(&result, 0.5).expect("mesh failed");
    println!(
        "done: ({} bytes) — vertices: {}, triangles: {}",
        buf.len(),
        mesh.vertices.len(),
        mesh.indices.len() / 3,
    );
}

use cadrum::{Color, Shape, Solid};
use glam::DVec3;

fn main() {
    let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

    // Primitives placed along X so they do not overlap
    let box_ = Solid::box_from_corners(DVec3::ZERO, DVec3::new(10.0, 20.0, 30.0))
        .color_paint(Some(Color::from_hex("#4a90d9").unwrap()));

    let cylinder = Solid::cylinder(DVec3::new(30.0, 0.0, 0.0), 8.0, DVec3::Z, 30.0)
        .color_paint(Some(Color::from_hex("#e67e22").unwrap()));

    let sphere = Solid::sphere(DVec3::new(60.0, 0.0, 15.0), 8.0)
        .color_paint(Some(Color::from_hex("#2ecc71").unwrap()));

    // Cone with pointed tip (r2=0)
    let cone = Solid::cone(DVec3::new(90.0, 0.0, 0.0), DVec3::Z, 8.0, 0.0, 30.0)
        .color_paint(Some(Color::from_hex("#e74c3c").unwrap()));

    // Torus centered at mid-height so it aligns visually with the others
    let torus = Solid::torus(DVec3::new(130.0, 0.0, 15.0), DVec3::Z, 12.0, 4.0)
        .color_paint(Some(Color::from_hex("#9b59b6").unwrap()));

    let shapes = vec![box_, cylinder, sphere, cone, torus];

    let mut f = std::fs::File::create(format!("{example_name}.step")).expect("failed to create file");
    cadrum::write_step_with_colors(&shapes, &mut f).expect("failed to write STEP");

    let svg = shapes.to_svg(DVec3::new(1.0, 1.0, 1.0), 0.5).expect("failed to export SVG");
    std::fs::write(format!("{example_name}.svg"), svg.as_bytes()).expect("failed to write SVG");
}

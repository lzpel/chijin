use cadrum::{Boolean, Color, Solid};
use glam::DVec3;

fn main() {
    let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

    // Base shapes: a box and a cylinder, overlapping at origin
    let make_box = || {
        Solid::cube(20.0, 20.0, 20.0)
            .color_paint(Some(Color::from_str("#4a90d9").unwrap()))
    };
    let make_cyl = || {
        Solid::cylinder(DVec3::new(10.0, 10.0, -5.0), 8.0, DVec3::Z, 30.0)
            .color_paint(Some(Color::from_str("#e67e22").unwrap()))
    };

    // union: merge both shapes into one — offset X=0
    let union: Vec<Solid> = Boolean::union(&[make_box()], &[make_cyl()])
        .expect("union failed")
        .into_solids()
        .into_iter()
        .map(|s| s.translate(DVec3::new(0.0, 0.0, 0.0)))
        .collect();

    // subtract: box minus cylinder — offset X=40
    let subtract: Vec<Solid> = Boolean::subtract(&[make_box()], &[make_cyl()])
        .expect("subtract failed")
        .into_solids()
        .into_iter()
        .map(|s| s.translate(DVec3::new(40.0, 0.0, 0.0)))
        .collect();

    // intersect: only the overlapping volume — offset X=80
    let intersect: Vec<Solid> = Boolean::intersect(&[make_box()], &[make_cyl()])
        .expect("intersect failed")
        .into_solids()
        .into_iter()
        .map(|s| s.translate(DVec3::new(80.0, 0.0, 0.0)))
        .collect();

    let shapes: Vec<Solid> = [union, subtract, intersect].concat();

    let mut f = std::fs::File::create(format!("{example_name}.step")).expect("failed to create file");
    cadrum::io::write_step(&shapes, &mut f).expect("failed to write STEP");

    let mut svg = std::fs::File::create(format!("{example_name}.svg")).expect("failed to create SVG file");
    cadrum::io::write_svg(&shapes, DVec3::new(1.0, 1.0, 2.0), 0.5, &mut svg).expect("failed to write SVG");
}

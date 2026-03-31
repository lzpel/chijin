use cadrum::{Boolean, Color, Shape, Solid};
use glam::DVec3;

fn main() {
    let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

    // Base shapes: a box and a cylinder, overlapping at origin
    let make_box = || {
        Solid::box_from_corners(DVec3::ZERO, DVec3::new(20.0, 20.0, 20.0))
            .color_paint(Some(Color::from_hex("#4a90d9").unwrap()))
    };
    let make_cyl = || {
        Solid::cylinder(DVec3::new(10.0, 10.0, -5.0), 8.0, DVec3::Z, 30.0)
            .color_paint(Some(Color::from_hex("#e67e22").unwrap()))
    };

    // union: merge both shapes into one — offset X=0
    let union = Boolean::union(&[make_box()], &[make_cyl()])
        .expect("union failed")
        .solids
        .translate(DVec3::new(0.0, 0.0, 0.0));

    // subtract: box minus cylinder — offset X=40
    let subtract = Boolean::subtract(&[make_box()], &[make_cyl()])
        .expect("subtract failed")
        .solids
        .translate(DVec3::new(40.0, 0.0, 0.0));

    // intersect: only the overlapping volume — offset X=80
    let intersect = Boolean::intersect(&[make_box()], &[make_cyl()])
        .expect("intersect failed")
        .solids
        .translate(DVec3::new(80.0, 0.0, 0.0));

    let shapes: Vec<Solid> = [union, subtract, intersect].concat();

    let mut f = std::fs::File::create(format!("{example_name}.step")).expect("failed to create file");
    cadrum::write_step_with_colors(&shapes, &mut f).expect("failed to write STEP");

    let svg = shapes.to_svg(DVec3::new(1.0, 1.0, 2.0), 0.5).expect("failed to export SVG");
    std::fs::write(format!("{example_name}.svg"), svg.as_bytes()).expect("failed to write SVG");
}

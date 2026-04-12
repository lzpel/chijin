use cadrum::{BSplineEnd, Edge, EdgeExt, Error, ProfileOrient, Solid, Transform};
use glam::{DQuat, DVec3};

const I_MAX: usize = 10;
const J_MAX: usize = 10;
fn s(i: usize, j: usize) -> DVec3 {
    let theta = (i as f64) / (I_MAX as f64) * 2.0 * std::f64::consts::PI;
    let phi = (j as f64) / (J_MAX as f64) * 2.0 * std::f64::consts::PI;
    let p=DVec3::new(1.0, 0.0, 0.0);
    let p_with_theta=DQuat::from_axis_angle(DVec3::Z, theta) * p;
    let p_with_phi=DQuat::from_axis_angle(DVec3::Y, phi) * (p_with_theta + DVec3::X*3.0);
    p_with_phi
}
fn profile(j: usize) -> [Edge;1]{
    let points: [DVec3; I_MAX] = std::array::from_fn(|i| s(i, j));
    [Edge::bspline(points, BSplineEnd::Periodic).unwrap()]
}
fn guides(i: usize, closed: bool) -> [Edge;1]{
    let points: [DVec3; J_MAX] = std::array::from_fn(|j| s(i, j));
    [Edge::bspline(points, if closed {BSplineEnd::Periodic} else {BSplineEnd::NotAKnot}).unwrap()]
}
fn pipe(w: &[Edge; 1]) -> Solid {
    let circle: Edge = Edge::circle(0.1, DVec3::Z).unwrap().align_z(w.start_tangent(), DVec3::X).translate(w.start_point());
    Solid::sweep([&circle], w, ProfileOrient::Torsion).unwrap()
}
fn gordon_surface(closed: bool) -> Result<Solid, Error> {
    let edges_profile: [[Edge; 1]; I_MAX] = std::array::from_fn(|i| profile(i));
    let edges_guide: [[Edge; 1]; J_MAX] = std::array::from_fn(|j| guides(j, closed));
    Solid::gordon(&edges_profile, &edges_guide)
}
fn main() {
    let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();
    let mut objects: Vec<Solid> = Vec::new();
    for (closed, offset) in [(false, 0.0), (true, 10.0)] {
        match gordon_surface(closed) {
            Ok(g) => {
                let volume = g.volume();
                eprintln!("closed={}: volume = {}", closed, volume);
                if 30.0 <= volume && volume <= 90.0 {
                    eprintln!("  -> in range. great");
                } else {
                    eprintln!("  -> out of range");
                }
                objects.push(g.translate(DVec3::Y * offset));
            }
            Err(e) => eprintln!("closed={}: error: {}", closed, e),
        }
    }
    let mut f = std::fs::File::create(format!("{example_name}.step")).unwrap();
    cadrum::io::write_step(&objects, &mut f).unwrap();
    let mut f_svg = std::fs::File::create(format!("{example_name}.svg")).unwrap();
    cadrum::io::write_svg(&objects, DVec3::new(1.0, 1.0, 1.0), 0.5, false, &mut f_svg).unwrap();
    eprintln!("wrote {0}.step / {0}.svg ({1} solids)", example_name, objects.len());
}
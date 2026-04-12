use cadrum::{BSplineEnd, Edge, EdgeExt, ProfileOrient, Solid, Transform};
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
fn guides(i: usize) -> [Edge;1]{
    let points: [DVec3; J_MAX] = std::array::from_fn(|j| s(i, j));
    [Edge::bspline(points, BSplineEnd::Periodic).unwrap()]
}
fn pipe(w: &[Edge; 1]) -> Solid {
    let circle: Edge = Edge::circle(0.1, DVec3::Z).unwrap().align_z(w.start_tangent(), DVec3::X).translate(w.start_point());
    Solid::sweep([&circle], w, ProfileOrient::Torsion).unwrap()
}
fn main() {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();
    let points: [Solid; I_MAX*J_MAX] = std::array::from_fn(|i| Solid::sphere(0.1).translate(s(i/J_MAX, i%J_MAX)));
    let edges_profile: [[Edge; 1]; I_MAX] = std::array::from_fn(|i| profile(i));
    let edges_guide: [[Edge; 1]; J_MAX] = std::array::from_fn(|j| guides(j));
    let profiles: [Solid; J_MAX+I_MAX] = std::array::from_fn(|j| if j<J_MAX {pipe(&profile(j))} else {pipe(&guides(j-J_MAX))});
    // gordon surface
    let mut objects: Vec<Solid> = points.into_iter().chain(profiles.translate(DVec3::Y*6.0)).collect();
    if let Ok(gordon_surface) = Solid::gordon(&edges_profile, &edges_guide) {
        objects.push(gordon_surface.translate(DVec3::Y*12.0));
    }
    let mut f = std::fs::File::create(format!("{example_name}.step")).unwrap();
    cadrum::io::write_step(&objects, &mut f).unwrap();
    let mut f = std::fs::File::create(format!("{example_name}.stl")).unwrap();
    cadrum::io::write_stl(&objects, 0.1, &mut f).unwrap();
    let mut f_svg = std::fs::File::create(format!("{example_name}.svg")).unwrap();
    cadrum::io::write_svg(&objects, DVec3::new(1.0, 1.0, 1.0), 0.5, false, &mut f_svg).unwrap();
}
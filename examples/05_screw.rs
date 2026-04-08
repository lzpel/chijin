use cadrum::{Edge, Solid};
use glam::DVec3;


fn build_m2_screw()->Solid{
    // iso m2 screw
    let r=1.0;
    let h_pitch = 0.4;
    let h_thread = 6.0;
    let r_head = 1.75;
    let h_head = 1.3;
    // iso m screw mountain height
    let r_delta=3f64.sqrt()/2.0 * h_pitch;//60度なので正三角形の高さから√3/2
    let r_root = r - (5.0/8.0)*r_delta;
    let helix=Edge::helix(r, h_pitch, h_thread, DVec3::Z);
    let profile=Edge::polygon(
        [
            DVec3::new(r_root,-h_pitch/2.0,0.0),
            DVec3::new(r,0.0,0.0),
            DVec3::new(r_root,h_pitch/2.0,0.0)
        ]
    );
    let profile=profile.align_z_with_x_hint(helix.start_tangent(), helix.start_point()).translate(helix.start_point());
    let thread=profile.sweep(helix);
    let thread_shaft=thread.union(Solid::cylinder(r_root, DVec3::Z, h_thread)).subtract(Solid::cylinder(r_root*2.0, DVec3::Z, h_thread));
    let head=Solid::cylinder(r_head, DVec3::Z, h_head).translate(DVec3::Z*h_thread);
    return thread_shaft.union(head);
}
fn main() {
    let screw=build_m2_screw();
    let mut f = std::fs::File::create("screw.step").expect("failed to create STEP file");
    cadrum::io::write_step([&screw], &mut f).expect("failed to write STEP");
    let mut f_svg=std::fs::File::create("screw.svg").expect("failed to create SVG file");
    cadrum::io::write_svg([&screw], DVec3::new(1.0, 1.0, 1.0), 0.5, &mut f_svg).expect("failed to write SVG");
    println!("wrote screw.step");
}
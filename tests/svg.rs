use cadrum::Solid;
#[cfg(feature = "color")]
use cadrum::Color;
use glam::DVec3;

fn dvec3(x: f64, y: f64, z: f64) -> DVec3 {
	DVec3::new(x, y, z)
}

#[test]
fn test_svg_box_isometric() {
	let shape: Vec<Solid> = vec![Solid::box_from_corners(DVec3::ZERO, dvec3(10.0, 10.0, 10.0))];
	let svg = cadrum::to_svg(&shape, dvec3(1.0, 1.0, 1.0).normalize(), 0.1)
		.unwrap();

	assert!(svg.starts_with("<svg"), "should start with <svg tag");
	assert!(svg.contains("</svg>"), "should end with </svg>");
	assert!(svg.contains("<polyline"), "should contain polyline elements");
	assert!(svg.contains("viewBox"), "should contain viewBox");
	// Should not have a top-level width/height attribute (responsive via viewBox only).
	// stroke-width is fine; we check that <svg ...> does not contain ' width="'.
	let svg_tag = &svg[..svg.find('>').unwrap()];
	assert!(!svg_tag.contains(" width="), "should not contain fixed width (responsive)");

	// Write to file for visual inspection
	std::fs::create_dir_all("out").unwrap();
	std::fs::write("out/box_isometric.svg", &svg).unwrap();
	println!("SVG length: {} bytes", svg.len());
}

#[test]
fn test_svg_box_top_down() {
	let shape: Vec<Solid> = vec![Solid::box_from_corners(DVec3::ZERO, dvec3(10.0, 10.0, 10.0))];
	let svg = cadrum::to_svg(&shape, DVec3::Z, 0.1).unwrap();

	assert!(svg.starts_with("<svg"));
	assert!(svg.contains("<polyline"));

	std::fs::create_dir_all("out").unwrap();
	std::fs::write("out/box_top.svg", &svg).unwrap();
}

#[test]
fn test_svg_cylinder() {
	let shape: Vec<Solid> = vec![Solid::cylinder(DVec3::ZERO, 5.0, DVec3::Z, 10.0)];
	let svg = cadrum::to_svg(&shape, dvec3(1.0, 0.5, 0.3).normalize(), 0.1)
		.unwrap();

	assert!(svg.contains("<polyline"));

	std::fs::create_dir_all("out").unwrap();
	std::fs::write("out/cylinder.svg", &svg).unwrap();
}

#[test]
fn test_svg_has_hidden_lines() {
	// Two boxes: the back one should have hidden edges
	let a: Vec<Solid> = vec![Solid::box_from_corners(DVec3::ZERO, dvec3(10.0, 10.0, 10.0))];
	let b: Vec<Solid> = vec![Solid::box_from_corners(dvec3(5.0, 5.0, 0.0), dvec3(15.0, 15.0, 10.0))];
	let shape: Vec<Solid> = cadrum::Boolean::union(&a, &b).unwrap().into();
	let svg = cadrum::to_svg(&shape, dvec3(1.0, 1.0, 1.0).normalize(), 0.1)
		.unwrap();

	assert!(svg.contains("#999"), "should contain hidden line color");

	std::fs::create_dir_all("out").unwrap();
	std::fs::write("out/two_boxes.svg", &svg).unwrap();
}

#[test]
#[cfg(feature = "color")]
fn test_svg_colored_box() {
	let mut shape: Vec<Solid> = vec![Solid::box_from_corners(DVec3::ZERO, dvec3(10.0, 10.0, 10.0))];

	// Assign a distinct color to each face based on its normal
	let palette: &[(DVec3, Color)] = &[
		(DVec3::Z,     Color { r: 1.0, g: 0.0, b: 0.0 }), // top:    red
		(DVec3::NEG_Z, Color { r: 0.0, g: 0.0, b: 1.0 }), // bottom: blue
		(DVec3::Y,     Color { r: 0.0, g: 1.0, b: 0.0 }), // back:   green
		(DVec3::NEG_Y, Color { r: 1.0, g: 1.0, b: 0.0 }), // front:  yellow
		(DVec3::X,     Color { r: 0.0, g: 1.0, b: 1.0 }), // right:  cyan
		(DVec3::NEG_X, Color { r: 1.0, g: 0.0, b: 1.0 }), // left:   magenta
	];
	let id_normal: Vec<(u64, DVec3)> = shape
		.iter().flat_map(|s| s.face_iter())
		.map(|f| (f.tshape_id(), f.normal_at_center()))
		.collect();
	for (id, normal) in &id_normal {
		for (dir, color) in palette {
			if normal.dot(*dir) > 0.9 {
				shape[0].colormap_mut().insert(*id, *color);
				break;
			}
		}
	}

	let svg = cadrum::to_svg(&shape, dvec3(1.0, 1.0, 1.0).normalize(), 0.1)
		.unwrap();

	// Should contain rgb colors from the colormap
	assert!(svg.contains("rgb("), "should contain rgb fill colors");
	assert!(!svg.contains("#ddd"), "should not contain default gray fill");

	std::fs::create_dir_all("out").unwrap();
	std::fs::write("out/colored_box.svg", &svg).unwrap();
}

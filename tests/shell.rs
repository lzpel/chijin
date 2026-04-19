use cadrum::Solid;

#[test]
fn test_shell_cube_reduces_volume() {
	let cube = Solid::cube(10.0, 10.0, 10.0);
	let original_volume = cube.volume();

	let open = cube.iter_face().next().unwrap();
	let shelled = cube.shell(-0.5, [open]).expect("shell should succeed");

	assert!(shelled.volume() > 0.0, "shelled solid must have positive volume");
	assert!(shelled.volume() < original_volume, "shelling inward must reduce volume");
}

#[test]
fn test_shell_cube_preserves_solid_structure() {
	let cube = Solid::cube(10.0, 10.0, 10.0);
	let open = cube.iter_face().next().unwrap();
	let shelled = cube.shell(-0.5, [open]).expect("shell should succeed");
	assert_eq!(shelled.shell_count(), 1, "shelled cube should remain a single shell");
}

#[test]
fn test_shell_outward_produces_wall() {
	let cube = Solid::cube(10.0, 10.0, 10.0);
	let open = cube.iter_face().next().unwrap();
	// Positive thickness: wall grows outward. The original solid becomes the
	// inner cavity of a 0.5-thick shell.
	let shell = cube.shell(0.5, [open]).expect("shell outward should succeed");
	assert!(shell.volume() > 0.0 && shell.volume() < 1000.0, "outer shell is wall material only, not the original cube");
	assert_eq!(shell.shell_count(), 1);
}


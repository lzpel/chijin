use std::fs::File;

fn main() -> std::io::Result<()> {
    let c = dummy_cadrum::cube();
    let mut f = File::create("cube.step")?;
    dummy_cadrum::write_step(&c, &mut f)?;
    println!("wrote cube.step");
    Ok(())
}

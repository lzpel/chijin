fn main() {
    cc::Build::new().file("csrc/dummy.c").compile("dummy");
    println!("cargo:rerun-if-changed=csrc/dummy.c");
}

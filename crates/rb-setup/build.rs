//! Hetzelfde pictogram als MathPrime zelf (zie `crates/rb-gui/assets/README.md`).

fn main() {
    println!("cargo:rerun-if-changed=../rb-gui/assets/mathprime.res");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if windows && msvc {
        let dir = std::env::var("CARGO_MANIFEST_DIR").expect("manifestmap");
        println!("cargo:rustc-link-arg-bins={dir}/../rb-gui/assets/mathprime.res");
    }
}

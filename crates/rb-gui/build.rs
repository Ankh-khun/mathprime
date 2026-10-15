//! Het pictogram in het exe-bestand (Windows): `assets/mathprime.res` is gemaakt met
//! `rc.exe /fo mathprime.res mathprime.rc` uit `mathprime.ico`; zie `assets/README.md`.

fn main() {
    println!("cargo:rerun-if-changed=assets/mathprime.res");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if windows && msvc {
        let dir = std::env::var("CARGO_MANIFEST_DIR").expect("manifestmap");
        println!("cargo:rustc-link-arg-bins={dir}/assets/mathprime.res");
    }
}

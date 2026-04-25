fn main() {
    // Calling `build_info_build::build_script` collects all data and makes it available to `build_info::build_info!`
    // and `build_info::format!` in the main program.
    _ = build_info_build::build_script();

    #[cfg(debug_assertions)]
    {
        println!("cargo:rerun-if-changed=build.rs");
    }
}

//! Compile and bake `app.contract` with the Rust data source into `OUT_DIR/app.plan`.

use contract::DataSource;

fn main() {
    contract::rerun_if_changed(std::path::Path::new("../app.contract"));
    println!("cargo:rerun-if-changed=../app.json");
    println!("cargo:rerun-if-changed=../data");
    println!("cargo:rerun-if-changed=build.rs");
    let plan = match contract::compile_path(std::path::Path::new("../app.contract")) {
        Ok(p) => p,
        Err(e) => panic!("app.contract:{e}"),
    };
    let baked = contract::bake(plan, exact_rust_app_data::Toggle)
        .unwrap_or_else(|e| panic!("bake: {e:?}"));
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("app.plan"), baked.encode()).unwrap();
    let app_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let target = std::env::var("TARGET").unwrap_or_default();
    let manifest = contract::Manifest::read(&app_dir).unwrap_or_else(|e| panic!("app.json: {e}"));
    let grants = exact_rust_app_data::Toggle.grants();
    let compat = exact_bake::compatibility_id(&app_dir, "web", &target, &manifest, Some(grants))
        .unwrap_or_else(|e| panic!("compatibility id: {e}"));
    std::fs::write(out_dir.join("compat.json"), compat.to_json()).unwrap();
    let entry = contract::rust_entry(
        "exact_rust_app_data::Toggle",
        "exact_rust_app_data::Toggle",
        contract::web_rust_mode(&compat.inputs),
    )
    .unwrap();
    std::fs::write(
        out_dir.join("entry.rs"),
        format!(
            "{entry}\n{}exact_web::host!(AppData, PLAN, COMPAT, app_data);\n",
            contract::web_linked(&baked, &compat.inputs)
        ),
    )
    .unwrap();
}

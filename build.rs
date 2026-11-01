fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/nexora.ico");
    println!("cargo:rerun-if-changed=assets/nexora.manifest");

    // Иконка, манифест и сведения о версии встраиваются в .exe.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/nexora.ico");
        res.set_manifest_file("assets/nexora.manifest");
        res.set("ProductName", "Nexora Unlocker");
        res.set("FileDescription", "Nexora Unlocker");
        res.set("CompanyName", "Nexora");
        res.compile()
            .expect("не удалось встроить ресурсы Windows (нужен Windows SDK из Visual Studio Build Tools)");
    }
}

//! Pe Windows, iconița aplicației se încorporează în executabil.
fn main() {
    println!("cargo:rerun-if-changed=../../assets/icons/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icons/icon.ico");
        res.set("ProductName", "HidroCalc");
        res.set("FileDescription", "HidroCalc - dimensionare retele de alimentare cu apa");
        if let Err(e) = res.compile() {
            println!("cargo:warning=iconița Windows nu a putut fi încorporată: {e}");
        }
    }
}

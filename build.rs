//! Puts the logo into the Windows executables (Explorer, the taskbar and Alt+Tab show it) and
//! fills in the file properties. Does nothing on other systems.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    embed_icon();
}

// `winresource` is a build-dependency on Windows hosts only, so the code must not exist elsewhere.
#[cfg(windows)]
fn embed_icon() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "Tronic Printer");
        res.set("FileDescription", "Tronic Printer: print from Windows on the Tronic Mini Pocket Printer");
        res.set("LegalCopyright", "MIT");
        if let Err(e) = res.compile() {
            // A missing resource compiler must not stop the build: the app just gets no icon.
            println!("cargo:warning=could not embed the icon: {e}");
        }
    }
}

fn main() {
    // evil-janet handles Janet library discovery and linking.
    // With the "link-system" feature it links to the system-installed libjanet.

    // macOS runs an unbundled executable in low-resolution mode
    // (`backingScaleFactor` 1) unless the binary carries an Info.plist with
    // NSHighResolutionCapable.  Embedding one in the Mach-O __info_plist
    // section makes `cargo run` render at the display's real scale instead of
    // compositing a 1× frame up to 2×.  It lives here, not in the `ui` crate:
    // a library's link args never reach the final executable.
    #[cfg(target_os = "macos")]
    {
        let plist =
            std::path::Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("Info.plist");
        std::fs::write(
            &plist,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>ai.magma.slint</string>
    <key>CFBundleName</key>
    <string>Magma</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
"#,
        )
        .expect("write Info.plist");
        println!(
            "cargo:rustc-link-arg-bins=-Wl,-sectcreate,__TEXT,__info_plist,{}",
            plist.display()
        );
    }
}

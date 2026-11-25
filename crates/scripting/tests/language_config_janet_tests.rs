#[macro_use]
mod helpers;
use scripting;

// ── language/define evaluates :config ───────────────────────────────────
//
// A language's config.janet carries the option defaults its mode setup
// reads (<id>-indent-width and friends).  Registration is the point where
// those defaults must land: a manifest always precedes mode definition
// and activation, so a setup can never read a missing option — whatever
// order the plugin enable flow runs in.

fn tmp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("magma_lang_cfg_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &std::path::Path, content: &str) -> String {
    std::fs::write(path, content).unwrap();
    path.to_str().unwrap().to_string()
}

#[test]
fn language_define_evaluates_config() {
    janet_test!(ed, {
        let cfg = write(
            &tmp_dir("define").join("config.janet"),
            r#"(option/set "zzcfg-indent-width" "2")"#,
        );
        let result = scripting::eval_result(&format!(
            r#"(string (language/define {{:id "zzcfg" :config "{}"}}))"#,
            cfg
        ));
        assert!(
            result.is_ok(),
            "define itself must succeed: {:?}",
            result
        );
        let value = scripting::eval_result(r#"(option/get "zzcfg-indent-width")"#).unwrap();
        assert_eq!(value, r#""2""#, "config option must be set right after define");
        let _ = ed;
    });
}

#[test]
fn language_define_tolerates_broken_config() {
    janet_test!(ed, {
        let cfg = write(
            &tmp_dir("broken").join("config.janet"),
            r#"(error "boom")"#,
        );
        let result = scripting::eval_result(&format!(
            r#"(string (language/define {{:id "zzbrok" :config "{}"}}))"#,
            cfg
        ));
        assert!(
            result.is_ok(),
            "a failing config must not fail the registration: {:?}",
            result
        );
        let _ = ed;
    });
}

#[test]
fn discovery_applies_config_without_enabling() {
    janet_test!(ed, {
        // The regression this guards: bootstrap only discovers plugins,
        // yet a buffer of that language can activate later — so the
        // config's options must exist at discovery time, before the
        // plugin entry ever runs.
        let root = tmp_dir("discover").join("plugins");
        let plugin = root.join("zzdisc");
        std::fs::create_dir_all(&plugin).unwrap();
        write(
            &plugin.join("manifest.janet"),
            r#"(plugin/define {:name "zzdisc" :entry "main.janet" :languages ["zzdisc"]
                 :extensions {"zzdisc" ["zzd"]}})"#,
        );
        write(
            &plugin.join("config.janet"),
            r#"(option/set "zzdisc-indent-width" "3")"#,
        );
        // The entry defines a defensive setup — the pattern every plugin
        // mode setup follows.
        write(
            &plugin.join("main.janet"),
            r#"(major-mode/define "zzdisc-mode"
  {:parent "text"
   :layers [:zzdisc-mode]
   :setup (fn []
            (def width (or (option/get "zzdisc-indent-width") "4"))
            (option/set-local "tab-width" width))})"#,
        );

        let discovered = scripting::eval_result(&format!(
            r#"(plugin/discover "{}")"#,
            root.to_str().unwrap()
        ));
        assert_eq!(discovered.unwrap(), "1", "the fixture plugin must be found");

        let loaded =
            scripting::eval_result(r#"(string (get (plugin/status "zzdisc") :loaded))"#)
                .unwrap_or_default();
        assert_eq!(loaded, r#""false""#, "discovery must not enable the plugin");

        let value = scripting::eval_result(r#"(option/get "zzdisc-indent-width")"#).unwrap();
        assert_eq!(
            value, r#""3""#,
            "config options must exist after discovery, before enable"
        );

        // End to end: enabling and activating the mode reads the option
        // that discovery already put in place.
        assert_eq!(scripting::eval(r#"(plugin/enable "zzdisc")"#), "ok");
        assert_eq!(
            scripting::eval(r#"(editor/run-command "zzdisc-mode")"#),
            "ok"
        );
        let local = scripting::eval_result(r#"(option/get-local "tab-width")"#).unwrap();
        assert_eq!(local, r#""3""#, "the setup must see the discovered option");
        std::fs::remove_dir_all(&tmp_dir("discover")).ok();
    });
}

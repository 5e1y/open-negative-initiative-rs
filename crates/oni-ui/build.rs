fn main() {
    // Dev and test builds read the .slint files at run time (Slint's live preview), a thin layer that
    // compiles in seconds where the whole interface takes minutes; release and the app profile compile it.
    if std::env::var("PROFILE").as_deref() == Ok("debug") {
        // SAFETY: a build script runs on one thread, before the compiler reads its environment.
        unsafe { std::env::set_var("SLINT_LIVE_PREVIEW", "1") };
    }
    println!("cargo:rerun-if-env-changed=PROFILE");
    // Every `@tr` reads its text alone, no component context, so one catalogue entry serves each text
    // wherever it shows, and the Rust side reads the same entries.
    let config = slint_build::CompilerConfiguration::new()
        .with_bundled_translations("../oni-i18n/lang")
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/app.slint", config).expect("ui/app.slint compiles");
}

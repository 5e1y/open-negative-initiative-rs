fn main() {
    // Every `@tr` reads its text alone, no component context, so one catalogue entry serves each text
    // wherever it shows, and the Rust side reads the same entries.
    let config = slint_build::CompilerConfiguration::new()
        .with_bundled_translations("../oni-i18n/lang")
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/app.slint", config).expect("ui/app.slint compiles");
}

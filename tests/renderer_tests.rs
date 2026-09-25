use celerterm::renderer::TextRenderer;

#[test]
fn test_font_shaping_and_ligature_processing() {
    let mut renderer = TextRenderer::new("JetBrainsMono Nerd Font", 14.0, 1.2);
    
    // Normal ASCII text
    let glyphs_ascii = renderer.shape_line("hello world");
    assert!(glyphs_ascii > 0);

    // Ligature sequences: "->", "=>", "!=", "==="
    let glyphs_ligature = renderer.shape_line("fn test() -> bool { a != b && x === y }");
    assert!(glyphs_ligature > 0);
}

#[test]
fn test_nerd_font_symbol_shaping() {
    let mut renderer = TextRenderer::new("JetBrainsMono Nerd Font", 14.0, 1.2);

    // Nerd Font icons: Folder (\u{f07b}), Git branch (\u{e725}), Rust gear (\u{e7a8})
    let nerd_text = " \u{f07b} project  \u{e725} main  \u{e7a8} cargo";
    let glyphs = renderer.shape_line(nerd_text);
    assert!(glyphs > 0);
}

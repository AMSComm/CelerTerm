use celerterm::renderer::TextRenderer;

#[test]
fn test_font_shaping_and_ligature_processing() {
    let mut renderer = TextRenderer::new("Firple", 13.0, 1.2);
    
    // Normal ASCII text
    let glyphs_ascii = renderer.shape_line("hello world");
    assert!(glyphs_ascii > 0);

    // Ligature sequences: "->", "=>", "!=", "==="
    let glyphs_ligature = renderer.shape_line("fn test() -> bool { a != b && x === y }");
    assert!(glyphs_ligature > 0);
}

#[test]
fn test_nerd_font_symbol_shaping() {
    let mut renderer = TextRenderer::new("Firple", 13.0, 1.2);

    // Nerd Font icons: Folder (\u{f07b}), Git branch (\u{e725}), Rust gear (\u{e7a8})
    let nerd_text = " \u{f07b} project  \u{e725} main  \u{e7a8} cargo";
    let glyphs = renderer.shape_line(nerd_text);
    assert!(glyphs > 0);
}

#[test]
fn test_box_and_block_char_detection() {
    // Unicode box-drawing characters
    assert!(TextRenderer::is_box_or_block('│'));
    assert!(TextRenderer::is_box_or_block('─'));
    assert!(TextRenderer::is_box_or_block('┌'));
    assert!(TextRenderer::is_box_or_block('┐'));
    assert!(TextRenderer::is_box_or_block('└'));
    assert!(TextRenderer::is_box_or_block('┘'));
    assert!(TextRenderer::is_box_or_block('╭'));
    assert!(TextRenderer::is_box_or_block('╯'));
    assert!(TextRenderer::is_box_or_block('┼'));

    // Block elements
    assert!(TextRenderer::is_box_or_block('█'));
    assert!(TextRenderer::is_box_or_block('▀'));
    assert!(TextRenderer::is_box_or_block('▄'));
    assert!(TextRenderer::is_box_or_block('▌'));
    assert!(TextRenderer::is_box_or_block('▐'));

    // Standard characters should not be detected as box chars
    assert!(!TextRenderer::is_box_or_block('A'));
    assert!(!TextRenderer::is_box_or_block('0'));
    assert!(!TextRenderer::is_box_or_block(' '));
    assert!(!TextRenderer::is_box_or_block('='));
}

#[test]
fn test_nerd_font_pua_detection() {
    // BMP Private Use Area (Nerd Font icons)
    assert!(TextRenderer::is_nerd_font_or_pua('\u{f07b}')); // folder
    assert!(TextRenderer::is_nerd_font_or_pua('\u{e725}')); // git branch
    assert!(TextRenderer::is_nerd_font_or_pua('\u{e7a8}')); // rust

    // Normal characters
    assert!(!TextRenderer::is_nerd_font_or_pua('x'));
    assert!(!TextRenderer::is_nerd_font_or_pua('│'));
}

#[test]
fn test_draw_box_and_block_char() {
    let renderer = TextRenderer::new("Firple", 13.0, 1.2);
    let mut buffer = vec![0u32; 100 * 100];

    // Draw box drawing vertical line
    renderer.draw_box_or_block_char(&mut buffer, 100, 100, 10.0, 10.0, '│', 0x00FF0000);
    // Verify some pixels were written
    assert!(buffer.contains(&0x00FF0000));

    // Draw full block
    renderer.draw_box_or_block_char(&mut buffer, 100, 100, 30.0, 30.0, '█', 0x0000FF00);
    assert!(buffer.contains(&0x0000FF00));
}

#[test]
fn test_with_fallbacks_font_loading() {
    let fallbacks = vec![
        "CaskaydiaCove Nerd Font Mono".to_string(),
        "JetBrainsMono NF".to_string(),
        "Menlo".to_string(),
    ];
    let renderer = TextRenderer::with_fallbacks("NonExistentFontXYZ", &fallbacks, 13.0, 1.2);
    assert!(renderer.cell_width > 0.0);
    assert!(renderer.cell_height > 0.0);
}

#[test]
fn test_firple_vn_shaping() {
    use cosmic_text::{Attrs, Buffer, Family, Metrics, Shaping};
    let mut renderer = TextRenderer::new("Firple VN", 13.0, 1.2);
    if renderer.font_family != "Firple VN" {
        // Skip on CI where local user font Firple VN is not installed
        return;
    }

    let sample = "i ì í ĩ ỉ ị Tiếng Việt 日本語 -> != ===";
    let metrics = Metrics::new(13.0, 13.0 * 1.2);
    let mut buffer = Buffer::new(&mut renderer.font_system, metrics);
    let attrs = Attrs::new().family(Family::Name(&renderer.font_family));
    buffer.set_text(&mut renderer.font_system, sample, attrs, Shaping::Advanced);
    buffer.shape_until_scroll(&mut renderer.font_system, false);

    // Verify all glyphs in sample are rendered with Firple VN (no fallback to System Font!)
    for run in buffer.layout_runs() {
        for glyph in run.glyphs {
            let ch = &sample[glyph.start..glyph.end];
            let face_families = renderer.font_system.db().face(glyph.font_id).map(|f| &f.families).unwrap();
            let is_firple_vn = face_families.iter().any(|(name, _)| name == "Firple VN");
            assert!(
                is_firple_vn,
                "Character '{}' should be rendered by 'Firple VN', but got: {:?}",
                ch, face_families
            );
        }
    }
}


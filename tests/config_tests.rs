use celerterm::config::schema::Config;

#[test]
fn test_default_config() {
    let config = Config::default();
    
    // Window defaults
    assert!(config.window.tabs_in_titlebar);
    assert!(!config.window.hide_traffic_lights);
    assert!(config.window.decorations);
    
    // macOS defaults
    assert!(config.macos.option_as_alt);
    
    // Font defaults
    assert_eq!(config.font.size, 14.0);
    assert!(config.font.ligatures);
    
    // Workspace defaults
    assert!(config.workspace.restore_on_startup);
    assert!(config.workspace.save_scrollback);
}

#[test]
fn test_parse_custom_toml() {
    let toml_str = r#"
        [window]
        hide_traffic_lights = true
        tabs_in_titlebar = true
        decorations = false

        [macos]
        option_as_alt = false

        [font]
        family = "FiraCode Nerd Font"
        size = 16.0
        ligatures = true

        [workspace]
        restore_on_startup = true
        save_scrollback = true
    "#;

    let config: Config = toml::from_str(toml_str).expect("Failed to parse custom TOML");
    
    assert!(config.window.hide_traffic_lights);
    assert!(!config.window.decorations);
    assert!(!config.macos.option_as_alt);
    assert_eq!(config.font.family, "FiraCode Nerd Font");
    assert_eq!(config.font.size, 16.0);
    assert!(config.font.ligatures);
}

#[test]
fn test_partial_toml_with_defaults() {
    let toml_str = r#"
        [window]
        hide_traffic_lights = true
    "#;

    let config: Config = toml::from_str(toml_str).expect("Failed to parse partial TOML");
    
    // Overridden field
    assert!(config.window.hide_traffic_lights);
    // Default fallback fields
    assert!(config.window.tabs_in_titlebar);
    assert!(config.macos.option_as_alt);
    assert_eq!(config.font.size, 14.0);
}

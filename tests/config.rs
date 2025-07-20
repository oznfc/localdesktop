#[cfg(target_os = "android")]
mod tests {
    use localdesktop::utils::config::*;
    use std::fs;
    use tempfile::tempdir;

    fn write_config_file(content: &str, base_dir: &str) -> String {
        let path = format!("{}/etc/localdesktop", base_dir);
        fs::create_dir_all(&path).unwrap();
        let file_path = format!("{}/localdesktop.toml", path);
        fs::write(&file_path, content).unwrap();
        file_path
    }

    #[test]
    fn should_handle_configs_without_try() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_str().unwrap();
        let content = r#"
        [user]
        username = "alice"

        [command]
        check = "check-cmd"
        install = "install-cmd"
        launch = "launch-cmd"
    "#;

        let _ = write_config_file(content, root);
        std::env::set_var("ARCH_FS_ROOT", root);

        let config = parse_config();
        assert_eq!(config.user.username, "alice");
        assert_eq!(config.command.check, "check-cmd");
        assert_eq!(config.command.install, "install-cmd");
        assert_eq!(config.command.launch, "launch-cmd");
    }

    #[test]
    fn should_handle_configs_with_try() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_str().unwrap();
        let content = r#"
        [user]
        username = "root"
        try_username = "testuser"

        [command]
        check = "check-cmd"
        try_check = "try-check"
        install = "install-cmd"
        launch = "launch-cmd"
    "#;

        let _ = write_config_file(content, root);
        std::env::set_var("ARCH_FS_ROOT", root);

        let config = parse_config();
        assert_eq!(config.user.username, "testuser");
        assert_eq!(config.command.check, "try-check");
        assert_eq!(config.command.install, "install-cmd");
    }

    #[test]
    fn should_comment_out_try_configs() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_str().unwrap();
        let config_path = write_config_file(
            r#"
        username = "root"
        try_username = "commented"

        check = "normal"
        try_check = "try"
    "#,
            root,
        );
        std::env::set_var("ARCH_FS_ROOT", root);

        let _ = parse_config(); // This triggers rewriting the config file
        let content = fs::read_to_string(&config_path).unwrap();

        assert!(content.contains("# try_username = \"commented\""));
        assert!(content.contains("# try_check = \"try\""));
    }
}

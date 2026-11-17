use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn cli_reproduces_the_readme_animation() {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let path = std::env::temp_dir().join(format!(
        "gale-shapley-replay-{}-{}.svg",
        std::process::id(),
        timestamp.as_nanos(),
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_gale-shapley-stability"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "solve",
            "examples/side-switch.txt",
            "--proposers",
            "both",
            "--animated-svg",
        ])
        .arg(&path)
        .output()
        .unwrap();

    let generated = fs::read_to_string(&path);
    if generated.is_ok() {
        fs::remove_file(&path).unwrap();
    }
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr),
    );
    let readme_animation = include_str!("../docs/images/side-switch-replay.svg");
    assert_eq!(generated.unwrap(), readme_animation.replace("\r\n", "\n"));
}

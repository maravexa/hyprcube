use std::process::Command;

fn version_output(flag: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_hyprcube"))
        .arg(flag)
        .output()
        .unwrap()
}

#[test]
fn long_version_flag_prints_crate_version() {
    let expected = format!("hyprcube {}\n", env!("CARGO_PKG_VERSION"));
    let output = version_output("--version");
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
}

#[test]
fn short_version_flag_prints_crate_version() {
    let expected = format!("hyprcube {}\n", env!("CARGO_PKG_VERSION"));
    let output = version_output("-V");
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
}

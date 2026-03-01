//! Integration tests for the `source` Runfile directive

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod common;

use common::*;
use std::fs;

/// Helper: write an arbitrary file (not necessarily named Runfile) inside `dir`.
fn write_file(dir: &std::path::Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).unwrap();
}

#[test]
fn test_source_absolute_path() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let lib_path = temp_dir.path().join("lib.run");
    fs::write(
        &lib_path,
        r#"
lib_greet() echo "from lib"
"#,
    )
    .unwrap();

    let runfile_content = format!(
        "source {}\nlocal_fn() echo \"local only\"\n",
        lib_path.display()
    );
    create_runfile(temp_dir.path(), &runfile_content);

    // Function defined in the sourced file should be available
    let output = test_command(&binary)
        .arg("lib_greet")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("from lib"));
}

#[test]
fn test_source_relative_path() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    write_file(
        temp_dir.path(),
        "shared.run",
        "helper() echo \"helper result\"\n",
    );

    create_runfile(temp_dir.path(), "source shared.run\nmain() echo \"main\"\n");

    let output = test_command(&binary)
        .arg("helper")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("helper result"));
}

#[test]
fn test_source_does_not_expand_inside_function_body() {
    // A `source` inside a block body must NOT be treated as a Runfile directive.
    // Even if the path points to a valid Runfile, its functions must NOT be merged
    // into the top-level namespace.
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let injected_path = temp_dir.path().join("injected.run");
    fs::write(&injected_path, "injected_fn() echo \"was injected\"\n").unwrap();

    // `source` appears inside a block body — injected_fn must NOT become top-level
    let runfile_content = format!(
        "check() {{\n    source {path}\n    echo \"inside check\"\n}}\n",
        path = injected_path.display()
    );
    create_runfile(temp_dir.path(), &runfile_content);

    // `injected_fn` should NOT be callable — only top-level source directives are expanded
    let output = test_command(&binary)
        .arg("injected_fn")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not found"));
}

#[test]
fn test_sourced_file_functions_override_order() {
    // Functions defined later (after source) override earlier ones from the sourced file.
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    write_file(
        temp_dir.path(),
        "base.run",
        "greet() echo \"hello from base\"\n",
    );

    create_runfile(
        temp_dir.path(),
        "source base.run\ngreet() echo \"hello from local\"\n",
    );

    let output = test_command(&binary)
        .arg("greet")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello from local"));
    assert!(!stdout.contains("hello from base"));
}

#[test]
fn test_source_nonexistent_file_emits_warning() {
    // A `source` pointing to a non-existent file should print a warning and continue.
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        "source /nonexistent/path/does_not_exist.run\nok() echo \"still works\"\n",
    );

    let output = test_command(&binary)
        .arg("ok")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("still works"));
    // A warning about the missing file should appear on stderr
    assert!(String::from_utf8_lossy(&output.stderr).contains("warning"));
}

#[test]
fn test_circular_source_does_not_loop() {
    // A → B → A circular source must terminate without infinite recursion.
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    let runfile_path = temp_dir.path().join("Runfile");
    let other_path = temp_dir.path().join("other.run");

    fs::write(
        &other_path,
        format!(
            "source {}\nother_fn() echo \"other\"\n",
            runfile_path.display()
        ),
    )
    .unwrap();

    fs::write(
        &runfile_path,
        format!("source {}\nmain_fn() echo \"main\"\n", other_path.display()),
    )
    .unwrap();

    let output = test_command(&binary)
        .arg("main_fn")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("main"));
}

#[test]
fn test_source_multiple_files() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    write_file(temp_dir.path(), "a.run", "fn_a() echo \"from a\"\n");
    write_file(temp_dir.path(), "b.run", "fn_b() echo \"from b\"\n");

    create_runfile(temp_dir.path(), "source a.run\nsource b.run\n");

    let output_a = test_command(&binary)
        .arg("fn_a")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute");

    assert!(output_a.status.success());
    assert!(String::from_utf8_lossy(&output_a.stdout).contains("from a"));

    let output_b = test_command(&binary)
        .arg("fn_b")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute");

    assert!(output_b.status.success());
    assert!(String::from_utf8_lossy(&output_b.stdout).contains("from b"));
}

#[test]
fn test_source_in_script_file() {
    // source directives should also work in .run script files executed directly.
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    write_file(temp_dir.path(), "lib.run", "lib_fn() echo \"from lib\"\n");

    let lib_path = temp_dir.path().join("lib.run");
    let script_path = temp_dir.path().join("main.run");

    fs::write(
        &script_path,
        format!("source {}\nlib_fn()\n", lib_path.display()),
    )
    .unwrap();

    let output = test_command(&binary)
        .arg(script_path.to_str().unwrap())
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("from lib"));
}

//! Polyglot language tests (Python, Node, Ruby, shebang)

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod common;

use common::*;
use std::process::Command;

// Python tests

#[test]
fn test_shell_attribute_python() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @shell python
math() {
    import sys
    result = 10 + 20
    print(f"Result: {result}")
}
"#,
    );

    if is_python_available() {
        let output = Command::new(&binary)
            .arg("math")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Result: 30"));
    }
}

#[test]
fn test_shell_attribute_python_with_args() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @shell python
greet() {
    import sys
    name = sys.argv[1] if len(sys.argv) > 1 else "World"
    print(f"Hello, {name}!")
}
"#,
    );

    if is_python_available() {
        let output = Command::new(&binary)
            .arg("greet")
            .arg("Alice")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Hello, Alice!"));
    }
}

#[test]
fn test_shell_python_multiline_with_loop() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @shell python
loop() {
    for i in range(1, 4):
        print(f"Number: {i}")
}
"#,
    );

    if is_python_available() {
        let output = Command::new(&binary)
            .arg("loop")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Number: 1"));
        assert!(stdout.contains("Number: 2"));
        assert!(stdout.contains("Number: 3"));
    }
}

#[test]
fn test_shebang_python_basic() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
py() {
    #!/usr/bin/env python3
    print("Hello from Python shebang")
}
"#,
    );

    if is_python_available() {
        let output = Command::new(&binary)
            .arg("py")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Hello from Python shebang"));
    }
}

#[test]
fn test_shebang_python3_explicit() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
py3() {
    #!/usr/bin/python3
    print("Explicit python3")
}
"#,
    );

    if is_python_available() {
        let output = Command::new(&binary)
            .arg("py3")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Explicit python3"));
    }
}

// Node tests

#[test]
fn test_shell_attribute_node() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r"
# @shell node
js() {
    const result = 15 + 25;
    console.log(`Result: ${result}`);
}
",
    );

    if is_node_available() {
        let output = Command::new(&binary)
            .arg("js")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Result: 40"));
    }
}

#[test]
fn test_shell_attribute_node_with_args() {
    if !is_node_available() {
        return;
    }

    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @shell node
greet() {
    const args = process.argv.slice(1);
    if (args.length > 0) {
        console.log(`Hello, ${args[0]}!`);
    } else {
        console.log("Hello, World!");
    }
}
"#,
    );

    let output = Command::new(&binary)
        .arg("greet")
        .arg("Alice")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, Alice!"));
}

#[test]
fn test_shell_node_multiline_with_loop() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r"
# @shell node
loop() {
    for (let i = 1; i <= 3; i++) {
        console.log(`Count: ${i}`);
    }
}
",
    );

    if is_node_available() {
        let output = Command::new(&binary)
            .arg("loop")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Count: 1"));
        assert!(stdout.contains("Count: 2"));
        assert!(stdout.contains("Count: 3"));
    }
}

#[test]
fn test_shebang_node() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
node_func() {
    #!/usr/bin/env node
    console.log("Hello from Node shebang");
}
"#,
    );

    if is_node_available() {
        let output = Command::new(&binary)
            .arg("node_func")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Hello from Node shebang"));
    }
}

// Ruby tests

#[test]
fn test_shell_attribute_ruby() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @shell ruby
rb() {
    result = 5 * 6
    puts "Result: #{result}"
}
"#,
    );

    if is_ruby_available() {
        let output = Command::new(&binary)
            .arg("rb")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Result: 30"));
    }
}

#[test]
fn test_shebang_ruby() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
ruby_func() {
    #!/usr/bin/env ruby
    puts "Hello from Ruby shebang"
}
"#,
    );

    if is_ruby_available() {
        let output = Command::new(&binary)
            .arg("ruby_func")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Hello from Ruby shebang"));
    }
}

#[test]
fn test_no_shebang_uses_default_shell() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
test() {
    echo "Default shell"
}
"#,
    );

    let output = Command::new(&binary)
        .arg("test")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Default shell"));
}

// Polyglot named parameter tests

#[test]
fn test_python_named_param() {
    if !is_python_available() {
        return;
    }

    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @shell python
greet(name) {
    print(f"Hello, {name}!")
}
"#,
    );

    let output = Command::new(&binary)
        .arg("greet")
        .arg("World")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, World!"), "stdout: {stdout}");
}

#[test]
fn test_python_named_param_with_default() {
    if !is_python_available() {
        return;
    }

    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @shell python
greet(name, greeting = "Hello") {
    print(f"{greeting}, {name}!")
}
"#,
    );

    // Test with default
    let output = Command::new(&binary)
        .arg("greet")
        .arg("World")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, World!"), "stdout: {stdout}");

    // Test overriding default
    let output = Command::new(&binary)
        .arg("greet")
        .arg("World")
        .arg("Hi")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hi, World!"), "stdout: {stdout}");
}

#[test]
fn test_node_named_param() {
    if !is_node_available() {
        return;
    }

    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r"
# @shell node
greet(name) {
    console.log(`Hello, ${name}!`);
}
",
    );

    let output = Command::new(&binary)
        .arg("greet")
        .arg("World")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, World!"), "stdout: {stdout}");
}

#[test]
fn test_node_named_param_with_default() {
    if !is_node_available() {
        return;
    }

    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @shell node
greet(name, greeting = "Hello") {
    console.log(`${greeting}, ${name}!`);
}
"#,
    );

    // Test with default
    let output = Command::new(&binary)
        .arg("greet")
        .arg("World")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, World!"), "stdout: {stdout}");

    // Test overriding default
    let output = Command::new(&binary)
        .arg("greet")
        .arg("World")
        .arg("Hi")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hi, World!"), "stdout: {stdout}");
}

#[test]
fn test_python_no_params_still_works() {
    if !is_python_available() {
        return;
    }

    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @shell python
hello() {
    print("Hello, World!")
}
"#,
    );

    let output = Command::new(&binary)
        .arg("hello")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, World!"), "stdout: {stdout}");
}

#[test]
fn test_ruby_named_param() {
    if !is_ruby_available() {
        return;
    }

    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @shell ruby
greet(name) {
    puts "Hello, #{name}!"
}
"#,
    );

    let output = Command::new(&binary)
        .arg("greet")
        .arg("World")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, World!"), "stdout: {stdout}");
}

#[test]
fn test_ruby_named_param_with_default() {
    if !is_ruby_available() {
        return;
    }

    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r##"
# @shell ruby
greet(name, greeting = "Hello") {
    puts "#{greeting}, #{name}!"
}
"##,
    );

    // Test with default
    let output = Command::new(&binary)
        .arg("greet")
        .arg("World")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, World!"), "stdout: {stdout}");

    // Test overriding default
    let output = Command::new(&binary)
        .arg("greet")
        .arg("World")
        .arg("Hi")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hi, World!"), "stdout: {stdout}");
}

#[test]
fn test_shell_attribute_bash_explicit() {
    let binary = get_binary_path();
    let temp_dir = create_temp_dir();

    create_runfile(
        temp_dir.path(),
        r#"
# @shell bash
bash_func() {
    echo "Explicit bash"
}
"#,
    );

    let output = Command::new(&binary)
        .arg("bash_func")
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Explicit bash"));
}

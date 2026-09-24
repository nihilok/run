#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::similar_names)]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

fn get_binary_path() -> PathBuf {
    let mut path = env::current_exe().unwrap();
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.push("run");
    if !path.exists() {
        let build_output = Command::new("cargo")
            .args(["build", "--bin", "run"])
            .output()
            .expect("Failed to build binary");
        assert!(build_output.status.success());
    }
    path
}

fn create_temp_dir() -> tempfile::TempDir {
    tempfile::TempDir::new().unwrap()
}

fn create_runfile(dir: &std::path::Path, content: &str) {
    let runfile_path = dir.join("Runfile");
    fs::write(runfile_path, content).unwrap();
}

fn test_command(binary: &PathBuf) -> Command {
    let mut cmd = Command::new(binary);
    cmd.env("RUN_NO_GLOBAL_MERGE", "1");
    cmd
}

#[test]
fn test_parallel_dag_speedup() {
    let temp_dir = create_temp_dir();
    let binary = get_binary_path();

    let runfile = r#"
task_a() {
    sleep 0.3
    echo "task_a done"
}

task_b() {
    sleep 0.3
    echo "task_b done"
}

# @depends task_a, task_b
all() {
    echo "all done"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    // Parallel run (-p)
    let start_parallel = Instant::now();
    let output_parallel = test_command(&binary)
        .current_dir(temp_dir.path())
        .args(["-p", "all"])
        .output()
        .unwrap();
    let elapsed_parallel = start_parallel.elapsed();

    assert!(output_parallel.status.success());
    let stdout_parallel = String::from_utf8_lossy(&output_parallel.stdout);
    assert!(stdout_parallel.contains("task_a done"));
    assert!(stdout_parallel.contains("task_b done"));
    assert!(stdout_parallel.contains("all done"));

    // Sequential run (-j 1)
    let start_seq = Instant::now();
    let output_seq = test_command(&binary)
        .current_dir(temp_dir.path())
        .args(["-j", "1", "all"])
        .output()
        .unwrap();
    let elapsed_seq = start_seq.elapsed();

    assert!(output_seq.status.success());
    let stdout_seq = String::from_utf8_lossy(&output_seq.stdout);
    assert!(stdout_seq.contains("task_a done"));
    assert!(stdout_seq.contains("task_b done"));
    assert!(stdout_seq.contains("all done"));

    // Parallel run should finish significantly faster than sequential (0.6s)
    assert!(
        elapsed_parallel.as_millis() < 550,
        "Parallel run took too long: {}ms",
        elapsed_parallel.as_millis()
    );
    assert!(
        elapsed_seq.as_millis() >= 580,
        "Sequential run was unexpectedly fast: {}ms",
        elapsed_seq.as_millis()
    );
}

#[test]
fn test_parallel_attribute_in_runfile() {
    let temp_dir = create_temp_dir();
    let binary = get_binary_path();

    let runfile = r#"
task_a() {
    sleep 0.3
    echo "task_a done"
}

task_b() {
    sleep 0.3
    echo "task_b done"
}

# @depends task_a, task_b
# @parallel
all() {
    echo "all done"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    // Parallel execution via @parallel attribute (without -p flag)
    let start = Instant::now();
    let output = test_command(&binary)
        .current_dir(temp_dir.path())
        .arg("all")
        .output()
        .unwrap();
    let elapsed = start.elapsed();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("task_a done"));
    assert!(stdout.contains("task_b done"));
    assert!(stdout.contains("all done"));

    assert!(
        elapsed.as_millis() < 550,
        "Expected @parallel to run tasks concurrently, took {}ms",
        elapsed.as_millis()
    );

    // Overriding with -j 1 should force sequential
    let start_seq = Instant::now();
    let output_seq = test_command(&binary)
        .current_dir(temp_dir.path())
        .args(["-j", "1", "all"])
        .output()
        .unwrap();
    let elapsed_seq = start_seq.elapsed();

    assert!(output_seq.status.success());
    assert!(
        elapsed_seq.as_millis() >= 580,
        "Expected -j 1 to force sequential execution, took {}ms",
        elapsed_seq.as_millis()
    );
}

#[test]
fn test_parallel_dag_diamond_deduplication() {
    let temp_dir = create_temp_dir();
    let binary = get_binary_path();

    let runfile = r#"
setup() {
    echo "SETUP_RAN"
}

# @depends setup
work_a() {
    echo "WORK_A_RAN"
}

# @depends setup
work_b() {
    echo "WORK_B_RAN"
}

# @depends work_a, work_b
# @parallel
finish() {
    echo "FINISH_RAN"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = test_command(&binary)
        .current_dir(temp_dir.path())
        .arg("finish")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Count occurrences of SETUP_RAN - must be exactly 1 (deduplicated)
    let setup_count = stdout.matches("SETUP_RAN").count();
    assert_eq!(
        setup_count, 1,
        "Expected setup to execute exactly once, but ran {setup_count} times"
    );

    assert!(stdout.contains("WORK_A_RAN"));
    assert!(stdout.contains("WORK_B_RAN"));
    assert!(stdout.contains("FINISH_RAN"));

    // Verify ordering: SETUP before WORK, and WORK before FINISH
    let setup_pos = stdout.find("SETUP_RAN").unwrap();
    let work_a_pos = stdout.find("WORK_A_RAN").unwrap();
    let work_b_pos = stdout.find("WORK_B_RAN").unwrap();
    let finish_pos = stdout.find("FINISH_RAN").unwrap();

    assert!(setup_pos < work_a_pos);
    assert!(setup_pos < work_b_pos);
    assert!(work_a_pos < finish_pos);
    assert!(work_b_pos < finish_pos);
}

#[test]
fn test_parallel_dag_failure_aborts() {
    let temp_dir = create_temp_dir();
    let binary = get_binary_path();

    let runfile = r#"
fail_task() {
    echo "FAIL_TASK_STARTED"
    exit 1
}

pass_task() {
    echo "PASS_TASK_STARTED"
}

# @depends fail_task, pass_task
# @parallel
downstream() {
    echo "DOWNSTREAM_SHOULD_NEVER_RUN"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = test_command(&binary)
        .current_dir(temp_dir.path())
        .arg("downstream")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("DOWNSTREAM_SHOULD_NEVER_RUN"),
        "Downstream task ran despite dependency failure"
    );
}

#[test]
fn test_parallel_dag_structured_output() {
    let temp_dir = create_temp_dir();
    let binary = get_binary_path();

    let runfile = r#"
step1() {
    echo "output 1"
}

step2() {
    echo "output 2"
}

# @depends step1, step2
# @parallel
done_task() {
    echo "all done"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = test_command(&binary)
        .current_dir(temp_dir.path())
        .args(["-o", "json", "done_task"])
        .output()
        .unwrap();

    if !output.status.success() {
        eprintln!("STDOUT: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("STDERR: {}", String::from_utf8_lossy(&output.stderr));
    }
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify it parses as valid JSON structured result
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");
    assert_eq!(json["context"]["function_name"], "done_task");

    let outputs = json["outputs"].as_array().expect("outputs array");
    assert!(
        outputs.len() >= 3,
        "Expected outputs from step1, step2, and done_task"
    );
}

#[test]
fn test_parallel_dag_cycle_detection() {
    let temp_dir = create_temp_dir();
    let binary = get_binary_path();

    let runfile = r#"
# @depends b
a() {
    echo "a"
}

# @depends a
b() {
    echo "b"
}
"#;
    create_runfile(temp_dir.path(), runfile);

    let output = test_command(&binary)
        .current_dir(temp_dir.path())
        .args(["-p", "a"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Circular dependency detected"),
        "Expected circular dependency error message, got: {stderr}"
    );
}

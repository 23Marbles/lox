use std::{
    error::Error,
    ffi::OsStr,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    time::Instant,
};

use libtest_with::{Failed, Measurement};
use lox::tree_walk::{error::diagnostic::Diagnostic, run_file_with_writer};
use regex::Regex;

use std::io::{self, Write};

struct NullWriter;

impl Write for NullWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len()) // pretend we wrote all bytes
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn main() {
    use libtest_with::{Arguments, Trial};

    // Parse command line arguments
    let args = Arguments::from_args();

    let tests: Vec<Trial> = recurse_build_check(Path::new("tests").to_path_buf())
        .into_iter()
        .map(|path| {
            let test_name = path.as_os_str().to_string_lossy().to_string();

            if path.starts_with("tests/benchmark") {
                Trial::bench(test_name, move |test_mode| {
                    match std::panic::catch_unwind(|| {
                        if test_mode {
                            Ok(None)
                        } else {
                            const REPEAT_TIMES: u64 = 100;

                            let mut times = Vec::new();

                            for _ in 0..REPEAT_TIMES {
                                let globals = lox::std_lib::default_lib();
                                let start = Instant::now();
                                run_file_with_writer(&path, globals, &mut NullWriter).map_err(
                                    |errs| {
                                        errs.iter().map(|err| err.to_string()).collect::<String>()
                                    },
                                )?;
                                times.push(start.elapsed().as_nanos() as u64);
                            }

                            Ok(Some(Measurement {
                                avg: (times.iter().sum::<u64>() / REPEAT_TIMES),
                                variance: (times.iter().max().unwrap()
                                    - times.iter().min().unwrap())
                                    / 2,
                            }))
                        }
                    }) {
                        Ok(result) => result,
                        Err(panic) => Err(Failed::from(format!(
                            "Benchmark panicked: {}",
                            format_panic(panic)
                        ))),
                    }
                })
            } else {
                Trial::test(test_name, move || {
                    match std::panic::catch_unwind(|| {
                        check_file(
                            &path,
                            read_file_expectation(&path).map_err(|err| {
                                Failed::from(format!("Could not read expactions: {err}"))
                            })?,
                        )
                    }) {
                        Ok(result) => result,
                        Err(panic) => Err(Failed::from(format!(
                            "Test panicked: {}",
                            format_panic(panic)
                        ))),
                    }
                })
            }
        })
        .collect();

    // Run all tests and exit the application appropriatly.
    libtest_with::run(&args, tests).exit();
}

fn format_panic(panic: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = panic.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else {
        "Unknown panic".to_string()
    }
}

fn recurse_build_check(dir: PathBuf) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Recursively collect from subdirectories
                paths.extend(recurse_build_check(path));
            } else if path.is_file() && path.extension() == Some(OsStr::new("lox")) {
                paths.push(path);
            }
        }
    }

    paths
}

#[derive(Debug, Default, Clone)]
enum Expectation {
    #[default]
    None,
    Ok(Vec<String>),
    Err(Vec<Diagnostic>),
}

/// # Panics
/// Panics if it is not a valid file
fn read_file_expectation(path: &Path) -> Result<Expectation, Box<dyn Error>> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    Ok(check_expectation(contents))
}

fn check_expectation(source: String) -> Expectation {
    let expect_out = Regex::new(r"// expect: (.+)").unwrap();
    let expect_err_gen = Regex::new(r"(?i)error").unwrap();

    let mut expectations = Expectation::default();

    for line in source.lines() {
        if let Some(cap) = expect_out.captures(line) {
            let exp = cap[1].trim();
            match expectations {
                Expectation::None => expectations = Expectation::Ok(vec![exp.to_string()]),
                Expectation::Ok(ref mut items) => items.push(exp.to_string()),
                Expectation::Err(..) => (),
            }
        }
        if let Some(_cap) = expect_err_gen.captures(line) {
            //let exp = cap[1].trim();
            match expectations {
                Expectation::None | Expectation::Ok(..) => expectations = Expectation::Err(vec![]),
                Expectation::Err(ref _diagnostics) => (),
            }
        }
    }

    expectations
}
/*
fn check_file(path: &Path, expectation: Expectation) -> Result<(), Failed> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    match run_with_out(&contents) {
        Ok(out) => match expectation {
            Expectation::Ok(exp_out) => {
                if exp_out == out {
                    Ok(())
                } else {
                    Err(Failed::from(format!(
                        "Expected output:\n{}\nGot:\n{}",
                        exp_out.join("\n"),
                        out.join("\n")
                    )))
                }
            }
            Expectation::Err(err) => Err(Failed::from(format!(
                "Expected errs:\n{}",
                err.iter().map(|e| e.to_string() + "\n").collect::<String>()
            ))),
            Expectation::None => {
                if out.is_empty() {
                    Ok(())
                } else {
                    Err(Failed::from(format!(
                        "Expected no output, got:\n{}",
                        out.join("\n")
                    )))
                }
            }
        },
        Err(err) => match expectation {
            Expectation::None => match err.kind {
                lox::tree_walk::error::run_err::RunErrorKind::IoError(error) => {
                    Err(Failed::from(format!("Io Error {error}")))
                }
                lox::tree_walk::error::run_err::RunErrorKind::Diagnostics(diagnostics) => {
                    if diagnostics.is_empty() {
                        Ok(())
                    } else {
                        Err(Failed::from(format!(
                            "Expected no error, got:\n{}",
                            diagnostics
                                .iter()
                                .map(|e| e.to_string() + "\n")
                                .collect::<String>()
                        )))
                    }
                }
            },
            Expectation::Ok(exp_out) => Err(Failed::from(format!(
                "Expected output:\n{}\nGot error(s)\n{}",
                exp_out.join("\n"),
                err.to_string()
            ))),
            Expectation::Err(_diagnostics) => Ok(()),
        },
    }
}
*/

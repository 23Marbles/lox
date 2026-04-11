#![allow(dead_code)]
#![allow(unused_imports)]
use std::{
    error::Error,
    ffi::OsStr,
    fmt::{Display, Formatter},
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    str::FromStr,
    time::Instant,
};

use libtest_with::{Failed, Measurement};
use lox::{
    Runner, SharedWriter,
    tree_walk::error_codes::{self, ErrorCode},
};
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

#[test]
fn test_main() {
    main();
}

fn main() {
    use libtest_with::{Arguments, Trial};

    // Parse command line arguments
    let args = Arguments::from_args();

    let tests: Vec<Trial> = recurse_build_check({
        let optional_root = std::env::var("TEST_ROOT");
        if let Ok(path) = optional_root {
            Path::new(&path).to_path_buf()
        } else {
            Path::new("tests").to_path_buf()
        }
    })
    .into_iter()
    .map(|path| {
        let test_name = path.as_os_str().to_string_lossy().to_string();
        Trial::test(test_name, move || {
            match std::panic::catch_unwind(|| {
                check_file(
                    &path,
                    read_file_expectation(&path)
                        .map_err(|err| Failed::from(format!("Could not read expactions: {err}")))?,
                )
            }) {
                Ok(result) => result,
                Err(panic) => Err(Failed::from(format!(
                    "Test panicked: {}",
                    format_panic(panic)
                ))),
            }
        })
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
    /// The error code
    Err(Vec<ErrExpectation>),
}

#[derive(Debug, Clone)]
struct ErrExpectation {
    code: ErrorCode,
}

impl Display for ErrExpectation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code)
    }
}

#[derive(Debug, Clone)]
enum ExpectationFromStrError {
    CouldNotParse,
    ErrorCodeParseError(error_codes::ErrorCodeFromStrErr),
}

impl Display for ExpectationFromStrError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ExpectationFromStrError::CouldNotParse => write!(f, "could not parse expectation"),
            ExpectationFromStrError::ErrorCodeParseError(err) => {
                write!(f, "error code parse error: {}", err)
            }
        }
    }
}

impl Error for ExpectationFromStrError {}

impl From<error_codes::ErrorCodeFromStrErr> for ExpectationFromStrError {
    fn from(err: error_codes::ErrorCodeFromStrErr) -> Self {
        ExpectationFromStrError::ErrorCodeParseError(err)
    }
}

impl FromStr for ErrExpectation {
    type Err = ExpectationFromStrError;

    fn from_str(s: &str) -> Result<Self, <Self as FromStr>::Err> {
        Ok(ErrExpectation { code: s.parse()? })
    }
}

impl FromStr for Expectation {
    type Err = ExpectationFromStrError;

    fn from_str(s: &str) -> Result<Self, <Self as FromStr>::Err> {
        let expect_out = Regex::new(r"// expect: (.+)").unwrap();
        let expect_err_gen = Regex::new(r"// error: (.+)").unwrap();

        let mut expectations = Expectation::default();

        for line in s.lines() {
            if let Some(cap) = expect_err_gen.captures(line) {
                let err = &cap[1];
                match expectations {
                    Expectation::None => expectations = Expectation::Err(vec![err.parse()?]),
                    Expectation::Ok(_) => expectations = Expectation::Err(vec![err.parse()?]),
                    Expectation::Err(ref mut err_expectations) => {
                        err_expectations.push(err.parse()?)
                    }
                }
            } else if let Some(cap) = expect_out.captures(line) {
                let out = cap[1].to_owned();
                match expectations {
                    Expectation::None => expectations = Expectation::Ok(vec![out]),
                    Expectation::Ok(ref mut items) => items.push(out),
                    // Do not overwrite the existing Err expectation
                    Expectation::Err(_) => (),
                }
            }
        }

        Ok(expectations)
    }
}

/// # Panics
/// Panics if it is not a valid file
fn read_file_expectation(path: &Path) -> Result<Expectation, Box<dyn Error>> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    Ok(contents.parse()?)
}

fn check_file(path: &Path, expectation: Expectation) -> Result<(), Failed> {
    let writer = SharedWriter::default();

    let out = Runner::new_with_writer(Box::new(writer.clone()))
        .with_globals(lox::std_lib::default_lib())
        .run_file(path)
        .map(|_| writer.read())
        .map_err(|errs| {
            errs.iter()
                .map(|e| e.code.as_ref().expect("Should have a code assigned"))
                .cloned()
                .collect::<Vec<_>>()
        });

    match out {
        Ok(out) => match expectation {
            Expectation::Ok(exp_out) => {
                if exp_out == out.lines().collect::<Vec<_>>() {
                    Ok(())
                } else {
                    Err(Failed::from(format!(
                        "Expected output:\n{}\nGot:\n{}",
                        exp_out.join("\n"),
                        out
                    )))
                }
            }
            Expectation::Err(errs) => Err(Failed::from(format!(
                "Expected errs:\n{}",
                errs.iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            ))),
            Expectation::None => {
                if out.is_empty() {
                    Ok(())
                } else {
                    Err(Failed::from(format!("Expected no output, got:\n{}", out)))
                }
            }
        },
        Err(errs) => match expectation {
            Expectation::None => Err(Failed::from("Expected no output, got error")),
            Expectation::Ok(exp_out) => Err(Failed::from(format!(
                "Expected output:\n{}\nGot error(s)\n{}",
                exp_out.join("\n"),
                errs.join("\n")
            ))),
            Expectation::Err(exp_errs) => {
                /*
                if codes == errs {
                Ok(())
                } else {
                Err(Failed::from(format!(
                "Expected errs:\n{}\nGot:\n{}",
                codes.join("\n"),
                errs.join("\n")
                )))
                } */
                Ok(())
            }
        },
    }
}

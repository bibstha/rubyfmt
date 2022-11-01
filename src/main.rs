#![deny(warnings, missing_copy_implementations)]

mod updates;

use similar::TextDiff;

use std::ffi::{OsStr, OsString};
use std::fs::{self, metadata, read_to_string, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::exit;

#[derive(Debug)]
enum FileError {
    Io(io::Error),
    SyntaxError,
}

#[derive(Debug, PartialEq, Copy, Clone)]
enum ErrorExit {
    NoExit,
    Exit,
}

fn rubyfmt_file(file_path: &Path) -> Result<(), FileError> {
    let buffer = read_to_string(&file_path).map_err(FileError::Io)?;
    let res = rubyfmt::format_buffer(&buffer);
    match res {
        Ok(res) => {
            let mut file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(file_path)
                .expect("file");
            write!(file, "{}", res).map_err(FileError::Io)?;
            Ok(())
        }
        Err(rubyfmt::RichFormatError::SyntaxError) => Err(FileError::SyntaxError),
        Err(e) => {
            // we're in a formatting loop, so print, and OK
            handle_error_from(e, file_path, ErrorExit::NoExit);
            Ok(())
        }
    }
}

fn rubyfmt_dir(path: &Path) -> io::Result<()> {
    for entry in fs::read_dir(path)? {
        let path = entry?.path();
        if path.is_dir() {
            rubyfmt_dir(&path)?;
        } else if path.extension() == Some(OsStr::new("rb")) {
            let res = rubyfmt_file(&path);
            if let Err(FileError::SyntaxError) = res {
                eprintln!(
                    "warning: {} contains syntax errors, ignoring for now",
                    path.display()
                );
            }
        }
    }
    Ok(())
}

fn format_parts(parts: &[OsString]) {
    for part in parts {
        if let Ok(md) = metadata(part) {
            if md.is_dir() {
                rubyfmt_dir(part.as_ref()).expect("failed to format directory");
            } else if md.is_file() {
                rubyfmt_file(part.as_ref()).expect("failed to format file");
            }
        }
    }
}

fn diff_file(path: &Path) -> String {
    let buffer = read_to_string(&path).expect("Failed to read file");
    let res = rubyfmt::format_buffer(&buffer);
    match res {
        Ok(res) => {
            let diff = TextDiff::from_lines(&buffer, &res);
            let path = path.to_str().unwrap();
            format!("{}", diff.unified_diff().header(path, path))
        }
        Err(e) => {
            // Since this is check and not a formatting loop,
            // we can exit on invalid input
            handle_error_from(e, path, ErrorExit::Exit);
            // We should be exiting in `handle_error_from`,
            // this is just to make the compiler happy
            unreachable!();
        }
    }
}

fn diff_parts(parts: Vec<&Path>) -> Vec<String> {
    let mut diffs = Vec::new();
    for part in parts {
        match metadata(part) {
            Ok(md) => {
                if md.is_dir() {
                    let path_bufs: Vec<PathBuf> = fs::read_dir(part)
                        .expect("Failed to read directory")
                        .into_iter()
                        .map(|entry| entry.expect("Failed to get directory entry").path())
                        .collect();
                    let paths = path_bufs.iter().map(|p| p.as_path()).collect();
                    diffs.append(&mut diff_parts(paths));
                } else if part.extension() == Some(OsStr::new("rb")) {
                    diffs.push(diff_file(part));
                }
            }
            Err(e) => {
                handle_error_from(rubyfmt::RichFormatError::IOError(e), part, ErrorExit::Exit);
            }
        }
    }

    // Remove any blank diffs -- these are no-ops
    diffs.retain(|diff| !diff.is_empty());
    diffs
}

fn handle_error_from(err: rubyfmt::RichFormatError, source: &Path, error_exit: ErrorExit) {
    use rubyfmt::RichFormatError::*;
    let exit_code = err.as_exit_code();
    let e = || {
        if error_exit == ErrorExit::Exit {
            exit(exit_code);
        }
    };
    match err {
        SyntaxError => {
            eprintln!("{} contained invalid ruby syntax", source.display());
            e();
        }
        rubyfmt::RichFormatError::RipperParseFailure(_) => {
            let bug_report = "
🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛
🐛                                                                                              🐛
🐛  Rubyfmt failed to correctly deserialize a tree from ripper. This is absolutely a bug        🐛
🐛  and you should send us a bug report at https://github.com/penelopezone/rubyfmt/issues/new.  🐛
🐛  Ideally you would include the full source code of the program you ran rubyfmt with.         🐛
🐛  If you can't do that for some reason, the best thing you can do is                          🐛
🐛  rerun rubyfmt on this program with the debug binary with `2>log_file` on the end            🐛
🐛  and then send us the log file that gets generated.                                          🐛
🐛                                                                                              🐛
🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛
            ";
            eprintln!("{}", bug_report);
            eprintln!("file was: {}", source.display());
            e();
        }
        IOError(ioe) => {
            eprintln!("IO error occurred while running rubyfmt: {:?}, this may indicate a programming error, please file a bug report at https://github.com/penelopezone/rubyfmt/issues/new", ioe);
            e();
        }
        rubyfmt::RichFormatError::OtherRubyError(s) => {
            eprintln!("A ruby error occurred: {}, please file a bug report at https://github.com/penelopezone/rubyfmt/issues/new", s);
            exit(exit_code);
        }
    }
}

fn main() {
    updates::begin_checking_for_updates();
    let res = rubyfmt::rubyfmt_init();
    if res != rubyfmt::InitStatus::OK as libc::c_int {
        panic!(
            "bad init status: {}",
            rubyfmt::ruby::current_exception_as_rust_string()
        );
    }
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let command = args.get(0).and_then(|x| x.to_str());
    match (command, &*args) {
        // Read from stdin
        (_, []) => {
            let mut buffer = String::new();
            io::stdin()
                .read_to_string(&mut buffer)
                .expect("reading from stdin to not fail");
            let res = rubyfmt::format_buffer(&buffer);
            match res {
                Ok(res) => {
                    write!(io::stdout(), "{}", res).expect("write works");
                    io::stdout().flush().expect("flush works");
                }
                Err(e) => handle_error_from(e, Path::new("stdin"), ErrorExit::Exit),
            }
        }
        // In Rust 1.53
        // (Some("--help" | "-h"), _) => {
        (Some("--help"), _) | (Some("-h"), _) => {
            eprintln!("{}", include_str!("../README.md"));
            exit(0);
        }
        (Some("--internal-fetch-latest-version"), _) => {
            updates::fetch_latest_version().unwrap();
        }
        // Single file
        (_, [filename]) => {
            if let Ok(md) = metadata(&filename) {
                if md.is_dir() {
                    format_parts(&[filename.clone()])
                } else {
                    let buffer = read_to_string(&filename).expect("file exists");
                    let res = rubyfmt::format_buffer(&buffer);
                    match res {
                        Ok(res) => {
                            write!(io::stdout(), "{}", res).expect("write works");
                            io::stdout().flush().expect("flush works");
                        }
                        Err(e) => handle_error_from(e, filename.as_ref(), ErrorExit::Exit),
                    }
                }
            } else {
                eprintln!("{} does not exist", Path::new(&filename).display());
                exit(rubyfmt::FormatError::IOError as i32)
            }
        }
        (Some("-c" | "--check"), [_, parts @ ..]) => {
            let paths = parts.iter().map(|part| part.as_ref()).collect();
            let text_diffs = diff_parts(paths);
            if text_diffs.is_empty() {
                // All good! No changes to make
                exit(0);
            } else {
                for diff in text_diffs {
                    write!(io::stdout(), "{}", diff).expect("Could not write to stdout");
                    io::stdout().flush().expect("flush works");
                }
                exit(1);
            }
        }
        // Multiple files
        (Some("-i"), [_, parts @ ..]) | (_, parts) => {
            format_parts(parts);
        }
    }
    updates::report_if_update_available();
}

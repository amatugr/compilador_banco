use std::collections::HashMap;
use std::env::{args, current_dir};
use std::fs::{create_dir, create_dir_all, read_dir, read_to_string, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{exit, Command, Stdio};

use chrono::prelude::*;
use human_panic::setup_panic;
#[cfg(not(target_os = "windows"))]
use spinners::{Spinner, Spinners};
use which::which;
use yansi::Paint;

fn main() {
    setup_panic!();

    let cwd = current_dir().unwrap_or(PathBuf::from("/"));

    if let Err(e) = which("pandoc") {
        eprintln!(
            "{} Could not find suitable pandoc installation ({})",
            Paint::red("ERROR").invert().bold(),
            e,
        );
        exit(1);
    }

    let args: Vec<String> = args()
        .collect::<Vec<String>>()
        .into_iter()
        .skip(1)
        .collect();

    let html_dir = cwd
        .join(args.get(0).unwrap_or(&("".to_owned())))
        .join("html");
    if !html_dir.exists() {
        if let Err(e) = create_dir(&html_dir) {
            eprintln!(
                "{} Error while creating html directory ({})",
                Paint::red("ERROR").invert().bold(),
                e,
            );
            exit(1);
        }
    }

    let src_dir = cwd.join(&args.get(0).unwrap_or(&("".to_owned())));
    let mut times = get_times(&src_dir);

    let files = find_tex(&src_dir);
    for file in files {
        let path = cwd.join(file);
        if !path.exists() {
            eprintln!(
                "{} ./{}: File does not exist",
                Paint::red("ERROR").invert().bold(),
                Paint::new(
                    path.strip_prefix(&cwd) // TODO Strip src_dir instead?
                        .unwrap()
                        .as_os_str()
                        .to_str()
                        .unwrap_or("UNNAMED")
                )
                .bold(),
                // Paint::red("File does not exist")
            );
            continue;
        } else if DateTime::<Utc>::from(path.metadata().unwrap().modified().unwrap())
            > DateTime::<Utc>::from_utc(
                NaiveDateTime::parse_from_str(
                    &times
                        .get(path.canonicalize().unwrap().to_str().unwrap())
                        .unwrap_or(&(String::from("0"))),
                    "%s",
                )
                .unwrap(),
                Utc,
            )
        {
            println!(
                "{} ./{}: Compiling LaTeX to HTML",
                Paint::cyan("INFO").invert().bold(),
                Paint::new(
                    path.strip_prefix(&cwd)
                        .unwrap()
                        .as_os_str()
                        .to_str()
                        .unwrap_or("UNNAMED")
                )
                .bold()
            );
            #[cfg(not(target_os = "windows"))]
            let sp = Spinner::new(&Spinners::OrangeBluePulse, "Executing pandoc".into());
            create_dir_all(
                html_dir
                    .join(path.strip_prefix(&src_dir).unwrap())
                    .parent()
                    .unwrap_or(Path::new("/")),
            )
            .unwrap();
            println!(
                "{}",
                html_dir
                    .join(path.strip_prefix(&src_dir).unwrap())
                    .to_str()
                    .unwrap()
                    .rsplit_once(".")
                    .unwrap()
                    .0
            );
            let mut cmd = Command::new("pandoc")
                .args([
                    &path.to_str().unwrap(),
                    "-f",
                    "latex",
                    "-t",
                    "html",
                    "-o",
                    &(html_dir
                        .join(path.strip_prefix(&src_dir).unwrap())
                        .to_str()
                        .unwrap()
                        .rsplit_once(".")
                        .unwrap()
                        .0
                        .to_owned()
                        + ".html"),
                    "--katex",
                ])
                .spawn()
                .unwrap();
            cmd.wait().expect("Command wasn't running");
            #[cfg(not(target_os = "windows"))]
            {
                sp.message("Successfully compiled \u{2705}".to_owned());
                std::thread::sleep(std::time::Duration::from_millis(90)); // Give time to change message
                sp.stop();
            }
            #[cfg(target_os = "windows")]
            println!(
                "Successfully compiled {} \u{2705}",
                path.strip_prefix(&cwd)
                    .unwrap()
                    .as_os_str()
                    .to_str()
                    .unwrap_or("UNNAMED")
            );
        } else {
            println!(
                "{} ./{}: No changes since last compilation",
                Paint::cyan("INFO").invert().bold(),
                Paint::new(
                    path.strip_prefix(&cwd)
                        .unwrap()
                        .as_os_str()
                        .to_str()
                        .unwrap_or("UNNAMED")
                )
                .bold()
            );
        }

        // Update compilation time in save_times
        times.insert(
            path.canonicalize().unwrap().to_str().unwrap().to_owned(),
            Utc::now().timestamp().to_string(),
        );

        println!();
    }
    save_times(&src_dir, times);
}

fn find_tex(base: &PathBuf) -> Vec<PathBuf> {
    let mut matches: Vec<PathBuf> = Vec::new();
    if !base.is_dir() {
        return vec![];
    };
    match read_dir(base) {
        Ok(read) => {
            for item in read {
                let item = item.unwrap().path();
                if item.is_file() && item.extension().unwrap_or_default() == "tex" {
                    matches.push(item);
                } else if item.is_dir() {
                    matches.append(&mut find_tex(&item))
                }
            }
        }
        Err(e) => println!(
            "{} {} {}: {}",
            Paint::yellow("WARN").invert().bold(),
            Paint::new("Could not read directory"),
            base.to_str().unwrap_or("UNKNOWN"),
            e
        ),
    }

    matches
}

fn get_times(dir: &PathBuf) -> HashMap<String, String> {
    let mut map = HashMap::new();

    let contents = match read_to_string(dir.join(".compilador_banco")) {
        Ok(res) => res,
        Err(e) => {
            println!(
                "{} {}: {}",
                Paint::yellow("WARN").invert().bold(),
                Paint::new("Load modification times table"),
                e
            );
            return map;
        }
    };

    for line in contents.lines() {
        let (filename, time) = line.split_once(";").unwrap();
        map.insert(filename.to_owned(), time.to_owned());
    }

    map
}

// ! REMEMBER TO USE .canonicalize on all files before sending to save and also when comparing
fn save_times(dir: &PathBuf, map: HashMap<String, String>) {
    let mut saves_file = File::create(dir.join(".compilador_banco")).unwrap();
    for (filename, time) in map {
        if let Err(e) = writeln!(saves_file, "{};{}", filename, time) {
            eprintln!(
                "{} Failed to write to saves file time for {} ({})",
                Paint::red("ERROR").invert().bold(),
                filename,
                e
            );
        };
    }
}

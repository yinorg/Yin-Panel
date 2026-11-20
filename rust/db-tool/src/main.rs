//! `yin-db` — the Rust counterpart of the Go backend's schema upgrade and
//! `migrate-db` command.

use std::process::ExitCode;

use yin_db_tool::{copy, load_conf, upgrade};

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("upgrade") => match args.get(1) {
            Some(path) => run_upgrade(path).await,
            None => Err("usage: yin-db upgrade <conf.yaml>".to_string()),
        },
        Some("copy") => run_copy(&args[1..]).await,
        _ => Err(
            "usage: yin-db upgrade <conf.yaml> | yin-db copy --source <conf> --target <conf>"
                .to_string(),
        ),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("yin-db: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run_upgrade(path: &str) -> Result<(), String> {
    let config = load_conf(path)?;
    if config.driver() != "sqlite" {
        return Err(format!(
            "upgrade runs on a SQLite deployment; this one uses {}",
            config.driver()
        ));
    }
    let file = config.sqlite_path();
    if !file.is_file() {
        return Err(format!("database not found: {}", file.display()));
    }
    println!("Upgrading {}", file.display());
    let applied = upgrade::upgrade(&file).await?;
    if applied.is_empty() {
        println!("Already up to date.");
    } else {
        for step in applied {
            println!("  {step}");
        }
    }
    println!("Upgrade complete.");
    Ok(())
}

async fn run_copy(args: &[String]) -> Result<(), String> {
    let mut source = None;
    let mut target = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--source" | "-source" => {
                source = args.get(index + 1).cloned();
                index += 2;
            }
            "--target" | "-target" => {
                target = args.get(index + 1).cloned();
                index += 2;
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let (Some(source), Some(target)) = (source, target) else {
        return Err(
            "usage: yin-db copy --source <conf.yaml> --target <conf.target.yaml>".to_string(),
        );
    };
    copy::run(&load_conf(&source)?, &load_conf(&target)?).await
}

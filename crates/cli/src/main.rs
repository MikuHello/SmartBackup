use clap::{Parser, Subcommand};
use serde_json::{Value, json};
use smart_backup_core::{App, error::Failure};
use std::{path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(
    name = "smart-backup",
    version,
    about = "Immutable, verified file backup — CLI prototype"
)]
struct Cli {
    #[arg(long, global = true)]
    home: Option<PathBuf>,
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Job {
        #[command(subcommand)]
        command: JobCommand,
    },
    Engine {
        #[command(subcommand)]
        command: EngineCommand,
    },
    Run {
        job: String,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        dry_run: bool,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    History {
        #[command(subcommand)]
        command: HistoryCommand,
    },
    Verify {
        archive: PathBuf,
    },
    Archive {
        #[command(subcommand)]
        command: ArchiveCommand,
    },
    Restore {
        archive: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}
#[derive(Subcommand)]
enum ConfigCommand {
    Validate,
}
#[derive(Subcommand)]
enum ArchiveCommand {
    List {
        archive: PathBuf,
        #[arg(long)]
        query: Option<String>,
    },
}
#[derive(Subcommand)]
enum HistoryCommand {
    List {
        #[arg(long)]
        job: Option<String>,
    },
    Show {
        id: String,
    },
}
#[derive(Subcommand)]
enum EngineCommand {
    Configure {
        #[arg(long)]
        path: PathBuf,
    },
}
#[derive(Subcommand)]
enum JobCommand {
    Create {
        name: String,
        #[arg(long, required = true)]
        source: Vec<String>,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        rule: Vec<String>,
        #[arg(long)]
        follow_gitignore: bool,
    },
    Show {
        job: String,
    },
    Edit {
        job: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        rule: Vec<String>,
        #[arg(long, conflicts_with = "rule")]
        clear_rules: bool,
    },
    Enable {
        job: String,
    },
    Disable {
        job: String,
    },
    Delete {
        job: String,
    },
    List,
}
fn execute(app: &App, command: Command) -> anyhow::Result<Value> {
    match command {
        Command::Engine {
            command: EngineCommand::Configure { path },
        } => Ok(serde_json::to_value(app.configure_engine(&path)?)?),
        Command::Run {
            job,
            force,
            dry_run,
        } => {
            if dry_run {
                app.preview(&job)
            } else {
                Ok(serde_json::to_value(app.run(&job, force)?)?)
            }
        }
        Command::Config {
            command: ConfigCommand::Validate,
        } => app.validate_config(),
        Command::History {
            command: HistoryCommand::List { job },
        } => app.history(job.as_deref()),
        Command::History {
            command: HistoryCommand::Show { id },
        } => Ok(serde_json::to_value(app.history_show(&id)?)?),
        Command::Verify { archive } => app.verify(&archive),
        Command::Archive {
            command: ArchiveCommand::List { archive, query },
        } => app.archive_list(&archive, query.as_deref()),
        Command::Restore { archive, output } => app.restore(&archive, &output),
        Command::Job { command } => match command {
            JobCommand::Create {
                name,
                source,
                output,
                rule,
                follow_gitignore,
            } => Ok(serde_json::to_value(app.create_job(
                &name,
                &source,
                &output,
                &rule,
                follow_gitignore,
            )?)?),
            JobCommand::Show { job } => Ok(serde_json::to_value(app.job(&job)?)?),
            JobCommand::List => app.jobs(),
            JobCommand::Edit {
                job,
                name,
                description,
                rule,
                clear_rules,
            } => Ok(serde_json::to_value(app.edit_job(
                &job,
                smart_backup_core::Changes {
                    name,
                    description,
                    enabled: None,
                    rules: if clear_rules || !rule.is_empty() {
                        Some(rule)
                    } else {
                        None
                    },
                },
            )?)?),
            JobCommand::Enable { job } => Ok(serde_json::to_value(app.edit_job(
                &job,
                smart_backup_core::Changes {
                    enabled: Some(true),
                    ..Default::default()
                },
            )?)?),
            JobCommand::Disable { job } => Ok(serde_json::to_value(app.edit_job(
                &job,
                smart_backup_core::Changes {
                    enabled: Some(false),
                    ..Default::default()
                },
            )?)?),
            JobCommand::Delete { job } => Ok(serde_json::to_value(app.delete_job(&job)?)?),
        },
    }
}
fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let help = matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            );
            if std::env::args_os().any(|a| a == "--json") && !help {
                println!(
                    "{}",
                    json!({"schema_version":1,"status":"failed","reason_code":"invalid_arguments","message":"Invalid arguments; run smart-backup --help"})
                );
            } else {
                let _ = error.print();
            }
            return ExitCode::from(if help { 0 } else { 4 });
        }
    };
    let home = cli.home.unwrap_or_else(|| {
        if let Some(value) = std::env::var_os("SMART_BACKUP_HOME") {
            PathBuf::from(value)
        } else if let Some(value) = std::env::var_os("LOCALAPPDATA") {
            PathBuf::from(value).join("SmartBackup")
        } else {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| ".".into()))
                .join(".local/share/smart-backup")
        }
    });
    let read_only = matches!(&cli.command, Command::Run { dry_run: true, .. });
    let result = smart_backup_core::install_cancel_handler()
        .and_then(|_| {
            if read_only {
                App::open_read_only(&home)
            } else if let Command::Job {
                command: JobCommand::Create { source, .. },
            } = &cli.command
            {
                App::open_with_sources(&home, source)
            } else {
                App::open(&home)
            }
        })
        .and_then(|app| execute(&app, cli.command));
    let (code, value) = match result {
        Ok(data) => {
            let status = data
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("success");
            let reason = data
                .get("reason_code")
                .and_then(Value::as_str)
                .unwrap_or("ok");
            let code = match status {
                "warning" => 1,
                "skipped" => 2,
                "cancelled" => 3,
                "failed" => 6,
                _ => 0,
            };
            (
                code,
                json!({"schema_version":1,"status":status,"reason_code":reason,"data":data}),
            )
        }
        Err(error) => {
            let (code, reason) = error
                .downcast_ref::<Failure>()
                .map(|e| (e.exit, e.reason))
                .unwrap_or((5, "environment_error"));
            (
                code,
                json!({"schema_version":1,"status":if code==3{"cancelled"}else{"failed"},"reason_code":reason,"message":format!("{error:#}")}),
            )
        }
    };
    if cli.json {
        println!("{}", value);
    } else {
        println!("{}", serde_json::to_string_pretty(&value).unwrap());
    }
    ExitCode::from(code as u8)
}

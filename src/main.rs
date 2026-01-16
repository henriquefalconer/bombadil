use ::url::Url;
use anyhow::Result;
use clap::Parser;
use std::{
    path::{Path, PathBuf},
    str::FromStr,
};
use tempfile::TempDir;

use antithesis_browser::{
    browser::BrowserOptions,
    proxy::Proxy,
    runner::{Runner, RunnerOptions},
    trace::writer::TraceWriter,
};

#[derive(Parser)]
#[command(version, about)]
struct CLI {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    Test {
        origin: Origin,
        #[arg(long)]
        seed: Option<String>,
        #[arg(long, default_value_t = false)]
        headless: bool,
        #[arg(long, default_value_t = false)]
        no_sandbox: bool,
        #[arg(long, default_value_t = 1024)]
        width: u16,
        #[arg(long, default_value_t = 768)]
        height: u16,
        #[arg(long, default_value_t = false)]
        exit_on_violation: bool,
        #[arg(long)]
        output_path: Option<PathBuf>,
    },
    Proxy {
        #[arg(long)]
        port: u16,
    },
}

#[derive(Clone)]
struct Origin {
    url: Url,
}

impl FromStr for Origin {
    type Err = url::ParseError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Url::parse(s)
            .or(Url::parse(&format!(
                "file://{}",
                std::path::absolute(s)
                    .expect("invalid path")
                    .to_str()
                    .expect("invalid path")
            )))
            .map(|url| Origin { url })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let env = env_logger::Env::default().default_filter_or("info");
    env_logger::Builder::from_env(env)
        .format_timestamp_millis()
        .format_target(true)
        // Until we hav a fix for https://github.com/mattsse/chromiumoxide/issues/287
        .filter_module("chromiumoxide::browser", log::LevelFilter::Error)
        .init();
    let cli = CLI::parse();
    match cli.command {
        Command::Test {
            origin,
            seed: _,
            headless,
            width,
            height,
            no_sandbox,
            exit_on_violation,
            output_path,
        } => {
            let output_path = match output_path {
                Some(path) => path,
                None => TempDir::with_prefix("states_")?.keep().to_path_buf(),
            };

            let user_data_directory = TempDir::with_prefix("user_data_")?;
            let browser_options = BrowserOptions {
                headless,
                user_data_directory: user_data_directory.path().to_path_buf(),
                width,
                height,
                no_sandbox,
                proxy: None,
            };
            let runner = Runner::new(
                origin.url,
                RunnerOptions {
                    stop_on_violation: exit_on_violation,
                },
                &browser_options,
            )
            .await?;
            let mut events = runner.start();
            let mut writer = TraceWriter::initialize(output_path).await?;

            let exit_code: anyhow::Result<Option<i32>> = async {
                loop {
                    match events.next().await {
                        Ok(Some(
                            antithesis_browser::runner::RunEvent::NewState {
                                state,
                                last_action,
                                violation,
                            },
                        )) => {
                            writer
                                .write(last_action, state, violation.clone())
                                .await?;

                            if let Some(violation) = violation {
                                log::error!("violation: {}", violation);
                                if exit_on_violation {
                                    break Ok(Some(2));
                                }
                            }
                        }
                        Ok(None) => break Ok(None),
                        Err(err) => {
                            eprintln!("next run event failure: {}", err);
                            break Ok(Some(1));
                        }
                    }
                }
            }
            .await;

            events.shutdown().await?;

            if let Some(exit_code) = exit_code? {
                std::process::exit(exit_code);
            }

            Ok(())
        }
        Command::Proxy { port } => {
            let mut proxy = Proxy::spawn(port).await?;
            log::info!("proxy started on 127.0.0.1:{}", proxy.port);
            Ok(proxy.done().await)
        }
    }
}

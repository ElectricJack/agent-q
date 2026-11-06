use clap::{CommandFactory, FromArgMatches};
use serde_json::{Value, json};
#[tokio::main]
async fn main() -> std::process::ExitCode {
    let program = aqv2::program();
    let json_output = std::env::args()
        .take_while(|a| a != "--")
        .any(|a| a == "--json");
    let parsed = aqv2::cli::Cli::command()
        .name(program)
        .bin_name(program)
        .try_get_matches()
        .and_then(|m| aqv2::cli::Cli::from_arg_matches(&m));
    let cli = match parsed {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            let _ = error.print();
            return std::process::ExitCode::SUCCESS;
        }
        Err(error) => {
            if json_output {
                print_json(
                    json!({"schema_version":1,"error":aqv2::error("invalid_arguments",&format!("Invalid command arguments; use {program} --help"))}),
                );
            } else {
                let _ = error.print();
            }
            return std::process::ExitCode::from(2);
        }
    };
    let json_output = json_output || cli.json;
    let render = cli.commands.command.render();
    match aqv2::run(cli).await {
        Ok(value) => {
            if json_output {
                print_json(json!({"schema_version":1,"data":value}));
            } else {
                println!("{}", aqv2::render::render(render, &value));
            }
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            let code = aqv2::exit_code(&error);
            if json_output {
                print_json(json!({"schema_version":1,"error":error}));
            } else if error["effect"] == "not_applied" && error["command"].is_string() {
                eprintln!("{program}: {}", aqv2::render::refusal(&error));
            } else {
                // JSON quoting keeps server/input control characters out of the terminal.
                eprintln!("{}: {}", error["code"], error["message"]);
                if let Some(id) = error["operation_id"].as_str() {
                    eprintln!("operation_id: {id}");
                }
                if let Some(recovery) = error["recovery"].as_str() {
                    eprintln!("Recover with: {recovery}");
                }
            }
            std::process::ExitCode::from(code)
        }
    }
}
fn print_json(value: Value) {
    println!("{}", value);
}

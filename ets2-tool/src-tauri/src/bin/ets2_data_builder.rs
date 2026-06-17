use std::path::PathBuf;

use ets2_tool_lib::shared::ets2data;

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(|value| value.as_str()) == Some("powertrain") {
        run_powertrain_builder(&args[1..]);
        return;
    }

    let repo_root = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(ets2data::default_repo_root);
    match ets2data::build_datasets(&repo_root) {
        Ok(summary) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&summary).unwrap_or_default()
            );
        }
        Err(error) => {
            eprintln!("{}", error);
            std::process::exit(1);
        }
    }
}

fn run_powertrain_builder(args: &[String]) {
    let Some(source_path) = args.first().map(PathBuf::from) else {
        eprintln!(
            "usage: ets2_data_builder powertrain <official_scs_or_extracted_dir> [repo_root] [game_version] [game]"
        );
        std::process::exit(1);
    };
    let repo_root = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(ets2data::default_repo_root);
    let game_version = args.get(2).map(|value| value.as_str()).unwrap_or("unknown");
    let game = args.get(3).map(|value| value.as_str()).unwrap_or("ets2");

    match ets2data::powertrain::build_powertrain_catalog(
        &repo_root,
        &source_path,
        game,
        game_version,
    ) {
        Ok(summary) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&summary).unwrap_or_default()
            );
        }
        Err(error) => {
            eprintln!("{}", error);
            std::process::exit(1);
        }
    }
}

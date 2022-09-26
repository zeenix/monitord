use std::path::PathBuf;
use std::str::FromStr;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use anyhow::Result;
use clap::Parser;
use clap_verbosity_flag::InfoLevel;
use configparser::ini::Ini;
use log::debug;
use log::error;
use log::info;

pub mod networkd;

const LONG_ABOUT: &str = "monitord: Know how happy your systemd is! 😊";

/// Clap CLI Args struct with metadata in help output
#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = LONG_ABOUT)]
struct Cli {
    /// Location of your monitord config
    #[clap(short, long, value_parser, default_value = "/etc/monitord.conf")]
    config: String,
    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity<InfoLevel>,
}

// TODO: Add other components as support is added
#[derive(serde::Serialize, serde::Deserialize, Debug, Eq, PartialEq)]
pub struct MonitordStats {
    networkd: networkd::NetworkdState,
}

fn stat_collector(config: Ini) -> Result<(), String> {
    let daemon_mode = config.getbool("monitord", "daemon").unwrap().unwrap();
    let collect_interval_ms: u128 = <u64 as Into<u128>>::into(
        config
            .getuint("monitord", "daemon_stats_refresh_secs")
            .unwrap()
            .unwrap(),
    ) * 1000;

    let mut monitord_stats = MonitordStats {
        networkd: networkd::NetworkdState {
            interfaces_state: vec![],
            managed_interfaces: 0,
        },
    };
    loop {
        let collect_start_time = Instant::now();

        info!("Starting stat collection run");

        // TODO: Move each collector into a function + thread
        // Run networkd collector if enabled
        if config.getbool("networkd", "enabled").unwrap().unwrap() {
            let networkd_start_path =
                PathBuf::from_str(config.get("networkd", "link_state_dir").unwrap().as_str());
            match networkd::parse_interface_state_files(networkd_start_path.unwrap()) {
                Ok(networkd_stats) => monitord_stats.networkd = networkd_stats,
                Err(err) => error!("networkd stats failed: {}", err),
            }
        }

        let elapsed_runtime = collect_start_time.elapsed().as_millis();
        info!("stat collection run took {}ms", elapsed_runtime);
        println!("{}", serde_json::to_string(&monitord_stats).unwrap());
        if !daemon_mode {
            break;
        }
        let sleep_time = collect_interval_ms - elapsed_runtime;
        info!("stat collection sleeping for {}", sleep_time);
        // TODO: Maybe just default to collect_interval on error
        thread::sleep(Duration::from_millis(sleep_time.try_into().unwrap()));
    }
    Ok(())
}

fn main() -> Result<(), String> {
    let args = Cli::parse();
    env_logger::Builder::new()
        .filter_level(args.verbose.log_level_filter())
        .init();

    info!("{} CLI Args: {:?}", LONG_ABOUT, args);

    debug!("Loading {} config", args.config);
    let mut config = Ini::new();
    let _config_map = config.load(args.config)?;

    stat_collector(config)
}

#![allow(dead_code)]

#[macro_use]
extern crate rust_i18n;

use std::sync::Arc;

use clap::Parser;
use easytier::{
    common::{
        config::{ConfigLoader, ConsoleLoggerConfig, FileLoggerConfig, TomlConfigLoader},
        constants::EASYTIER_VERSION,
        error::Error,
    },
    tunnel::{tcp::TcpTunnelListener, udp::UdpTunnelListener, TunnelListener},
    utils::{init_logger, setup_panic_handler},
};

mod client_manager;
mod db;
mod migrator;
mod restful;

#[cfg(feature = "embed")]
mod web;

rust_i18n::i18n!("locales", fallback = "en");

#[derive(Parser, Debug)]
#[command(name = "easytier-core", author, version = EASYTIER_VERSION , about, long_about = None)]
struct Cli {
    #[arg(short, long, default_value = "et.db", help = t!("cli.db").to_string())]
    db: String,

    #[arg(
        long,
        help = t!("cli.console_log_level").to_string(),
    )]
    console_log_level: Option<String>,

    #[arg(
        long,
        help = t!("cli.file_log_level").to_string(),
    )]
    file_log_level: Option<String>,

    #[arg(
        long,
        help = t!("cli.file_log_dir").to_string(),
    )]
    file_log_dir: Option<String>,

    #[arg(
        long,
        short='c',
        default_value = "22020",
        help = t!("cli.config_server_port").to_string(),
    )]
    config_server_port: u16,

    #[arg(
        long,
        short='p',
        default_value = "udp",
        help = t!("cli.config_server_protocol").to_string(),
    )]
    config_server_protocol: String,

    #[arg(
        long,
        short='a',
        default_value = "11211",
        help = t!("cli.api_server_port").to_string(),
    )]
    api_server_port: u16,

    #[cfg(feature = "embed")]
    #[arg(
        long,
        short='l',
        default_value = "11210",
        help = t!("cli.web_server_port").to_string(),
    )]
    web_server_port: u16,

    #[cfg(feature = "embed")]
    #[arg(
        long,
        help = t!("cli.no_web").to_string(),
        default_value = "false"
    )]
    no_web: bool,
}

pub fn get_listener_by_url(
    l: &url::Url,
) -> Result<Box<dyn TunnelListener>, Error> {
    Ok(match l.scheme() {
        "tcp" => Box::new(TcpTunnelListener::new(l.clone())),
        "udp" => Box::new(UdpTunnelListener::new(l.clone())),
        _ => {
            return Err(Error::InvalidUrl(l.to_string()));
        }
    })
}

#[tokio::main]
async fn main() {
    let locale = sys_locale::get_locale().unwrap_or_else(|| String::from("en-US"));
    rust_i18n::set_locale(&locale);
    setup_panic_handler();

    let cli = Cli::parse();
    let config = TomlConfigLoader::default();
    config.set_console_logger_config(ConsoleLoggerConfig {
        level: cli.console_log_level,
    });
    config.set_file_logger_config(FileLoggerConfig {
        dir: cli.file_log_dir,
        level: cli.file_log_level,
        file: None,
    });
    init_logger(config, false).unwrap();

    // let db = db::Db::new(":memory:").await.unwrap();
    let db = db::Db::new(cli.db).await.unwrap();

    let listener = get_listener_by_url(
        &format!("{}://0.0.0.0:{}", cli.config_server_protocol, cli.config_server_port).parse().unwrap(),
    )
    .unwrap();
    let mut mgr = client_manager::ClientManager::new(db.clone());
    mgr.serve(listener).await.unwrap();
    let mgr = Arc::new(mgr);

    let mut restful_server = restful::RestfulServer::new(
        format!("0.0.0.0:{}", cli.api_server_port).parse().unwrap(),
        mgr.clone(),
        db,
    )
    .await
    .unwrap();

    restful_server.start().await.unwrap();

    #[cfg(feature = "embed")]
    let mut web_server = web::WebServer::new(
        format!("0.0.0.0:{}", cli.web_server_port).parse().unwrap()
    )
    .await
    .unwrap();

    #[cfg(feature = "embed")]
    if !cli.no_web {
        web_server.start().await.unwrap();
    }

    tokio::signal::ctrl_c().await.unwrap();
}

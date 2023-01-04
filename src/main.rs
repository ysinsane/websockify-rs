// Ping service example.
//
// You can install and uninstall this service using other example programs.
// All commands mentioned below shall be executed in Command Prompt with Administrator privileges.
//
// Service installation: `install_service.exe`
// Service uninstallation: `uninstall_service.exe`
//
// Start the service: `net start ping_service`
// Stop the service: `net stop ping_service`
//
// Ping server sends a text message to local UDP port 1234 once a second.
// You can verify that service works by running netcat, i.e: `ncat -ul 1234`.

use tracing_appender::{non_blocking, rolling};
use tracing_error::ErrorLayer;
use tracing_subscriber::{filter::EnvFilter, layer::SubscriberExt, util::SubscriberInitExt, Registry};

use std::env;
extern crate websockify_rs;
#[cfg(feature = "daemonize")]
use websockify_rs::service;
#[cfg(windows)]
fn main() -> windows_service::Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,websockify_rs=debug,tower_http=debug"));
    // 输出到控制台中
    let formatting_layer = fmt::layer().pretty().with_writer(std::io::stderr);
    let log_folder = env::var("WEBSOCKIFY_LOG_FOLDER").unwrap_or("C:/websockify-logs".into());
    let log_name = env::var("WEBSOCKIFY_LOG_FILENAME").unwrap_or("app.log".into());
    let file_appender = rolling::daily(log_folder, log_name);
    // 输出到文件中

    let (non_blocking_appender, _guard) = non_blocking(file_appender);
    let file_layer = fmt::layer().with_ansi(false).with_writer(non_blocking_appender);
    use tracing_subscriber::fmt;
    
    use tracing_subscriber::fmt::time::OffsetTime;
    use time::UtcOffset;
    let offset = UtcOffset::current_local_offset().expect("should get local offset!");
    let timer = OffsetTime::new(offset, time::format_description::well_known::Rfc3339);
    let time_layer = fmt::layer().with_timer(timer);

    // 注册
    Registry::default()
        .with(env_filter)
        // ErrorLayer 可以让 color-eyre 获取到 span 的信息
        .with(ErrorLayer::default())
        .with(formatting_layer)
        .with(file_layer)
        .with(time_layer)
        .init();
    run()
}
#[cfg(feature = "daemonize")]
fn run() -> windows_service::Result<()>{
    service::run()
}
#[cfg(not(feature = "daemonize"))]
fn run()-> windows_service::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let handle = axum_server::Handle::new();
    
    runtime.block_on(async {
       crate::websockify_rs::web::start_server(handle).await;
    });
    windows_service::Result::Ok(())
}
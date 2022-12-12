use clap::Parser;

/// Proxy for socket and websocket
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Args {
    /// folder of the static content
    #[arg(short, long, default_value_t=String::new())]
    pub(crate) web: String,

    /// the socket address of vnc host
    #[arg(short, long, default_value_t=String::from("127.0.0.1:5900"))]
    pub(crate) target: String,

    /// the socket address of websevice
    #[arg(short, long, default_value_t=String::from("127.0.0.1:9000"))]
    pub(crate) source: String,
}
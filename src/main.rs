use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, get_service},
    Router,
};
use clap::Parser;
use futures::{
    sink::SinkExt,
    stream::{SplitSink, SplitStream, StreamExt},
};
use std::net::SocketAddr;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::oneshot::{self, error::TryRecvError},
};
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tracing::{ info};
/// Proxy for socket and websocket
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// folder of the static content
    #[arg(short, long, default_value_t=String::new())]
    web: String,

    /// the socket address of vnc host
    #[arg(short, long, default_value_t=String::from("localhost:5900"))]
    target: String,

    /// the socket address of websevice
    #[arg(short, long, default_value_t=String::from("localhost:9000"))]
    source: String,
}
#[derive(Clone)]
struct AppState {
    addr: SocketAddr,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_websockets=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let assets_dir = args.web.clone(); //PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let target_addr = lookup_host(&args.target)
        .await
        .expect("Wrong target address")
        .next()
        .expect("Wrong target address");

    let state = AppState { addr: target_addr };
    // build our application with some routes
    let mut app = Router::new();
    if &args.web != "" {
        app = app.fallback_service(
            get_service(ServeDir::new(assets_dir).append_index_html_on_directories(true)).handle_error(
                |error: std::io::Error| async move {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Unhandled internal error: {}", error),
                    )
                },
            ),
        )
    }
    app = app
        // routes are matched from bottom to top, so we have to put `nest` at the
        // top since it matches all routes
        .route("/websockify", get(ws_handler).with_state(state))
        // logging so we can see whats going on
        .layer(TraceLayer::new_for_http().make_span_with(DefaultMakeSpan::default().include_headers(true)));

    // run it with hyper
    //let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    //use std::net::{SocketAddr, ToSocketAddrs};
    use tokio::net::lookup_host;
    let addr = lookup_host(&args.source)
        .await
        .expect("Wrong source address")
        .next()
        .expect("Wrong source address");
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr).serve(app.into_make_service()).await.unwrap();
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    // accept connections and process them serially

    // 也要在handle_socket里把从websocket读到的内容写入tcp
    ws.on_upgrade(move |socket| handle_socket(socket, state.addr))
}

async fn handle_socket(socket: WebSocket, addr: SocketAddr) {
    println!("连接vnc target：{}", addr);
    let stream = TcpStream::connect(addr).await.unwrap();
    println!("打开端口成功");
    let (read_half, write_half) = stream.into_split();
    //let reader = BufReader::new(read_half);

    let (s_writer, s_reader) = socket.split();
    let (tx, rx) = oneshot::channel::<bool>();
    let t1 = tokio::spawn(forward(read_half, s_writer, rx));
    let t2 = tokio::spawn(backward(s_reader, write_half, tx));
    tokio::join!(t1, t2).0.unwrap();
    println!("socket_handler exit")
}

/// 处理从socket到webscoket的消息
async fn forward(
    mut reader: OwnedReadHalf,
    mut writer: SplitSink<WebSocket, Message>,
    mut rx: tokio::sync::oneshot::Receiver<bool>,
) {
    loop {
        match rx.try_recv() {
            // The channel is currently empty
            Err(TryRecvError::Empty) => {}
            _ => {
                println!("socket =!=> webscoket");
                return;
            }
        }

        let mut buf = [0; 512 * 1024]; // buffer size 最佳设置公式为 带宽 * 延迟*2（前提是独占所有资源）
        let size = reader.read(&mut buf).await.unwrap();
        println!("sk=>ws:{:4.2} kb", size as f64 / 1024.0);
        if size == 0 {
            info!("no data in socket");
            continue;
        }
        match writer.send(Message::Binary((&buf[..size]).to_vec())).await {
            Ok(_) => {}
            Err(e) => {
                println!("向ws写入数据失败{:?}", e)
            }
        };
    }
}

/// 处理从websocket到scoket的消息
async fn backward(
    mut reader: SplitStream<WebSocket>,
    mut writer: OwnedWriteHalf,
    tx: tokio::sync::oneshot::Sender<bool>,
) {
    while let Some(msg) = reader.next().await {
        if let Ok(msg) = msg {
            match msg {
                Message::Text(t) => {
                    println!("client sent str: {:?}", t);
                }
                Message::Binary(bf) => {
                    println!("ws=>sk:{:4.2} kb", bf.len() as f64 / 1024.0);

                    writer.write(&bf).await.expect("写入失败");
                }
                Message::Ping(_) => {
                    println!("socket ping");
                }
                Message::Pong(_) => {
                    println!("socket pong");
                }
                Message::Close(_) => {
                    println!("client disconnected");
                    return;
                }
            }
        } else {
            println!("client disconnected");
            match tx.send(true) {
                Ok(_) => {
                    println!("********")
                }
                Err(e) => {
                    println!("oneshot 通知发送出错{:?}", e)
                }
            }
            println!("停止处理从websocket到scoket的消息");
            return;
        }
    }
}

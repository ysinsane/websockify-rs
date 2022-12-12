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
use axum_server::Handle;
use clap::Parser;
use futures::{
    sink::SinkExt,
    stream::{SplitSink, SplitStream, StreamExt},
};
use std::net::SocketAddr;
use std::sync::mpsc::Receiver as StdRx;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::oneshot::{self, error::TryRecvError, Receiver},
};
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use crate::args::Args;
use tracing::{info, error, debug};

#[derive(Clone)]
struct AppState {
    addr: SocketAddr,
}
pub async fn graceful_shutdown(handle: Handle, rx: StdRx<()>) {
    // Signal the server to shutdown using Handle.
    match rx.recv(){
        _ => {
            // for some reason, use graceful_shutdown will result in windows service stop fail(just for a while)
            // _handle.graceful_shutdown(Some(grace_wait_time)); 
            handle.shutdown();
        },
    }
}

pub async fn start_server(handle: Handle) {
    let args = match Args::try_parse() {
        Ok(it) => it,
        Err(err) => {error!("Arg parse error:{}", err); panic!("")},
    };
    // tracing_subscriber::registry()
    //     .with(
    //         tracing_subscriber::EnvFilter::try_from_default_env()
    //             .unwrap_or_else(|_| "example_websockets=debug,tower_http=debug".into()),
    //     )
    //     .with(tracing_subscriber::fmt::layer())
    //     .init();
    let assets_dir = args.web.clone(); //PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    info!("source 地址：{}", args.source);
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

    use tokio::net::lookup_host;
    let addr = lookup_host(&args.source)
    .await
    .expect("Wrong source address")
    .next()
    .expect("Wrong source address");
    info!("axum web server listening on {}", addr);
    match axum_server::bind(addr)
        .handle(handle)
        .serve(app.into_make_service())
        .await{
            Ok(_) => {info!("正常结束")},
            Err(e) => {error!("start axum server error: {}",e)},
        };
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    // accept connections and process them serially

    // 也要在handle_socket里把从websocket读到的内容写入tcp
    ws.on_upgrade(move |socket| handle_socket(socket, state.addr))
}

async fn handle_socket(socket: WebSocket, addr: SocketAddr) {
    info!("连接vnc target：{}", addr);
    let stream = TcpStream::connect(addr).await.unwrap();
    info!("打开端口成功");
    let (read_half, write_half) = stream.into_split();
    //let reader = BufReader::new(read_half);

    let (s_writer, s_reader) = socket.split();
    let (tx, rx) = oneshot::channel::<bool>();
    let t1 = tokio::spawn(forward(read_half, s_writer, rx));
    let t2 = tokio::spawn(backward(s_reader, write_half, tx));
    futures::future::join_all(vec![t1, t2]).await;
    info!("socket_handler exit")
}

/// 处理从socket到webscoket的消息
async fn forward(mut reader: OwnedReadHalf, mut writer: SplitSink<WebSocket, Message>, mut rx: Receiver<bool>) {
    loop {
        match rx.try_recv() {
            // The channel is currently empty
            Err(TryRecvError::Empty) => {}
            _ => {
                info!("socket =!=> webscoket");
                return;
            }
        }

        let mut buf = vec![0; 512 * 1024]; // buffer size 最佳设置公式为 带宽 * 延迟*2（前提是独占所有资源）
        let size = reader.read(&mut buf).await.unwrap();
        debug!("sk=>ws:{:4.2} kb", size as f64 / 1024.0);
        if size == 0 {
            info!("no data in socket");
            continue;
        }
        match writer.send(Message::Binary((&buf[..size]).to_vec())).await {
            Ok(_) => {}
            Err(e) => {
                info!("向ws写入数据失败{:?}", e);
                return;
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
                Message::Binary(bf) => {
                    debug!("ws=>sk:{:4.2} kb", bf.len() as f64 / 1024.0);
                    writer.write(&bf).await.expect("写入失败");
                }
                Message::Close(_) => {
                    info!("client initiativly disconnected"); // eg: when client close the webpage
                    return;
                }
                _ => (), // handle no other messages except Binary
            }
        } else {
            info!("client passively disconnected"); // eg: when you disconnect the network
            match tx.send(true) {
                Ok(_) => {}
                Err(e) => {
                    error!("oneshot 通知发送出错{:?}", e)
                }
            }
            return;
        }
    }
}

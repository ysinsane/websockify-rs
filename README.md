## websockify-rs: WebSockets support for windows application/server

This is a rust implement of the [websockify-js](https://github.com/novnc/websockify-js), which is part of the
[noVNC](https://github.com/kanaka/noVNC) project.

At the most basic level, websockify just translates WebSockets traffic
to normal socket traffic. Websockify accepts the WebSockets handshake,
parses it, and then begins forwarding traffic between the client and
the target in both directions.

Note that this is the Rust version of websockify. The primary
project is the [Python version of websockify](https://github.com/novnc/websockify).

To run websockify-rs:

    install the environment so you can build a rust application
    cd websockify-rs
    cargo build --release
    run the executable with the proper options

```
Options:
  -w, --web <WEB>        folder of the static content
  -t, --target <TARGET>  the socket address of vnc host [default: localhost:5900], you can overide this value by specify a new value after the websockt path, eg: /websockify/192.168.20.1:5900
  -s, --source <SOURCE>  the socket address of websevice [default: localhost:9000]
  -h, --help             Print help information
  -V, --version          Print version information
```
To build an executable for windows service
```
cargo run --bin websockify-rs --features daemonize
```

### Todo

- [x] WebSocket/Socket Proxy
- [x] Windows Daemonize Support
- [ ] HTTPS Support
- [ ] Message Logging

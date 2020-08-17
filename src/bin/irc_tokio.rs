use futures_util::{future, pin_mut, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

#[tokio::main]
async fn main() {
    let connect_addr = "ws://irc-ws.chat.twitch.tv:80";

    let url = url::Url::parse(&connect_addr).unwrap();

    let (stdin_tx, stdin_rx) = futures_channel::mpsc::unbounded();
    tokio::spawn(read_stdin(stdin_tx.clone()));

    let (ws_stream, _) = connect_async(url).await.expect("Failed to connect");
    println!("WebSocket handshake has been successfully completed");

    let (write, read) = ws_stream.split();

    let stdin_to_ws = stdin_rx.map(Ok).forward(write);
    let ws_to_stdout = {
        read.for_each(|message| async {
            let message = message.unwrap();
            let data = message.into_text().unwrap();

            if data.trim() == "PING :tmi.twitch.tv" {
                stdin_tx.unbounded_send(Message::text("PONG :tmi.twitch.tv")).unwrap();  
            }
            tokio::io::stdout().write_all(data.as_bytes()).await.unwrap();
        })
    };

    pin_mut!(stdin_to_ws, ws_to_stdout);

    stdin_tx.unbounded_send(Message::text("PASS oauth:qijc787vqh6xprmbbluoaievc39yla")).unwrap();
    stdin_tx.unbounded_send(Message::text("NICK iMarluxia")).unwrap();
    stdin_tx.unbounded_send(Message::text("JOIN #ludwig")).unwrap();
    future::select(stdin_to_ws, ws_to_stdout).await;
}

// Our helper method which will read data from stdin and send it along the
// sender provided.
async fn read_stdin(tx: futures_channel::mpsc::UnboundedSender<Message>) {
    loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        tx.unbounded_send(Message::text(input)).unwrap();
    }
}

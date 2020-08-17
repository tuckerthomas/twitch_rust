use serde::Deserialize;
use std::{collections::HashMap, env, sync::Arc};
use tokio::{sync::Mutex, task};
use warp::{http::StatusCode, Filter, Rejection, Reply};

use futures_util::{future, pin_mut, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

use twitch_rust::message::parse_message;

// Chat IRC implementation
// https://crates.io/crates/tokio-tungstenite

#[derive(Debug, Deserialize)]
pub struct AuthCodeResponse {
    pub code: String,
    pub scope: String,
    pub state: Option<String>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct AccessToken {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i32,
    pub scope: Option<Vec<String>>,
    pub token_type: String,
}

#[derive(Default, Debug, Clone)]
pub struct State {
    pub access_token: AccessToken,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let state = Arc::new(Mutex::new(State::default()));

    /*
    let mut authorize_params = HashMap::new();
    authorize_params.insert("client_id", "p083ksyz79hrrsbg56j1v7cwhirhr4");
    //map.insert("client_secret", "z3t35yrqcmw4kexqem5yzrxzu357qq");
    authorize_params.insert("redirect_uri", "http://home.tuckerthomas.com/");
    //map.insert("grant_type", "client_credentials");
    authorize_params.insert("response_type", "code");
    authorize_params.insert("scopes", "");

    let resp2 = client.get("https://id.twitch.tv/oauth2/authorize")
        .json(&authorize_params)
        .send()
        .await?; */

    // https://id.twitch.tv/oauth2/authorize?client_id=p083ksyz79hrrsbg56j1v7cwhirhr4&redirect_uri=http://localhost:8080/test&response_type=code

    env::set_var("RUST_LOG", "test=debug");

    env_logger::init();

    let log = warp::log("test");
    let new_state = state.clone();
    let hello = warp::path!("test")
        .and(warp::get())
        .and(warp::query::<AuthCodeResponse>())
        .and(warp::any().map(move || Arc::clone(&new_state)))
        .and_then(handler)
        .with(log);

    let warp_thread = tokio::spawn(async move {
        warp::serve(hello).run(([0, 0, 0, 0], 8080)).await;
    });

    // Log into twitch oauth
    open::that("https://id.twitch.tv/oauth2/authorize?client_id=p083ksyz79hrrsbg56j1v7cwhirhr4&scope=chat:read&redirect_uri=http://localhost:8080/test&response_type=code").unwrap();

    let new_state = Arc::clone(&state);
    let init_thread = tokio::spawn(async move {
        loop {
            let new_state = new_state.lock().await;
            if new_state.access_token.access_token == "" {
                println!("Not logged in!");
                std::thread::sleep(std::time::Duration::from_millis(100));
            } else {
                println!(
                    "Logged in, access token: {}",
                    new_state.access_token.access_token
                );
                break;
            }
        }
    });

    let ws_thread = tokio::spawn(async move {
        std::thread::sleep(std::time::Duration::from_millis(1000));
        let url = url::Url::parse("ws://irc-ws.chat.twitch.tv:80").unwrap();

        let (stdin_tx, stdin_rx) = futures_channel::mpsc::unbounded();
        tokio::spawn(read_stdin(stdin_tx));

        let (ws_stream, _) = connect_async(url).await.expect("Failed to connect");
        println!("WebSocket handshake has been successfully completed");

        let (write, read) = ws_stream.split();

        let stdin_to_ws = stdin_rx.map(Ok).forward(write);
        let ws_to_stdout = {
            read.for_each(|message| async {
                let data = message.unwrap().into_data();
                tokio::io::stdout().write_all(&data).await.unwrap();
            })
        };

        pin_mut!(stdin_to_ws, ws_to_stdout);
        future::select(stdin_to_ws, ws_to_stdout).await;
    });

    init_thread.await?;

    let connect_addr = "wss://irc-ws.chat.twitch.tv:443";

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
            println!("{:#?}", parse_message(&data));
            tokio::io::stdout().write_all(data.as_bytes()).await.unwrap();
        })
    };

    pin_mut!(stdin_to_ws, ws_to_stdout);

    let oauth = format!("PASS oauth:{}", state.lock().await.access_token.access_token);
    stdin_tx.unbounded_send(Message::text(oauth)).unwrap();
    stdin_tx.unbounded_send(Message::text("NICK iMarluxia")).unwrap();
    stdin_tx.unbounded_send(Message::text("JOIN #moonmoon")).unwrap();
    future::select(stdin_to_ws, ws_to_stdout).await;

    warp_thread.await?;
    ws_thread.await?;

    Ok(())
}

async fn handler(
    auth_code: AuthCodeResponse,
    state: Arc<Mutex<State>>,
) -> Result<impl Reply, Rejection> {
    get_access_token(auth_code, state).await.unwrap();
    Ok(StatusCode::OK)
}

async fn get_access_token(
    query: AuthCodeResponse,
    state: Arc<Mutex<State>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    let mut auth_code_params = HashMap::new();

    auth_code_params.insert("client_id", "p083ksyz79hrrsbg56j1v7cwhirhr4");
    auth_code_params.insert("client_secret", "z3t35yrqcmw4kexqem5yzrxzu357qq");
    auth_code_params.insert("code", &query.code);
    auth_code_params.insert("grant_type", "authorization_code");
    auth_code_params.insert("redirect_uri", "http://localhost:8080/test");

    let access_token_text = client
        .post("https://id.twitch.tv/oauth2/token")
        .query(&auth_code_params)
        .send()
        .await?
        .text()
        .await?;

    let access_token: Result<AccessToken, serde_json::Error> =
        serde_json::from_str(&access_token_text.clone());

    match access_token {
        Ok(new_token) => {
            println!("Access Token Acquired!");
            let mut state = state.lock().await;
            state.access_token = new_token;
            return Ok(());
        }
        Err(e) => {
            println!("Access Token Text: {:#?}", access_token_text);
            return Err(Box::new(e));
        }
    }
}

async fn read_stdin(tx: futures_channel::mpsc::UnboundedSender<Message>) {
    loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        tx.unbounded_send(Message::text(input)).unwrap();
    }
}
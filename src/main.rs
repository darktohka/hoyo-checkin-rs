use reqwest::{
    blocking::Client,
    header::{HeaderMap, HeaderValue},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs};

pub struct Game<'a> {
    name: &'a str,
    act_id: &'a str,
    url_get_status: &'a str,
    url_sign: &'a str,
    rpc_sign_game: Option<&'a str>,
}

const GAMES: &[Game] = &[
    Game {
        name: "Genshin Impact",
        act_id: "e202102251931481",
        url_get_status: "https://sg-hk4e-api.hoyolab.com/event/sol/info",
        url_sign: "https://sg-hk4e-api.hoyolab.com/event/sol/sign",
        rpc_sign_game: None,
    },
    Game {
        name: "Honkai Star Rail",
        act_id: "e202303301540311",
        url_get_status: "https://sg-public-api.hoyolab.com/event/luna/os/info",
        url_sign: "https://sg-public-api.hoyolab.com/event/luna/os/sign",
        rpc_sign_game: None,
    },
    Game {
        name: "Zenless Zone Zero",
        act_id: "e202406031448091",
        url_get_status: "https://sg-public-api.hoyolab.com/event/luna/zzz/os/info",
        url_sign: "https://sg-public-api.hoyolab.com/event/luna/zzz/os/sign",
        rpc_sign_game: Some("zzz"),
    },
];

#[derive(Deserialize)]
pub struct Config {
    accounts: Vec<Account>,
    healthcheck: Option<String>,
}

#[derive(Deserialize)]
pub struct Account {
    name: String,
    cookies: HashMap<String, String>,
}

#[derive(Serialize)]
pub struct SignRequest {
    act_id: String,
}

#[derive(Deserialize)]
pub struct SignData {
    is_sign: Option<bool>,
}

#[derive(Deserialize)]
pub struct SignResponse {
    retcode: Option<i32>,
    message: Option<String>,
    data: Option<SignData>,
}

struct HoyolabCheckin<'a> {
    account: &'a Account,
    client: &'a Client,
    games: &'a [Game<'a>],
}

impl<'a> HoyolabCheckin<'a> {
    fn new(account: &'a Account, client: &'a Client, games: &'a [Game]) -> Self {
        Self {
            account,
            client,
            games,
        }
    }

    fn get_status(&self, game: &Game) -> Result<bool, String> {
        let request = self
            .client
            .get(game.url_get_status)
            .query(&[("lang", "en-us"), ("act_id", &game.act_id)])
            .headers(self.build_headers(game));
        let response: SignResponse = request
            .send()
            .map_err(|e| e.to_string())?
            .json()
            .map_err(|e| e.to_string())?;

        let return_code = response.retcode.unwrap_or(0);

        if return_code != 0 {
            return Err(response
                .message
                .unwrap_or_else(|| format!("Return code is {}", return_code).to_string()));
        }

        Ok(response
            .data
            .map_or(false, |data| data.is_sign.unwrap_or(false)))
    }

    fn sign(&self, game: &Game) -> Result<(), String> {
        let data = serde_json::to_string(&SignRequest {
            act_id: game.act_id.to_string(),
        })
        .map_err(|e| e.to_string())?;

        let request = self
            .client
            .post(game.url_sign)
            .query(&[("lang", "en-us")])
            .headers(self.build_headers(game))
            .body(data);
        let response: SignResponse = request
            .send()
            .map_err(|e| e.to_string())?
            .json()
            .map_err(|e| e.to_string())?;

        let return_code = response.retcode.unwrap_or(0);

        if return_code == -5003 {
            // Traveler, you've already checked in today~
            return Ok(());
        }

        if return_code != 0 {
            return Err(response
                .message
                .unwrap_or_else(|| format!("Return code is {}", return_code).to_string()));
        }

        Ok(())
    }

    fn process_game(&self, game: &Game) -> bool {
        match self.get_status(game) {
            Ok(false) => {
                if let Err(e) = self.sign(game) {
                    println!(
                        "Failed to sign in for {} on {}: {}",
                        self.account.name, game.name, e
                    );
                    return false;
                }

                if let Ok(true) = self.get_status(game) {
                    println!(
                        "Daily check-in successful for {} on {}!",
                        self.account.name, game.name
                    );
                    return true;
                }

                println!(
                    "ERROR: Unable to claim check-in rewards for {} on {}",
                    self.account.name, game.name
                );
            }
            Ok(true) => println!(
                "Daily check-in already done for {} on {}!",
                self.account.name, game.name
            ),
            Err(e) => println!(
                "Failed check-in for {} on {}: {}",
                self.account.name, game.name, e
            ),
        }

        false
    }

    fn process(&self) -> bool {
        let mut success = true;

        for game in self.games {
            if !self.process_game(game) {
                success = false;
            }
        }

        success
    }

    fn build_headers(&self, game: &Game) -> HeaderMap {
        let mut headers = HeaderMap::new();

        headers.insert(
            "Accept",
            HeaderValue::from_static("application/json, text/plain, */*"),
        );
        headers.insert(
            "Accept-Language",
            HeaderValue::from_static("en-US,en;q=0.5"),
        );
        headers.insert(
            "Origin",
            HeaderValue::from_static("https://act.hoyolab.com"),
        );
        headers.insert(
            "Referer",
            HeaderValue::from_static("https://act.hoyolab.com"),
        );
        headers.insert(
            "Content-Type",
            HeaderValue::from_static("application/json;charset=utf-8"),
        );
        headers.insert("User-Agent", HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/116.0.0.0 Safari/537.36"));
        headers.insert("x-rpc-app_version", HeaderValue::from_static("2.34.1"));
        headers.insert("x-rpc-client_type", HeaderValue::from_static("4"));

        if let Some(rpc_sign_game) = &game.rpc_sign_game {
            headers.insert(
                "x-rpc-signgame",
                HeaderValue::from_str(rpc_sign_game)
                    .expect("Failed to build x-rpc-signgame header"),
            );
        }

        headers.insert(
            "Cookie",
            HeaderValue::from_str(
                &self
                    .account
                    .cookies
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("; "),
            )
            .expect("Failed to build cookie header"),
        );

        headers
    }
}

fn main() {
    let data = fs::read_to_string("config.json").expect("Failed to read config.json");
    let config: Config = serde_json::from_str(&data).expect("Invalid JSON");

    let mut success = true;
    let client = Client::new();

    for account in config.accounts {
        let checkin = HoyolabCheckin::new(&account, &client, GAMES);

        if !checkin.process() {
            success = false;
        }
    }

    if let Some(healthcheck) = config.healthcheck {
        let url = if !success {
            format!("{}/fail", healthcheck)
        } else {
            healthcheck.to_string()
        };

        let _ = client.get(&url).send();
    }
}

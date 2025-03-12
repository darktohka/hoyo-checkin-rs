use reqwest::{
    blocking::Client,
    header::{HeaderMap, HeaderValue},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs};

const GAME_NAMES: &[(&str, &str)] = &[
    ("genshin", "Genshin Impact"),
    ("starrail", "Honkai Star Rail"),
    ("zenless", "Zenless Zone Zero"),
];

const ACT_ID: &[(&str, &str)] = &[
    ("genshin", "e202102251931481"),
    ("starrail", "e202303301540311"),
    ("zenless", "e202406031448091"),
];

const URL_GET_STATUS: &[(&str, &str)] = &[
    ("genshin", "https://sg-hk4e-api.hoyolab.com/event/sol/info"),
    (
        "starrail",
        "https://sg-public-api.hoyolab.com/event/luna/os/info",
    ),
    (
        "zenless",
        "https://sg-public-api.hoyolab.com/event/luna/zzz/os/info",
    ),
];

const URL_SIGN: &[(&str, &str)] = &[
    ("genshin", "https://sg-hk4e-api.hoyolab.com/event/sol/sign"),
    (
        "starrail",
        "https://sg-public-api.hoyolab.com/event/luna/os/sign",
    ),
    (
        "zenless",
        "https://sg-public-api.hoyolab.com/event/luna/zzz/os/sign",
    ),
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
}

impl<'a> HoyolabCheckin<'a> {
    fn new(account: &'a Account, client: &'a Client) -> Self {
        Self { account, client }
    }

    fn get_status(&self, game: &str) -> Result<bool, String> {
        let act_id = ACT_ID.iter().find(|&&(g, _)| g == game).unwrap().1;
        let url = URL_GET_STATUS.iter().find(|&&(g, _)| g == game).unwrap().1;

        let request = self
            .client
            .get(url)
            .query(&[("lang", "en-us"), ("act_id", act_id)])
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

    fn sign(&self, game: &str) -> Result<(), String> {
        let act_id = ACT_ID.iter().find(|&&(g, _)| g == game).unwrap().1;
        let url = URL_SIGN.iter().find(|&&(g, _)| g == game).unwrap().1;

        let data = serde_json::to_string(&SignRequest {
            act_id: act_id.to_string(),
        })
        .map_err(|e| e.to_string())?;

        let request = self
            .client
            .post(url)
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

    fn process_game(&self, game: &str) -> bool {
        let name = GAME_NAMES.iter().find(|&&(g, _)| g == game).unwrap().1;

        match self.get_status(game) {
            Ok(false) => {
                if let Err(e) = self.sign(game) {
                    println!(
                        "Failed to sign in for {} on {}: {}",
                        self.account.name, name, e
                    );
                    return false;
                }

                if let Ok(true) = self.get_status(game) {
                    println!(
                        "Daily check-in successful for {} on {}!",
                        self.account.name, name
                    );
                    return true;
                }

                println!(
                    "ERROR: Unable to claim check-in rewards for {} on {}",
                    self.account.name, name
                );
            }
            Ok(true) => println!(
                "Daily check-in already done for {} on {}!",
                self.account.name, name
            ),
            Err(e) => println!(
                "Failed check-in for {} on {}: {}",
                self.account.name, name, e
            ),
        }
        false
    }

    fn process(&self) -> bool {
        GAME_NAMES.iter().all(|&(game, _)| self.process_game(game))
    }

    fn build_headers(&self, game: &str) -> HeaderMap {
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

        if game == "zenless" {
            headers.insert("x-rpc-signgame", HeaderValue::from_static("zzz"));
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
        let checkin = HoyolabCheckin::new(&account, &client);

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

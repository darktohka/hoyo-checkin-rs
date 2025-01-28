use reqwest::{blocking::Client, header::HeaderMap};
use serde_json::Value;
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
        "https://sg-act-nap-api.hoyolab.com/event/luna/zzz/os/info",
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
        "https://sg-act-nap-api.hoyolab.com/event/luna/zzz/os/sign",
    ),
];

struct HoyolabCheckin {
    name: String,
    cookies: HashMap<String, String>,
    client: Client,
}

impl HoyolabCheckin {
    fn new(name: &str, cookies: HashMap<String, String>) -> Self {
        Self {
            name: name.to_string(),
            cookies,
            client: Client::new(),
        }
    }

    fn get_status(&self, game: &str) -> Result<bool, String> {
        let act_id = ACT_ID.iter().find(|&&(g, _)| g == game).unwrap().1;
        let url = URL_GET_STATUS.iter().find(|&&(g, _)| g == game).unwrap().1;

        let request = self
            .client
            .get(url)
            .query(&[("lang", "en-us"), ("act_id", act_id)])
            .headers(self.build_headers(game))
            .header(
                "Cookie",
                self.cookies
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("; "),
            );
        let response: Value = request
            .send()
            .map_err(|e| e.to_string())?
            .json()
            .map_err(|e| e.to_string())?;

        if let Some(message) = response.get("message") {
            if response.get("retcode").unwrap_or(&Value::from(0)) != &Value::from(0) {
                return Err(message.to_string());
            }
        }

        Ok(response["data"]["is_sign"].as_bool().unwrap_or(false))
    }

    fn sign(&self, game: &str) -> Result<(), String> {
        let act_id = ACT_ID.iter().find(|&&(g, _)| g == game).unwrap().1;
        let url = URL_SIGN.iter().find(|&&(g, _)| g == game).unwrap().1;

        let data = serde_json::json!({ "act_id": act_id });

        let request = self
            .client
            .post(url)
            .query(&[("lang", "en-us")])
            .headers(self.build_headers(game))
            .json(&data);
        let response: Value = request
            .send()
            .map_err(|e| e.to_string())?
            .json()
            .map_err(|e| e.to_string())?;

        if let Some(message) = response.get("message") {
            if response.get("retcode").unwrap_or(&Value::from(0)) != &Value::from(0) {
                return Err(message.to_string());
            }
        }

        Ok(())
    }

    fn process_game(&self, game: &str) -> bool {
        let name = GAME_NAMES.iter().find(|&&(g, _)| g == game).unwrap().1;

        match self.get_status(game) {
            Ok(false) => {
                if let Err(e) = self.sign(game) {
                    println!("Failed to sign in for {} on {}: {}", self.name, name, e);
                    return false;
                }

                if let Ok(true) = self.get_status(game) {
                    println!("Daily check-in successful for {} on {}!", self.name, name);
                    return true;
                }

                println!(
                    "ERROR: Unable to claim check-in rewards for {} on {}",
                    self.name, name
                );
            }
            Ok(true) => println!("Daily check-in already done for {} on {}!", self.name, name),
            Err(e) => println!("Failed check-in for {} on {}: {}", self.name, name, e),
        }
        false
    }

    fn process(&self) -> bool {
        GAME_NAMES.iter().all(|&(game, _)| self.process_game(game))
    }

    fn build_headers(&self, game: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        let act_id = ACT_ID.iter().find(|&&(g, _)| g == game).unwrap().1;

        headers.insert(
            "Accept",
            "application/json, text/plain, */*".parse().unwrap(),
        );
        headers.insert("Accept-Language", "en-US,en;q=0.5".parse().unwrap());
        headers.insert(
            "Origin",
            "https://webstatic-sea.mihoyo.com".parse().unwrap(),
        );
        headers.insert(
            "Referer",
            format!("https://webstatic-sea.mihoyo.com/ys/event/signin-sea/index.html?act_id={}&lang=en-us", act_id).parse().unwrap(),
        );
        headers.insert(
            "Content-Type",
            "application/json;charset=utf-8".parse().unwrap(),
        );
        headers.insert("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/116.0.0.0 Safari/537.36".parse().unwrap());
        headers
    }
}

fn main() {
    let config = fs::read_to_string("config.json").expect("Failed to read config.json");
    let data: Value = serde_json::from_str(&config).expect("Invalid JSON");

    let mut success = true;

    if let Some(accounts) = data["accounts"].as_array() {
        for account in accounts {
            let name = account["name"].as_str().unwrap_or("").to_string();
            let cookies: HashMap<String, String> =
                serde_json::from_value(account["cookies"].clone()).unwrap();
            let checkin = HoyolabCheckin::new(&name, cookies);
            if !checkin.process() {
                success = false;
            }
        }
    }

    if let Some(healthcheck) = data["healthcheck"].as_str() {
        let url = if !success {
            format!("{}/fail", healthcheck)
        } else {
            healthcheck.to_string()
        };
        let _ = Client::new().get(&url).send();
    }
}

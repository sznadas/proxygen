#![feature(plugin)]
#![plugin(clippy)]

#![feature(custom_derive, plugin)]
#![plugin(serde_macros)]

#![feature(plugin)]
#![plugin(maud_macros)]

extern crate serde;
extern crate serde_json;

extern crate maud;
use maud::PreEscaped;

#[macro_use]
extern crate nickel;
use nickel::{Nickel, FormBody, MediaType};
use nickel::status::StatusCode;

extern crate regex;
use regex::Regex;

#[macro_use]
extern crate lazy_static;

mod card;
use card::Card;
mod error;
use error::ProxygenError;


const PROXYGEN_HTML: &'static str = include_str!("proxygen.html");
const PROXYGEN_CSS: &'static str = include_str!("proxygen.css");
const RESULTS_CSS: &'static str = include_str!("results.css");
const MAX_CARDS: u64 = 1000;

lazy_static!{
    static ref RE: Regex = Regex::new(r"^\s*(\d+)?x?\s*(\D*?)\s*$").unwrap();
}

pub fn sanitize_name(name: &str) -> String {
    // These should cover all non-unhinged/unglued cases.
    // People who want unhinged/unglued stuff can make sure they're precise
    name.to_lowercase()
        .replace("\u{e6}", "ae")
        .replace("\u{e0}", "a")
        .replace("\u{e1}", "a")
        .replace("\u{e2}", "a")
        .replace("\u{e9}", "e")
        .replace("\u{ed}", "i")
        .replace("\u{f6}", "o")
        .replace("\u{fa}", "u")
        .replace("\u{fb}", "u")
}

fn parse_decklist(decklist: &str) -> Result<Vec<(u64, Card)>, ProxygenError> {
    let mut count = 0;
    let mut out = Vec::new();
    for entry in decklist.lines() {
        let trimmed = entry.trim();
        if !entry.is_empty() {
            let (n, c) = match RE.captures(trimmed) {
                Some(captures) => {
                    let amount: u64 = match captures.at(1) {
                        Some(v) => v.parse().unwrap(),
                        None => 1,
                    };

                    count += amount;
                    if count > MAX_CARDS {
                        return Err(ProxygenError::TooManyCards);
                    }

                    let card_name = captures.at(2).unwrap();

                    let card = match card::Card::from_name(card_name) {
                        Ok(v) => v,
                        Err(e) => {
                            return Err(e);
                        }
                    };

                    (amount, card)
                }
                None => return Err(ProxygenError::DecklistParseError(String::from(trimmed))),
            };
            out.push((n, c));
        };
    }
    Ok(out)
}

fn main() {
    println!("Building database..");
    Card::from_name("Island").unwrap_or_else(|e| panic!("Error building database: {:?}", e));

    let mut server = Nickel::new();

    server.utilize(router!{
        post "/proxygen" => |req, mut res| {
            let form_body = try_with!(res, req.form_body());
            println!("{:?}", form_body);
            let decklist = String::from(form_body.get("decklist").unwrap());

            let parsed = match parse_decklist(&decklist) {
                Ok(v) => v,
                Err(ProxygenError::TooManyCards) => {
                    *res.status_mut() = StatusCode::BadRequest;
                    return res.send(format!("Too many proxies requested. Request at most {} proxies at a time", MAX_CARDS))
                }
                Err(ProxygenError::InvalidCardName(s)) => {
                    *res.status_mut() = StatusCode::BadRequest;
                    return res.send(format!("Invalid card name: {:?}", s));
                },
                Err(ProxygenError::DecklistParseError(s)) => {
                    *res.status_mut() = StatusCode::BadRequest;
                    return res.send(format!("Error parsing decklist at line: {:?}", s));
                },
                Err(ProxygenError::MulticardHasMalformedNames(s)) => {
                    *res.status_mut() = StatusCode::InternalServerError;
                    return res.send(format!("A split/flip/transform has more than 2 different forms. Are you using unhinged/unglued cards? Card: {:?}", s))
                }
                Err(e) => {
                    *res.status_mut() = StatusCode::InternalServerError;
                    return res.send(format!("An error happened interally that wasn't handled properly. Tell the developer '{:?}'", e));
                }
            };

            let mut div_chain = String::new();

            for pair in parsed {
                let (n, card) = pair;
                for _ in 0..n {
                    div_chain.push_str(&card.to_html());
                }
            }

            let mut doc = String::new();
            html!(doc, html {
                head {
                    meta charset="UTF-8"
                    style {
                        ^PreEscaped(RESULTS_CSS)
                    }
                }
                body {
                    ^PreEscaped(div_chain)
                }
            }).unwrap();
            return res.send(doc)
        }
    });

    // Static files
    server.utilize(router! {
        get "/proxygen.css" => |_req, mut res| {
            res.set(MediaType::Css);

            return res.send(PROXYGEN_CSS)
        }
        get "/proxygen" => |_req, res| {
            return res.send(PROXYGEN_HTML)
        }
    });

    server.listen("127.0.0.1:6767");
}

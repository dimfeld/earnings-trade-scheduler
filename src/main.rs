

extern crate chrono;
extern crate crossbeam;
extern crate csv;
#[macro_use] extern crate failure;
extern crate itertools;
extern crate json;
#[macro_use] extern crate lazy_static;
extern crate reqwest;
extern crate regex;
extern crate scraper;
extern crate serde;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate slog;
extern crate sloggers;

mod earnings;

use std::env;
use sloggers::Build;
use sloggers::terminal::TerminalLoggerBuilder;
use std::collections::HashMap;
use failure::{Error, ResultExt};
use chrono::{NaiveDate, Datelike, Weekday, Duration};
use reqwest::header::{Headers, UserAgent};

use earnings::{EarningsDateTime, Date};

#[derive(Deserialize)]
struct BacktestResult {
    symbol : String,
    wins : usize,
    losses: usize,
    win_rate : String,
    avg_trade_return : String,
    total_return : String,
    backtest_len : usize,
    next_earnings : String,
    strategy : String,
}

impl BacktestResult {
    fn earnings_date(&self) -> Result<EarningsDateTime, Error> {
        earnings::Date::parse_from_str(self.next_earnings.as_str(), "%m/%d%y")
            .map(|d| earnings::EarningsDateTime{date: d, time: earnings::AnnounceTime::Unknown})
            .map_err(|e| e.into())
    }
}

fn init_logger() -> slog::Logger {
    TerminalLoggerBuilder::new()
        .level(sloggers::types::Severity::Debug)
        .destination(sloggers::terminal::Destination::Stderr)
        .timezone(sloggers::types::TimeZone::Local)
        .build()
        .expect("building logger")
}



fn main() {
    let mut logger = init_logger();

    // let filename = std::env::args().nth(1).expect("filename");
    let symbol = std::env::args().nth(1).expect("symbol");


    // let mut reader = csv::ReaderBuilder::new()
    //     .has_headers(true)
    //     .from_path(filename)
    //     .expect("opening csv");


    // let backtests = reader.deserialize::<BacktestResult>()
    //     .into_iter()
    //     .fold(HashMap::<String, Vec<BacktestResult>>::new(), |mut acc, test| {
    //         let test = test.expect("csv row");
    //         acc
    //             .entry(test.symbol.clone())
    //             .or_insert_with(Vec::new)
    //             .push(test);
    //         acc
    //     });


    // For each symbol, get the earnings date info
    // Given the best guess at earnings, spit out the possible open/close dates for each scenario, sorted by the average trade return.

    let mut headers = Headers::new();
    headers.set(UserAgent::new("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_13_2) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/63.0.3239.132 Safari/537.36"));

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .expect("building client");

    let earnings_dates = earnings::get_earnings_date_estimates(&logger, &client, symbol.as_str());

    for x in &earnings_dates {
        println!("{} - {}", x.source, x.datetime);
    }

    let guess = earnings::best_earnings_guess(&earnings_dates);
    println!("Best guess: {:?}", guess);
}

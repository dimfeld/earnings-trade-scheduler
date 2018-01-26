
#[macro_use] extern crate slog;
extern crate sloggers;
extern crate csv;
extern crate reqwest;
extern crate crossbeam;
extern crate serde;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate failure;
extern crate chrono;

use std::env;
use sloggers::Build;
use sloggers::terminal::TerminalLoggerBuilder;
use std::collections::HashMap;
use failure::{Error, ResultExt};

use chrono::{NaiveDate, Datelike, Weekday, Duration};

type Date = NaiveDate;

enum AnnounceTime {
    BeforeMarket,
    AfterMarket,
    Unknown,
}

struct EarningsDate {
    date : Date,
    time : AnnounceTime,
    source : &'static str,
}

impl EarningsDate {
    /// Return the date of the last trading before the earnings date, along with an estimated error range.
    pub fn last_session(&self) -> (Date, usize) {
        match self.time {
            AnnounceTime::BeforeMarket => (self.date.pred(), 0),
            AnnounceTime::AfterMarket => (self.date, 0),
            AnnounceTime::Unknown => (self.date, 1),
        }
    }
}

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

fn init_logger() -> slog::Logger {
    TerminalLoggerBuilder::new()
        .level(sloggers::types::Severity::Debug)
        .destination(sloggers::terminal::Destination::Stderr)
        .timezone(sloggers::types::TimeZone::Local)
        .build()
        .expect("building logger")
}

struct EarningsGuess {
    last_session : String,
    concurrences : Vec<EarningsDate>,
    close_disagreements : Vec<EarningsDate>,
    far_disagreements : Vec<EarningsDate>,
}

/// If the date falls on a weekend, step back to the closest weekday.
trait DatelikeExt {
    fn closest_weekday(&self) -> Self;
}

impl DatelikeExt for Date {
    fn closest_weekday(&self) -> Date {
        match self.weekday() {
            Weekday::Sat => *self - Duration::days(1),
            Weekday::Sun => *self - Duration::days(2),
            _ => *self
        }
    }
}

fn best_earnings_guess(dates : &[EarningsDate]) -> EarningsGuess {

    // Group the EarningsDates by the last trading session.


    EarningsGuess {
        last_session: String::new(),
        concurrences: Vec::new(),
        close_disagreements: Vec::new(),
        far_disagreements: Vec::new(),
    }
}



struct EarningsSource {
    name : &'static str,
    url: &'static str,
    extract: (fn(reqwest::Response) -> Option<EarningsDate>),
}

fn extract_bloomberg(response : reqwest::Response) -> Option<EarningsDate> {
    None
}

fn extract_nasdaq(response : reqwest::Response) -> Option<EarningsDate> {
    None
}

fn extract_finviz(response : reqwest::Response) -> Option<EarningsDate> {
    None
}

fn extract_yahoo(response : reqwest::Response) -> Option<EarningsDate> {
    None
}

fn extract_zacks(response : reqwest::Response) -> Option<EarningsDate> {
    None
}

fn get_earnings_date_estimates(logger : &slog::Logger, sources : &[&EarningsSource], symbol : &str) -> Vec<EarningsDate> {
    crossbeam::scope(|scope| {
        let joins = sources.iter()
            .map(|source| {
                scope.spawn(move || {
                    let url = source.url.replace("{}", symbol);
                    let response = reqwest::get(url.as_str()).with_context(|_| format!("Symbol {}, source {}", symbol, source.name))?;
                    let date = (source.extract)(response);
                    let x : Result<_, Error> = Ok(date);
                    x
                })
            })
            .collect::<Vec<_>>();

        joins.into_iter()
            .filter_map(|j| {
                match j.join() {
                    Err(e) => {
                        error!(logger, "{}", e);
                        None
                    },
                    Ok(date) => date
                }
            })
            .collect::<Vec<_>>()
    })
}

fn main() {
    let mut logger = init_logger();

    let filename = std::env::args().nth(1).expect("filename");


    let sources = vec![
        EarningsSource{
            name: "Bloomberg",
            url: "https://www.bloomberg.com/quote/{}:US",
            extract: extract_bloomberg,
        },
        EarningsSource{
            name: "NASDAQ",
            url: "http://www.nasdaq.com/earnings/report/{}",
            extract: extract_nasdaq,
        },
        EarningsSource{
            name: "FinViz",
            url: "https://finviz.com/quote.ashx?t={}",
            extract: extract_finviz,
        },
        EarningsSource{
            name: "Yahoo",
            url: "https://finance.yahoo.com/quote/{}",
            extract: extract_yahoo,
        },
        EarningsSource{
            name: "Zacks",
            url: "https://www.zacks.com/stock/quote/{}",
            extract: extract_zacks,
        },
    ];

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(filename)
        .expect("opening csv");


    let backtests = reader.deserialize::<BacktestResult>()
        .into_iter()
        .fold(HashMap::<String, Vec<BacktestResult>>::new(), |mut acc, test| {
            let test = test.expect("csv row");
            acc
                .entry(test.symbol.clone())
                .or_insert_with(Vec::new)
                .push(test);
            acc
        });


    // For each symbol, get the earnings date info
    // Given the best guess at earnings, spit out the possible open/close dates for each scenario, sorted by the average trade return.
}

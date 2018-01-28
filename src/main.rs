

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

mod cmlviz;
mod earnings;

use std::env;
use sloggers::Build;
use sloggers::terminal::TerminalLoggerBuilder;
use std::collections::HashMap;
use failure::{Error, ResultExt};
use chrono::{NaiveDate, Datelike, Weekday, Duration};
use reqwest::header::{Headers, UserAgent};

use earnings::{EarningsDateTime, Date};



fn init_logger() -> slog::Logger {
    TerminalLoggerBuilder::new()
        .level(sloggers::types::Severity::Debug)
        .destination(sloggers::terminal::Destination::Stderr)
        .timezone(sloggers::types::TimeZone::Local)
        .build()
        .expect("building logger")
}

struct TestsAndEarnings {
    symbol : String,
    best_test: cmlviz::BacktestResult,
    tests : Vec<cmlviz::BacktestResult>,
    earnings : earnings::EarningsGuess,
}

fn main() {
    let mut logger = init_logger();

    let filename = std::env::args().nth(1).expect("filename");
    // let symbol = std::env::args().nth(1).expect("symbol");


    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(filename)
        .expect("opening csv");


    let backtests_by_symbol = reader.deserialize::<cmlviz::BacktestResultInput>()
        .into_iter()
        .map(|t| cmlviz::BacktestResult::from_input(t?))
        .fold(HashMap::<String, Vec<cmlviz::BacktestResult>>::new(), |mut acc, test| {
            let test = test.expect("csv row");
            println!("{:?}", test);
            acc
                .entry(test.symbol.clone())
                .or_insert_with(Vec::new)
                .push(test);
            acc
        });


    // For each symbol, get the earnings date info
    // Given the best guess at earnings, spit out the possible open/close dates for each scenario, sorted by the average trade return.

    let mut headers = Headers::new();
    headers.set(UserAgent::new("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_13_2) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/63.0.3239.132 Safari/537.36"));

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .expect("building client");

    let tests_with_earnings = backtests_by_symbol
        .into_iter()
        .map(|(symbol, tests)| {
            let mut earnings_dates = earnings::get_earnings_date_estimates(&logger, &client, symbol.as_str());
            let test_date = earnings::SourcedEarningsTime{
                source: "CML",
                datetime: tests[0].next_earnings,
            };
            earnings_dates.push(test_date);

            let guess = earnings::best_earnings_guess(&earnings_dates);
            let best_test = cmlviz::get_best_test(&tests);

            TestsAndEarnings{
                symbol: symbol,
                best_test: best_test,
                tests: tests,
                earnings: guess,
            }
        })
        .collect::<Vec<_>>();

    // for (symbol, backtests) in backtests_by_symbol {

    //     for x in &earnings_dates {
    //         println!("{} - {}", x.source, x.datetime);
    //     }


    //     println!("Best guess: {:?}", guess);
    // }


}

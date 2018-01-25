
#[macro_use] extern crate slog;
extern crate sloggers;
extern crate csv;
extern crate reqwest;
extern crate crossbeam;
extern crate serde;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate failure;

use std::env;
use sloggers::Build;
use sloggers::terminal::TerminalLoggerBuilder;
use std::collections::HashMap;

enum AnnounceTime {
    BeforeMarket,
    AfterMarket,
    Unknown,
}

struct EarningsDate {
    date : String,
    time : AnnounceTime,
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

fn best_earnings_guess(dates : &[EarningsDate]) -> String {
    String::new() // TODO implement this
}


fn get_earnings_date_estimates(symbol : &str) -> Vec<EarningsDate> {
    crossbeam::scope(|scope| {
        let nasdaq = scope.spawn(|| {
            None
        });

        let finviz = scope.spawn(|| {
            None
        });

        vec![nasdaq.join(), finviz.join()].into_iter().filter_map(|x| x).collect::<Vec<_>>()
    })
}

fn main() {
    let mut logger = init_logger();

    let filename = std::env::args().nth(1).expect("filename");

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

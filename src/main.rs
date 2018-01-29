

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
extern crate clap;
extern crate structopt;
#[macro_use] extern crate structopt_derive;

mod cmlviz;
mod earnings;

use failure::{Error, ResultExt};
use itertools::Itertools;
use std::io::Write;
use std::fs::File;
use sloggers::Build;
use sloggers::terminal::TerminalLoggerBuilder;
use std::collections::{HashMap, BTreeMap};
use reqwest::header::{Headers, UserAgent};
use structopt::StructOpt;

fn init_logger() -> slog::Logger {
    TerminalLoggerBuilder::new()
        .level(sloggers::types::Severity::Debug)
        .destination(sloggers::terminal::Destination::Stderr)
        .timezone(sloggers::types::TimeZone::Local)
        .build()
        .expect("building logger")
}

#[derive(StructOpt)]
#[structopt(name="preearnings_call_scheduler", about="Pre-earnings Call Scheduler")]
struct Config {
    #[structopt(long="start", help="Process symbols with earnings after this date")]
    start_date : Option<earnings::Date>,

    #[structopt(long="end", help="Process symbols with earnings before this date")]
    end_date : Option<earnings::Date>,

    #[structopt(help = "Input file")]
    input : String,

    #[structopt(help="Output file")]
    output : Option<String>,
}

#[derive(Debug,Serialize)]
struct TestsAndEarnings {
    symbol : String,
    tests : Vec<cmlviz::BacktestResult>,
    best_test_index: usize,
    earnings : earnings::EarningsGuess,
}

fn run_it(logger : &slog::Logger) -> Result<(), Error> {
    let cfg = Config::from_args();
    let filename = &cfg.input;

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(&filename)
        .context("opening csv")?;


    // Read the file and group the tests by symbol.
    info!(logger, "Reading file {}", filename);
    let backtests_by_symbol = reader.deserialize::<cmlviz::BacktestResultInput>()
        .into_iter()
        .map(|t| cmlviz::BacktestResult::from_input(t?))
        .map(|t| t.expect("csv row"))
        .filter(|t| cfg.start_date.map_or(true, |x| t.next_earnings.date >= x) && cfg.end_date.map_or(true, |x| t.next_earnings.date <= x))
        .fold(HashMap::<String, Vec<cmlviz::BacktestResult>>::new(), |mut acc, test| {
            // Assume that the CSV data is proper so we don't do real error handling on it.

            acc
                .entry(test.symbol.clone())
                .or_insert_with(Vec::new)
                .push(test);
            acc
        });


    let mut headers = Headers::new();
    headers.set(UserAgent::new("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_13_2) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/63.0.3239.132 Safari/537.36"));

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .context("building client")?;

    let tests_with_earnings = backtests_by_symbol
        .into_iter()
        .map(|(symbol, tests)| {
            info!(logger, "Processing symbol {}", symbol);

            // Figure out our best guess at the earnings date based on the CML data and a bunch of other sources.
            let mut earnings_dates = earnings::get_earnings_date_estimates(&logger, &client, symbol.as_str());
            let test_date = earnings::SourcedEarningsTime{
                source: "CML".into(),
                datetime: tests[0].next_earnings,
            };
            earnings_dates.push(test_date);

            let guess = earnings::best_earnings_guess(&earnings_dates);

            // The "best test" is just the one that has the highest average trade return.
            // In general the win rates for the various strategies are close enough that it's not worth factoring it in
            // beyond the effect that it already has on the average return.
            let best_test = cmlviz::get_best_test(&tests);

            let open_date = tests[best_test].strategy.open_date(guess.last_session);
            let close_date = tests[best_test].strategy.close_date(guess.last_session);
            let key = (open_date, close_date, symbol.clone());
            let result = TestsAndEarnings{
                symbol: symbol,
                tests: tests,
                best_test_index: best_test,
                earnings: guess,
            };
            (key, result)
        })
        .collect::<BTreeMap<_, _>>();

    // TODO Nice output formatting
    let mut output = cfg.output
        .map(|path| {
            let b = Box::new(File::create(path)?);
            let r : Result<Box<Write>, std::io::Error> = Ok(b);
            r
        })
        .unwrap_or_else(|| Ok(Box::new(std::io::stdout())))
        .context("Opening output file")?;

    for ((open_date, close_date, symbol), _) in tests_with_earnings {
        writeln!(output, "{} - {} : {}", open_date, close_date, symbol)?;
    }

    Ok(())
}

fn main() {
    let logger = init_logger();

    if let Err(e) = run_it(&logger) {
        let msg = e.causes()
            .map(|e| e.to_string())
            .join("\n  ");
        error!(logger, "{}", msg);
    }
}


use std::env;
use std::fmt;
use std::fmt::{Display, Formatter};
use crossbeam;
use itertools::Itertools;
use slog;
use reqwest;
use std::collections::HashMap;
use failure::{Error, ResultExt};
use scraper::{Html, Selector};
use chrono;
use chrono::{NaiveDate, Datelike, Weekday, Duration};
use regex::Regex;
use json;

#[derive(Debug, Fail)]
enum EarningsError {
    #[fail(display = "Could not find selector")]
    SelectorNotFound,
}

struct EarningsSource {
    name : &'static str,
    url: &'static str,
    extract: (fn(logger : &slog::Logger, reqwest::Response) -> Result<Option<EarningsDateTime>,Error>),
}

static SOURCES : &[EarningsSource] = &[
        EarningsSource{
            name: "Bloomberg",
            url: "https://www.bloomberg.com/quote/{}:US",
            extract: extract_bloomberg,
        },
        // NASDAQ seeems to have aggressive anti-scraping measures in place, or something.
        // The data is taken from Zack's anyway, so not a big deal.
        // EarningsSource{
        //     name: "NASDAQ",
        //     url: "http://www.nasdaq.com/earnings/report/{}",
        //     extract: extract_nasdaq,
        // },
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


pub type Date = NaiveDate;

#[derive(Debug)]
pub enum AnnounceTime {
    BeforeMarket,
    AfterMarket,
    Unknown,
}

impl Display for AnnounceTime {
    fn fmt(&self, f :&mut Formatter) -> Result<(), fmt::Error> {
        match *self {
            AnnounceTime::BeforeMarket => write!(f, "BMO"),
            AnnounceTime::AfterMarket => write!(f, "AMC"),
            AnnounceTime::Unknown => Ok(())
        }
    }
}

#[derive(Debug)]
pub struct EarningsDateTime{
    pub date: Date,
    pub time: AnnounceTime,
}

impl EarningsDateTime {
    /// Return the date of the last trading before the earnings date, along with an estimated error range.
    pub fn last_session(&self) -> (Date, usize) {
        match self.time {
            AnnounceTime::BeforeMarket => (self.date.pred(), 0),
            AnnounceTime::AfterMarket => (self.date, 0),
            AnnounceTime::Unknown => (self.date, 1),
        }
    }
}

impl Display for EarningsDateTime {
    fn fmt(&self, f : &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "{} {}", self.date, self.time)
    }
}

#[derive(Debug)]
pub struct SourcedEarningsTime {
    pub datetime : EarningsDateTime,
    pub source : &'static str,
}


pub struct EarningsGuess {
    last_session : String,
    concurrences : Vec<SourcedEarningsTime>,
    close_disagreements : Vec<SourcedEarningsTime>,
    far_disagreements : Vec<SourcedEarningsTime>,
}

pub fn best_earnings_guess(dates : &[EarningsDateTime]) -> EarningsGuess {

    // Group the EarningsDates by the last trading session.


    EarningsGuess {
        last_session: String::new(),
        concurrences: Vec::new(),
        close_disagreements: Vec::new(),
        far_disagreements: Vec::new(),
    }
}

fn extract_bloomberg(logger : &slog::Logger, mut response : reqwest::Response) -> Result<Option<EarningsDateTime>, Error> {
    let document = Html::parse_document(response.text()?.as_str());
    let selector = Selector::parse(r#"span[class^="nextAnnouncementDate"]"#).unwrap();
    document.select(&selector)
        .next()
        .and_then(|node| node.text().next())
        .map(|text| Date::parse_from_str(text, "%m/%d/%Y").map(|d| EarningsDateTime{date: d, time: AnnounceTime::Unknown} ))
        .map_or(Ok(None), |v| v.map(Some)) // Switch Option<Result<T, E>> to Result<Option<T>, Error>
        .map_err(|e| e.into())
}



fn extract_nasdaq(logger : &slog::Logger, mut response : reqwest::Response) -> Result<Option<EarningsDateTime>, Error> {

    lazy_static! {
        static ref RE: Regex = Regex::new(r#"earnings on\s*(\d{1,2}/\d{1,2}/\d{4})\s*(after market close|before market open)?."#).unwrap();
    }

    let document = Html::parse_document(response.text()?.as_str());
    let selector = Selector::parse(r#"#two_column_main_content_reportdata"#).unwrap();
    document.select(&selector)
        .next()
        .and_then(|node| node.text().next())
        .and_then(|text| {
           RE.captures_iter(text)
            .next()
            .map(|cap| {
                let date = Date::parse_from_str(&cap[1], "%m/%d/%Y")?;
                let earnings_time = match &cap[2] {
                    "after market close" => AnnounceTime::AfterMarket,
                    "before market open" => AnnounceTime::BeforeMarket,
                    _ => AnnounceTime::Unknown,
                };

                Ok(EarningsDateTime{
                    date: date,
                    time: earnings_time,
                })
            })

        })
        .map_or(Ok(None), |v| v.map(Some)) // Switch Option<Result<T, E>> to Result<Option<T>, Error>

}

fn extract_finviz(logger : &slog::Logger, mut response : reqwest::Response) -> Result<Option<EarningsDateTime>, Error> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r#"(\S+ \d{1,2})\s*(AMC|BMO)?"#).unwrap();
    }

    let text = response.text()?;
    let document = Html::parse_document(text.as_str());
    let selector = Selector::parse(r#"table.snapshot-table2 tr:nth-child(11) > td:nth-child(6) > b"#).unwrap();

    document.select(&selector)
        .next()
        .and_then(|node| node.text().next())
        .and_then(|text| {
            RE.captures_iter(text)
                .next()
                .map(|cap| {
                    // Special date parsing because this one doesn't include the year.
                    let mut parsed = chrono::format::Parsed::new();
                    chrono::format::parse(&mut parsed, &cap[1], chrono::format::strftime::StrftimeItems::new("%b %d"))?;

                    let mut date = Date::from_ymd(2018, parsed.month.unwrap(), parsed.day.unwrap());
                    if date < chrono::Local::today().naive_local() {
                        date = date.with_year(date.year() + 1).unwrap();
                    }

                    // let date = Date::parse_from_str(&cap[1], "%b %d")?;
                    let earnings_time = match &cap[2] {
                        "AMC" => AnnounceTime::AfterMarket,
                        "BMO" => AnnounceTime::BeforeMarket,
                        _ => AnnounceTime::Unknown,
                    };

                    Ok(EarningsDateTime {
                        date: date,
                        time: earnings_time,
                    })
                })
        })
        .map_or(Ok(None), |v| v.map(Some)) // Switch Option<Result<T, E>> to Result<Option<T>, Error>
}

fn extract_yahoo(logger : &slog::Logger, mut response : reqwest::Response) -> Result<Option<EarningsDateTime>, Error> {
    let text = response.text()?;
    let prefix = "root.App.main = ";

    text.as_str()
        .lines()
        .find(|line| line.starts_with(prefix))
        .ok_or_else(|| format_err!("Could not location JSON bootstrap payload"))
        .and_then(|line| {
            let value = json::parse(&line[prefix.len()..line.len()-1])?;
            let date = value["context"]["dispatcher"]["stores"]["QuoteSummaryStore"]["calendarEvents"]["earnings"]["earningsDate"][0]["raw"].as_i64()
                .map(|v| {
                    let d = chrono::NaiveDateTime::from_timestamp(v, 0).date();
                    EarningsDateTime{
                        date: d,
                        time: AnnounceTime::Unknown,
                    }
                });

            Ok(date)
        })
}

fn extract_zacks(logger : &slog::Logger, mut response : reqwest::Response) -> Result<Option<EarningsDateTime>, Error> {
    let text = response.text()?;
    let document = Html::parse_document(text.as_str());
    let main_selector = Selector::parse(r#"#stock_key_earnings > table > tbody > tr:nth-child(5) > td:nth-child(2)"#).unwrap();
    let sup_selector = Selector::parse(r#"sup"#).unwrap();

    let earnings_node = document.select(&main_selector).next().ok_or(EarningsError::SelectorNotFound)?;

    let time = earnings_node.select(&sup_selector)
        .next()
        .and_then(|node| node.text().next())
        .map_or(AnnounceTime::Unknown, |text| {
            match text {
                "*AMC" => AnnounceTime::AfterMarket,
                "*BMO" => AnnounceTime::BeforeMarket,
                _ => AnnounceTime::Unknown,
            }
        });

    earnings_node
        .children()
        .find(|node| node.value().is_text())
        .and_then(|date_text_node| date_text_node.value().as_text())
        .map(|date_text| {
            let date = Date::parse_from_str(date_text, "%m/%d/%y").with_context(|_| format!("parsing date {:?}", date_text))?;

            Ok(EarningsDateTime{
                date: date,
                time: time,
            })
        })
        .map_or(Ok(None), |v| v.map(Some)) // Switch Option<Result<T, E>> to Result<Option<T>, Error>
}

pub fn get_earnings_date_estimates(logger : &slog::Logger, client : &reqwest::Client, symbol : &str) -> Vec<SourcedEarningsTime> {
    crossbeam::scope(|scope| {
        let joins = SOURCES.iter()
            .map(|source| {
                scope.spawn(move || {
                    let url = source.url.replace("{}", symbol);
                    let response = client.get(url.as_str()).send().with_context(|_| format!("URL {}", url))?;
                    let is_success = response.status().is_success();
                    if !is_success {
                        return Err(response.error_for_status().unwrap_err().into());
                    }

                    let d = (source.extract)(logger, response).with_context(|_| format!("URL {}", url))?
                        .map(|datetime| SourcedEarningsTime{
                            datetime: datetime,
                            source: source.name,
                        });

                    if d.is_none() {
                        warn!(logger, "URL {} had no earnings date", url);
                    }

                    let x : Result<_, Error> = Ok(d);
                    x
                })
            })
            .collect::<Vec<_>>();

        joins.into_iter()
            .filter_map(|j| {
                match j.join() {
                    Err(e) => {
                        // let the_error = e.cause();
                        let msg = e.causes()
                            .map(|e| e.to_string())
                            .join("\n  ");
                        error!(logger, "{}", msg);
                        None
                    },
                    Ok(date) => date
                }
            })
            .collect::<Vec<_>>()
    })
}

use std::fmt;
use std::fmt::{Display, Formatter};
use std::borrow::Cow;
use crossbeam;
use itertools::Itertools;
use slog;
use reqwest;
use std::collections::HashMap;
use failure::{Error, ResultExt, err_msg};
use scraper::{Html, Selector};
use chrono;
use chrono::{NaiveDate, Datelike, Timelike, Weekday, Duration, DateTime, TimeZone, Utc};
use regex::Regex;
use json;
use htmlescape::decode_html;

#[derive(Debug, Fail)]
enum EarningsError {
    #[fail(display = "Could not find selector")]
    SelectorNotFound,

    #[fail(display = "Could not locate JSON bootstrap payload")]
    JsonPayloadNotFound,
}

struct EarningsSource {
    name : &'static str,
    url: &'static str,
    extract: (fn(logger : &slog::Logger, reqwest::Response) -> Result<Option<EarningsDateTime>,Error>),
}

static SOURCES : &[EarningsSource] = &[
        // EarningsSource{
        //     name: "Bloomberg",
        //     url: "https://www.bloomberg.com/quote/{}:US",
        //     extract: extract_bloomberg,
        // },
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
        EarningsSource{
            name: "Estimize",
            url: "https://www.estimize.com/{}",
            extract: extract_estimize,
        }
    ];


pub type Date = NaiveDate;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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

/// If the date falls on a weekend, step back to the closest weekday.
pub trait DatelikeExt {
    /// Get the closest trading day to this one, always going backwards on a weekend.
    fn closest_trading_day(&self) -> Self;
    fn next_trading_day(&self) -> Self;
    fn prev_trading_day(&self) -> Self;

}

impl DatelikeExt for Date {
    fn closest_trading_day(&self) -> Date {
        match self.weekday() {
            Weekday::Sat => *self - Duration::days(1),
            Weekday::Sun => *self - Duration::days(2),
            _ => *self,
        }
    }

    fn next_trading_day(&self) -> Date {
        match self.weekday() {
            Weekday::Fri => *self + Duration::days(3),
            Weekday::Sat => *self + Duration::days(2),
            _ => self.succ(),
        }
    }

    fn prev_trading_day(&self) -> Date {
        match self.weekday() {
            Weekday::Mon => *self - Duration::days(3),
            Weekday::Sun => *self - Duration::days(2),
            _ => self.pred(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EarningsDateTime{
    pub date: Date,
    pub time: AnnounceTime,
}

impl EarningsDateTime {
    /// Return the date of the last trading session before the earnings announcement, along with a "fuzzy" indication
    pub fn last_session(&self) -> (Date, bool) {
        match self.time {
            AnnounceTime::BeforeMarket => (self.date.prev_trading_day(), false),
            AnnounceTime::AfterMarket => (self.date, false),
            AnnounceTime::Unknown => (self.date, true),
        }
    }
}

impl Display for EarningsDateTime {
    fn fmt(&self, f : &mut Formatter) -> Result<(), fmt::Error> {
        match self.time {
            AnnounceTime::Unknown => write!(f, "{}", self.date),
            _ => write!(f, "{} {}", self.date, self.time),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcedEarningsTime {
    pub datetime : EarningsDateTime,
    pub source : Cow<'static, str>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarningsGuess {
    pub last_session : Date,
    pub concurrences : Vec<SourcedEarningsTime>,
    pub close_disagreements : Vec<SourcedEarningsTime>,
    pub far_disagreements : Vec<SourcedEarningsTime>,
}

pub fn best_earnings_guess(dates : &[SourcedEarningsTime]) -> Option<EarningsGuess> {

    let mut guesses : HashMap<Date, Vec<(&SourcedEarningsTime, bool)>> = HashMap::new();

    // Group the EarningsDates by the last trading session.
    for date in dates {
        let (last, fuzz) = date.datetime.last_session();
        guesses.entry(last)
            .or_insert_with(Vec::new)
            .push((date, fuzz));

        if fuzz {
            // Add entries for the next and previous weekdays
            guesses.entry(last.next_trading_day())
                .or_insert_with(Vec::new)
                .push((date, true));

            guesses.entry(last.prev_trading_day())
                .or_insert_with(Vec::new)
                .push((date, true));
        }
    }

    // Now that they're grouped by date, figure out which one is the best guess.
    let mut highest_fuzzy_count = 0;
    let mut highest_fuzzy_date = Date::from_num_days_from_ce(1);
    let mut highest_exact_count = 0;
    let mut highest_exact_date = Date::from_num_days_from_ce(1);

    // Get the highest count for both exact dates and fuzzy dates, giving preference to the earliest date.
    let today = chrono::Local::today().naive_local();
    for (date, guess) in guesses.iter() {
        let date = *date;
        if date < today {
            // Past earnings dates should never be a candidate.
            continue
        }

        let fuzzy_count = guess.len();
        let exact_count = guess.iter().filter(|&&(_, from_fuzzy)| from_fuzzy).count();

        if fuzzy_count > highest_fuzzy_count || (fuzzy_count == highest_fuzzy_count && date < highest_fuzzy_date) {
            highest_fuzzy_count = fuzzy_count;
            highest_fuzzy_date = date;
        }

        if exact_count > highest_exact_count || (exact_count == highest_exact_count && date < highest_exact_date) {
            highest_exact_count = fuzzy_count;
            highest_exact_date = date;
        }
    }

    if highest_fuzzy_count == 0 {
        return None
    }

    let best_date = highest_fuzzy_date;
    let concurrences = guesses.remove(&best_date).unwrap_or_else(Vec::new).iter().map(|&(guess, _)| guess.clone()).collect::<Vec<_>>();

    let prev_date = guesses.remove(&best_date.prev_trading_day()).unwrap_or_else(Vec::new);
    let next_date = guesses.remove(&best_date.next_trading_day()).unwrap_or_else(Vec::new);

    let close_disagreements : Vec<SourcedEarningsTime> = prev_date.into_iter().chain(next_date.into_iter())
        .fold(Vec::new(), |mut acc, (guess, _)| {
            // Really simple dedupe check. Since there are only a few sources this is fine and we don't need an O(1) lookup.
            if concurrences.iter().find(|g| g.source == guess.source).is_none() && acc.iter().find(|g| g.source == guess.source).is_none() {
                acc.push(guess.clone());
            }
            acc
        });

    // Everything else we haven't seen yet is a far disagreement.
    let far_disagreements : Vec<SourcedEarningsTime> = guesses.into_iter()
        .flat_map(|(_, guesses)| guesses.into_iter())
        .fold(Vec::new(), |mut acc, (guess, _)| {
            if concurrences.iter().find(|g| g.source == guess.source).is_none() &&
                close_disagreements.iter().find(|g| g.source == guess.source).is_none() &&
                acc.iter().find(|g| g.source == guess.source).is_none() {

                acc.push(guess.clone());
            }
            acc
        });

    Some(EarningsGuess {
        last_session: best_date,
        concurrences: concurrences,
        close_disagreements: close_disagreements,
        far_disagreements: far_disagreements,
    })
}

#[allow(dead_code)]
fn extract_bloomberg(_logger : &slog::Logger, mut response : reqwest::Response) -> Result<Option<EarningsDateTime>, Error> {
    let document = Html::parse_document(response.text()?.as_str());
    let selector = Selector::parse(r#"span[class^="nextAnnouncementDate"]"#).unwrap();
    document.select(&selector)
        .next()
        .and_then(|node| node.text().next())
        .map(|text| Date::parse_from_str(text, "%m/%d/%Y").map(|d| EarningsDateTime{date: d, time: AnnounceTime::Unknown} ))
        .map_or(Ok(None), |v| v.map(Some)) // Switch Option<Result<T, E>> to Result<Option<T>, Error>
        .map_err(|e| e.into())
}


#[allow(dead_code)]
fn extract_nasdaq(_logger : &slog::Logger, mut response : reqwest::Response) -> Result<Option<EarningsDateTime>, Error> {

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
            .and_then(|cap| {

                let earnings_time = cap.get(2)
                    .map_or(AnnounceTime::Unknown, |m| {
                        match m.as_str() {
                            "after market close" => AnnounceTime::AfterMarket,
                            "before market open" => AnnounceTime::BeforeMarket,
                            _ => AnnounceTime::Unknown,
                        }
                    });

                cap.get(1).map(|m| {
                    let date = Date::parse_from_str(m.as_str(), "%m/%d/%Y")?;
                    Ok(EarningsDateTime{
                        date: date,
                        time: earnings_time,
                    })
                })
            })

        })
        .map_or(Ok(None), |v| v.map(Some)) // Switch Option<Result<T, E>> to Result<Option<T>, Error>

}

fn extract_finviz(_logger : &slog::Logger, mut response : reqwest::Response) -> Result<Option<EarningsDateTime>, Error> {
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
                .and_then(|cap| {
                    let earnings_time = cap.get(2).map_or(AnnounceTime::Unknown, |m| {
                        match m.as_str() {
                            "AMC" => AnnounceTime::AfterMarket,
                            "BMO" => AnnounceTime::BeforeMarket,
                            _ => AnnounceTime::Unknown,
                        }
                    });

                    // Special date parsing because this one doesn't include the year.
                    cap.get(1).map(|text| {
                        let mut parsed = chrono::format::Parsed::new();
                        chrono::format::parse(&mut parsed, text.as_str(), chrono::format::strftime::StrftimeItems::new("%b %d"))?;
                        let today = chrono::Local::today().naive_local();
                        let mut date = Date::from_ymd(today.year(), parsed.month.unwrap(), parsed.day.unwrap());
                        // If it's in the past (minus a bit of buffer for recent earnings), then it's probably next year.
                        if date < (today - Duration::days(30)) {
                            date = date.with_year(date.year() + 1).unwrap();
                        }

                        Ok(EarningsDateTime {
                            date: date,
                            time: earnings_time,
                        })
                    })
                })

        })
        .map_or(Ok(None), |v| v.map(Some)) // Switch Option<Result<T, E>> to Result<Option<T>, Error>
}

fn extract_yahoo(_logger : &slog::Logger, mut response : reqwest::Response) -> Result<Option<EarningsDateTime>, Error> {
    let text = response.text()?;
    let prefix = "root.App.main = ";

    text.as_str()
        .lines()
        .find(|line| line.starts_with(prefix))
        .ok_or_else(|| Error::from(EarningsError::JsonPayloadNotFound))
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

fn extract_zacks(_logger : &slog::Logger, mut response : reqwest::Response) -> Result<Option<EarningsDateTime>, Error> {
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

fn extract_estimize(logger : &slog::Logger, mut response : reqwest::Response) -> Result<Option<EarningsDateTime>, Error> {
    lazy_static! {
        static ref MATCH_RE: Regex = Regex::new(r#"data-react-class="releases/app""#).unwrap();
        static ref EXTRACT_RE: Regex = Regex::new(r#"data-react-props="(.*)">"#).unwrap();
        static ref TODAY : DateTime<Utc> = Utc::now();
    }

    let text = response.text()?;

    text.as_str()
        .lines()
        .find(|line| MATCH_RE.is_match(line))
        .ok_or_else(|| err_msg("match re did not match"))
        .and_then(|line| {
            EXTRACT_RE.captures_iter(line)
                .next()
                .and_then(|c| c.get(1))
                .ok_or_else(|| err_msg("extract RE did not match"))
        })
        .and_then(|mtch| {
            decode_html(mtch.as_str())
                .map_err(|e| format_err!("decode failed: {:?}", e))
        })
        .and_then(|line| {

            warn!(logger, "{}", line);
            let value = json::parse(&line)?;

            let earnings_date = value["presenter"]["allReleases"]
                .members()
                .map(|val| {
                    let report_time = val["reportsAt"].as_i64().unwrap_or(0) / 1000;
                    Utc.timestamp(report_time, 0)
                })
                .find(|date| date > &*TODAY /* This is something weird with lazy_static? Probably a better way to do this */)
                .map(|date| {
                    // Market closes at 4:00 PM EST. Accounting for DST we'll check for 8:00 PM UTC as the market close time.
                    let after_market = date.hour() > 20;

                    let time = if after_market { AnnounceTime::AfterMarket } else { AnnounceTime::BeforeMarket };

                    EarningsDateTime{
                        date: date.date().naive_local(),
                        time: time,
                    }
                });

            Ok(earnings_date)
        })
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
                            source: source.name.into(),
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
                        warn!(logger, "{}", msg);
                        None
                    },
                    Ok(date) => date
                }
            })
            .collect::<Vec<_>>()
    })
}

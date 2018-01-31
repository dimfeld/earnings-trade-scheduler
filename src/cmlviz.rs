use failure::{Error, ResultExt};
use earnings::{Date, EarningsDateTime, AnnounceTime};
use chrono::{Datelike, Duration, Weekday};

#[derive(Debug,Deserialize,Serialize,Clone,Copy)]
pub enum Strategy {
    #[serde(rename="call_3d_preearnings")]
    Call3DaysBeforeEarnings,

    #[serde(rename="call_7d_preearnings")]
    Call7DaysBeforeEarnings,

    #[serde(rename="call_14d_preearnings")]
    Call14DaysBeforeEarnings,

    #[serde(rename="strangle_7d_preearnings")]
    Strangle7DaysBeforeEarnings,

    #[serde(rename="strangle_14d_preearnings")]
    Strangle14DaysBeforeEarnings,
}

impl Strategy {
    pub fn open_date(&self, last_preearnings_session : Date) -> Date {
        match *self {
            Strategy::Call3DaysBeforeEarnings => {
                let crosses_weekend = match last_preearnings_session.weekday() {
                    Weekday::Mon | Weekday::Tue | Weekday::Wed => true,
                    _ => false
                };

                let delta = if crosses_weekend { 5 } else {3};
                last_preearnings_session - Duration::days(delta)
            },
            Strategy::Call7DaysBeforeEarnings => last_preearnings_session - Duration::days(7),
            Strategy::Call14DaysBeforeEarnings => last_preearnings_session - Duration::days(14),
            Strategy::Strangle7DaysBeforeEarnings => last_preearnings_session - Duration::days(7),
            Strategy::Strangle14DaysBeforeEarnings => last_preearnings_session - Duration::days(14),
        }
    }

    pub fn close_date(&self, last_preearnings_session : Date) -> Date {
        match *self {
            Strategy::Call3DaysBeforeEarnings => last_preearnings_session,
            Strategy::Call7DaysBeforeEarnings => last_preearnings_session,
            Strategy::Call14DaysBeforeEarnings => last_preearnings_session,
            Strategy::Strangle7DaysBeforeEarnings => last_preearnings_session,
            Strategy::Strangle14DaysBeforeEarnings => last_preearnings_session,
        }
    }

    pub fn short_name(&self) -> &'static str {
        match *self {
            Strategy::Call3DaysBeforeEarnings => "E-3 Call",
            Strategy::Call7DaysBeforeEarnings => "E-7 Call",
            Strategy::Call14DaysBeforeEarnings => "E-14 Call",
            Strategy::Strangle7DaysBeforeEarnings => "E-7 Strangle",
            Strategy::Strangle14DaysBeforeEarnings => "E-14 Strangle",
        }
    }
}

#[derive(Debug,Deserialize)]
pub struct BacktestResultInput {
    pub symbol : String,
    pub wins : usize,
    pub losses: usize,
    pub win_rate : String,
    pub avg_trade_return : String,
    pub total_return : String,
    pub backtest_length : usize,
    pub next_earnings : String,
    pub strategy : Strategy,
}

#[derive(Debug,Clone,Serialize)]
pub struct BacktestResult {
    pub symbol : String,
    pub wins : usize,
    pub losses: usize,
    pub win_rate : i32,
    pub avg_trade_return : i32,
    pub total_return : i32,
    pub backtest_length : usize,
    pub next_earnings : EarningsDateTime,
    pub strategy : Strategy,
}

impl BacktestResult {
    #[inline]
    fn sort_key(&self) -> i32 { self.avg_trade_return }

    pub fn from_input(input : BacktestResultInput) -> Result<BacktestResult, Error> {
        let earnings_date = Date::parse_from_str(input.next_earnings.as_str(), "%Y‑%m‑%d")
            .map(|d| EarningsDateTime{date: d, time: AnnounceTime::Unknown})
            .with_context(|e| format!("next_earnings `{}` {}", input.next_earnings, e))?;

        let win_rate = input.win_rate.chars()
            .take_while(|x| x.is_digit(10) || *x == '‑')
            .map(|x| if x == '‑' { '-' } else {x})
            .collect::<String>()
            .parse::<i32>()
            .with_context(|e| format!("win_rate {} : {}", input.win_rate, e))?;

        let avg_trade_return = input.avg_trade_return.chars()
            .take_while(|x| x.is_digit(10) || *x == '‑')
            .map(|x| if x == '‑' { '-' } else {x})
            .collect::<String>()
            .parse::<i32>()
            .with_context(|e| format!("avg_trade_return {} : {}", input.avg_trade_return, e))?;

        let total_return = input.total_return.chars()
            .take_while(|x| x.is_digit(10) || *x == '‑')
            .map(|x| if x == '‑' { '-' } else {x})
            .collect::<String>().parse::<i32>()
            .with_context(|e| format!("total_return {} : {}", input.total_return, e))?;

        Ok(BacktestResult{
            symbol: input.symbol,
            wins: input.wins,
            losses: input.losses,
            win_rate: win_rate,
            avg_trade_return: avg_trade_return,
            total_return: total_return,
            backtest_length: input.backtest_length,
            next_earnings: earnings_date,
            strategy: input.strategy,
        })
    }
}

pub fn get_best_test(tests : &[BacktestResult]) -> usize {
    tests.iter()
        .enumerate()
        .max_by_key(|&(_, x)| x.sort_key())
        .map(|x| x.0)
        .unwrap_or(0)
}
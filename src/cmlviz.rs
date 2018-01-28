use failure::Error;
use earnings::{Date, EarningsDateTime, AnnounceTime, DatelikeExt};
use chrono::{Duration};

#[derive(Debug,Deserialize,Clone,Copy)]
pub enum Strategy {
    #[serde(rename="call_3d_preearnings")]
    Call3DaysBeforeEarnings,

    #[serde(rename="call_3d_preearnings")]
    Call7DaysBeforeEarnings,

    #[serde(rename="call_14d_preearnings")]
    Call14DaysBeforeEarnings,
}

impl Strategy {
    pub fn open_date(&self, last_preearnings_session : Date) -> Date {
        match *self {
            Strategy::Call3DaysBeforeEarnings => (last_preearnings_session - Duration::days(3)).closest_trading_day(),
            Strategy::Call7DaysBeforeEarnings => last_preearnings_session - Duration::days(7),
            Strategy::Call14DaysBeforeEarnings => last_preearnings_session - Duration::days(14),
        }
    }

    pub fn close_date(&self, last_preearnings_session : Date) -> Date {
        match *self {
            Strategy::Call3DaysBeforeEarnings => last_preearnings_session,
            Strategy::Call7DaysBeforeEarnings => last_preearnings_session,
            Strategy::Call14DaysBeforeEarnings => last_preearnings_session,
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
    pub backtest_len : usize,
    pub next_earnings : String,
    pub strategy : Strategy,
}

#[derive(Debug,Clone)]
pub struct BacktestResult {
    pub symbol : String,
    pub wins : usize,
    pub losses: usize,
    pub win_rate : i32,
    pub avg_trade_return : i32,
    pub total_return : i32,
    pub backtest_len : usize,
    pub next_earnings : EarningsDateTime,
    pub strategy : Strategy,
}

impl BacktestResult {
    #[inline]
    fn sort_key(&self) -> i32 { self.avg_trade_return }

    pub fn from_input(input : BacktestResultInput) -> Result<BacktestResult, Error> {
        let earnings_date = Date::parse_from_str(input.next_earnings.as_str(), "%m/%d%y")
            .map(|d| EarningsDateTime{date: d, time: AnnounceTime::Unknown})?;

        Ok(BacktestResult{
            symbol: input.symbol,
            wins: input.wins,
            losses: input.losses,
            win_rate: input.win_rate.parse::<i32>()?,
            avg_trade_return: input.avg_trade_return.parse::<i32>()?,
            total_return: input.total_return.parse::<i32>()?,
            backtest_len: input.backtest_len,
            next_earnings: earnings_date,
            strategy: input.strategy,
        })
    }
}

pub fn get_best_test(tests : &[BacktestResult]) -> BacktestResult {
    tests.iter().max_by_key(|x| x.sort_key()).unwrap().clone()
}
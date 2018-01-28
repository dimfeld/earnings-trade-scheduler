use failure::{Error, ResultExt};
use earnings::{Date, EarningsDateTime, AnnounceTime, DatelikeExt};
use chrono::{Duration};

#[derive(Debug,Deserialize,Clone,Copy)]
pub enum Strategy {
    #[serde(rename="call_3d_preearnings")]
    Call3DaysBeforeEarnings,

    #[serde(rename="call_7d_preearnings")]
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
    pub backtest_length : usize,
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
    pub backtest_length : usize,
    pub next_earnings : EarningsDateTime,
    pub strategy : Strategy,
}

impl BacktestResult {
    #[inline]
    fn sort_key(&self) -> i32 { self.avg_trade_return }

    pub fn from_input(input : BacktestResultInput) -> Result<BacktestResult, Error> {
        let earnings_date = Date::parse_from_str(input.next_earnings.as_str(), "%m/%d/%y")
            .map(|d| EarningsDateTime{date: d, time: AnnounceTime::Unknown})
            .with_context(|e| format!("next_earnings {} : {}", input.next_earnings, e))?;

        let win_rate = input.win_rate.chars()
            .take_while(|x| x.is_digit(10))
            .collect::<String>()
            .parse::<i32>()
            .with_context(|e| format!("win_rate {} : {}", input.win_rate, e))?;

        let avg_trade_return = input.avg_trade_return.chars()
            .take_while(|x| x.is_digit(10))
            .collect::<String>()
            .parse::<i32>()
            .with_context(|e| format!("avg_trade_return {} : {}", input.avg_trade_return, e))?;

        let total_return = input.total_return.chars()
            .take_while(|x| x.is_digit(10))
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

pub fn get_best_test(tests : &[BacktestResult]) -> BacktestResult {
    tests.iter().max_by_key(|x| x.sort_key()).unwrap().clone()
}
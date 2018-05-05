use failure::{Error, ResultExt, err_msg};
use earnings::{Date, DatelikeExt, EarningsDateTime, AnnounceTime};
use chrono::{Datelike, Duration, Weekday};
use std::str::FromStr;
use std::collections::HashMap;

#[derive(Debug,Deserialize,Serialize,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub enum Strategy {
    #[serde(rename="call_3d_preearnings")]
    Call3DaysBeforeEarnings,

    #[serde(rename="call_7d_preearnings")]
    Call7DaysBeforeEarnings,

    #[serde(rename="call_14d_preearnings")]
    Call14DaysBeforeEarnings,

    #[serde(rename="strangle_4d_preearnings")]
    Strangle4DaysBeforeEarnings,

    #[serde(rename="strangle_7d_preearnings")]
    Strangle7DaysBeforeEarnings,

    #[serde(rename="strangle_14d_preearnings")]
    Strangle14DaysBeforeEarnings,

    #[serde(rename="put_spread_post_earnings")]
    PutSpreadAfterEarnings,

    #[serde(rename="iron_condor_post_earnings")]
    IronCondorAfterEarnings,

    #[serde(rename="long_straddle_post_earnings")]
    LongStraddleAfterEarnings,
}

impl FromStr for Strategy {
    type Err = Error;
    fn from_str(s : &str) -> Result<Self, Self::Err> {
        match s {
            "call_3d_preearnings" => Ok(Strategy::Call3DaysBeforeEarnings),
            "call_7d_preearnings" => Ok(Strategy::Call7DaysBeforeEarnings),
            "call_14d_preearnings" => Ok(Strategy::Call14DaysBeforeEarnings),
            "strangle_4d_preearnings" => Ok(Strategy::Strangle4DaysBeforeEarnings),
            "strangle_7d_preearnings" => Ok(Strategy::Strangle7DaysBeforeEarnings),
            "strangle_14d_preearnings" => Ok(Strategy::Strangle14DaysBeforeEarnings),
            "put_spread_post_earnings" => Ok(Strategy::PutSpreadAfterEarnings),
            "iron_condor_post_earnings" => Ok(Strategy::IronCondorAfterEarnings),
            "long_straddle_post_earnings" => Ok(Strategy::LongStraddleAfterEarnings),
            _ => Err(err_msg(format!("Unknown strategy {}", s))),
        }
    }
}

impl Strategy {
    pub fn preearnings_strategies() -> Vec<Strategy> {
        vec![Strategy::Call3DaysBeforeEarnings, Strategy::Call7DaysBeforeEarnings, Strategy::Call14DaysBeforeEarnings, Strategy::Strangle4DaysBeforeEarnings, Strategy::Strangle7DaysBeforeEarnings, Strategy::Strangle14DaysBeforeEarnings]
    }

    pub fn postearnings_strategies() -> Vec<Strategy> {
        vec![Strategy::PutSpreadAfterEarnings, Strategy::IronCondorAfterEarnings, Strategy::LongStraddleAfterEarnings]
    }

    pub fn open_date(&self, last_preearnings_session : Date) -> Date {
        match *self {
            Strategy::Call3DaysBeforeEarnings => {
                let delta = match last_preearnings_session.weekday() {
                    Weekday::Mon | Weekday::Tue | Weekday::Wed => 5,
                    _ => 3
                };

                last_preearnings_session - Duration::days(delta)
            },
            Strategy::Call7DaysBeforeEarnings => last_preearnings_session - Duration::days(7),
            Strategy::Call14DaysBeforeEarnings => last_preearnings_session - Duration::days(14),
            Strategy::Strangle4DaysBeforeEarnings => {
                let delta = match last_preearnings_session.weekday() {
                    Weekday::Mon | Weekday::Tue | Weekday::Wed | Weekday::Thu => 6,
                    _ => 4,
                };

                last_preearnings_session - Duration::days(delta)
            },
            Strategy::Strangle7DaysBeforeEarnings => last_preearnings_session - Duration::days(7),
            Strategy::Strangle14DaysBeforeEarnings => last_preearnings_session - Duration::days(14),
            Strategy::PutSpreadAfterEarnings | Strategy::IronCondorAfterEarnings | Strategy::LongStraddleAfterEarnings => last_preearnings_session.next_trading_day(),
        }
    }

    pub fn close_date(&self, last_preearnings_session : Date) -> Date {
        let close = match *self {
            Strategy::Call3DaysBeforeEarnings => last_preearnings_session,
            Strategy::Call7DaysBeforeEarnings => last_preearnings_session,
            Strategy::Call14DaysBeforeEarnings => last_preearnings_session,
            Strategy::Strangle4DaysBeforeEarnings => last_preearnings_session,
            Strategy::Strangle7DaysBeforeEarnings => last_preearnings_session,
            Strategy::Strangle14DaysBeforeEarnings => last_preearnings_session,
            Strategy::PutSpreadAfterEarnings => last_preearnings_session + Duration::days(22),
            Strategy::IronCondorAfterEarnings => last_preearnings_session + Duration::days(32),
            Strategy::LongStraddleAfterEarnings => last_preearnings_session + Duration::days(8),
        };

        close.closest_trading_day()
    }

    pub fn short_name(&self) -> &'static str {
        match *self {
            Strategy::Call3DaysBeforeEarnings => "E-3 Call",
            Strategy::Call7DaysBeforeEarnings => "E-7 Call",
            Strategy::Call14DaysBeforeEarnings => "E-14 Call",
            Strategy::Strangle4DaysBeforeEarnings => "E-4 Strangle",
            Strategy::Strangle7DaysBeforeEarnings => "E-7 Strangle",
            Strategy::Strangle14DaysBeforeEarnings => "E-14 Strangle",
            Strategy::IronCondorAfterEarnings => "E+1 Iron Condor",
            Strategy::PutSpreadAfterEarnings => "E+1 Put Spread",
            Strategy::LongStraddleAfterEarnings => "E+1 Long Straddle",
        }
    }

    pub fn abbreviation(&self) -> &str {
        match *self {
            Strategy::Call3DaysBeforeEarnings => "E-3C",
            Strategy::Call7DaysBeforeEarnings => "E-7C",
            Strategy::Call14DaysBeforeEarnings => "E-14C",
            Strategy::Strangle4DaysBeforeEarnings => "E-4S",
            Strategy::Strangle7DaysBeforeEarnings => "E-7S",
            Strategy::Strangle14DaysBeforeEarnings => "E-14S",
            Strategy::IronCondorAfterEarnings => "E+1IC",
            Strategy::PutSpreadAfterEarnings => "E+1P",
            Strategy::LongStraddleAfterEarnings => "E+1LS",
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
    pub fn sort_key(&self) -> isize { self.avg_trade_return as isize }

    pub fn stats(&self) -> String {
        format!("({avg_return}%,{wins}/{losses})", avg_return=self.avg_trade_return, wins=self.wins, losses=self.losses)
    }

    pub fn from_input(input : BacktestResultInput) -> Result<BacktestResult, Error> {
        let earnings_str = input.next_earnings.replace("Not Verified", "");
        let earnings_date = Date::parse_from_str(earnings_str.as_str(), "%Y‑%m‑%d")
            .map(|d| EarningsDateTime{date: d, time: AnnounceTime::Unknown})
            .with_context(|e| format!("next_earnings `{}` {}", input.next_earnings, e))?;

        let win_rate = input.win_rate.chars()
            .take_while(|x| x.is_digit(10) || *x == '‑' || *x == '-') // These are actually two different dash characters
            .map(|x| if x == '‑' { '-' } else {x})
            .collect::<String>()
            .parse::<i32>()
            .with_context(|e| format!("win_rate {} : {}", input.win_rate, e))?;

        let avg_trade_return = input.avg_trade_return.chars()
            .take_while(|x| x.is_digit(10) || *x == '‑' || *x == '-')
            .map(|x| if x == '‑' { '-' } else {x})
            .collect::<String>()
            .parse::<i32>()
            .with_context(|e| format!("avg_trade_return {} : {}", input.avg_trade_return, e))?;

        let total_return = input.total_return.chars()
            .take_while(|x| x.is_digit(10) || *x == '‑' || *x == '-')
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

pub fn get_best_test_per_strategy(tests : &[BacktestResult]) -> HashMap<Strategy, usize> {
    tests.iter()
        .enumerate()
        .fold(HashMap::new(), |mut acc, (index, test)| {
            {
                let value = acc.entry(test.strategy).or_insert(index);
                if tests[*value].sort_key() < test.sort_key() {
                    *value = index;
                }
            }

            acc
        })
}
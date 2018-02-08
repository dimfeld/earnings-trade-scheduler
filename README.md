This utility takes a CSV generated from the CML TradeMachine scanner results and generates recommended trade entry and exit dates for each one.

There isn't an easy way to generate this CSV right now, but if you scan by strategy you can then export the results by setting a breakpoint in `datatables.js` where it has all the results and running some Javascript in the console to print the results.

The CSV should have this format: `symbol,wins,losses,win_rate,avg_trade_return,total_return,backtest_length,next_earnings,strategy`. The last column is the strategy name, which you'll need to fill in yourself, and the other columns are the raw data.

The `strategy` column should be one of these values depending on the strategy you're exporting:

* `call_3d_preearnings`
* `call_7d_preearnings`
* `call_14d_preearnings`
* `strangle_7d_preearnings`
* `strangle_14d_preearnings`
* `iron_condor_postearnings`
* `put_spread_postearnings`
* `long_straddle_postearnings`

If this sounds like a hassle, well, it is. But this utility saves me hours of work each week that I was spending picking the best strategy to use and verifying the correct earnings date.

### Usage

```
> earnings-trade-scheduler --help
earnings-trade-scheduler 1.0.0
Daniel Imfeld <dimfeld>
Earnings Trade Scheduler

USAGE:
    earnings-trade-scheduler [FLAGS] [OPTIONS] <input>

FLAGS:
    -h, --help       Prints help information
        --post       Include only post-earnings strategies
        --pre        Include only pre-earnings strategies
    -V, --version    Prints version information

OPTIONS:
        --end <end_date>              Process symbols with earnings before this date
    -o, --output <output>             Output file
        --save-raw <save_raw>         Save the raw data to a JSON file
        --start <start_date>          Process symbols with earnings after this date
    -s, --strategy <strategies>...    Strategies to include

ARGS:
    <input>    Input file
```
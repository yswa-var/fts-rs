# /// script
# requires-python = ">=3.14"
# dependencies = [
#     "marimo>=0.25.1",
# ]
# ///

"""Interactive notes and command runner for the fts-rs market-data pipeline."""

import marimo

__generated_with = "0.25.1"
app = marimo.App(width="full")


@app.cell
def _():
    import marimo as mo
    import subprocess
    from pathlib import Path

    return Path, mo, subprocess


@app.cell
def _(Path):
    repo_root = Path(__file__).resolve().parents[3]
    command_timeout_seconds = 30
    return command_timeout_seconds, repo_root


@app.cell
def _(mo):
    mo.md("""
    # fts-rs market-data pipeline notes

    The live path is:

    **Dhan WebSocket → Rust feed gateway → Redis Stream (`ticks`) → Rust
    bar aggregator → ClickHouse (`bars_1m`)**

    The historical path is separate:

    **Dhan historical API → historical worker → ClickHouse (`bar_1m` or
    `bar_daily`)**

    Long-running services are shown as commands but are not started
    automatically when this notebook opens. Run them in separate terminals
    so the feed, aggregator, and optional UI can run at the same time.
    """)
    return


@app.cell
def _():
    pipeline_commands = {
        "1. Check Rust targets": {
            "argv": ["cargo", "check", "--all-targets"],
            "kind": "one-shot",
            "notes": "Compile-checks the feed, bar aggregator, and historical worker.",
        },
        "2. Start Redis": {
            "argv": ["docker", "run", "-d", "--name", "redis", "-p", "6379:6379", "redis:latest"],
            "kind": "one-shot",
            "notes": "Creates the Redis transport used by the live tick stream.",
        },
        "3. Start ClickHouse": {
            "argv": [
                "docker", "run", "-d", "--name", "clickhouse",
                "--restart", "unless-stopped", "-p", "8123:8123", "-p", "9000:9000",
                "-e", "CLICKHOUSE_DB=quant", "-e", "CLICKHOUSE_USER=quant",
                "-e", "CLICKHOUSE_PASSWORD=quantpass",
                "-e", "CLICKHOUSE_DEFAULT_ACCESS_MANAGEMENT=1",
                "-v", "clickhouse_data:/var/lib/clickhouse",
                "clickhouse/clickhouse-server:latest",
            ],
            "kind": "one-shot",
            "notes": "Starts the analytical store. The bar writer creates bars_1m on startup.",
        },
        "4. Run live feed producer": {
            "argv": ["cargo", "run"],
            "kind": "long-running",
            "notes": "Authenticates with Dhan, loads master.csv instruments tagged GOLD, and publishes full-depth ticks to Redis.",
        },
        "5. Run live bar aggregator": {
            "argv": ["cargo", "run", "--bin", "bar_aggregator"],
            "kind": "long-running",
            "notes": "Reads Redis ticks through a consumer group, builds event-time one-minute bars, and batches them into ClickHouse.",
        },
        "6. Run live tick browser": {
            "argv": ["cargo", "run", "--bin", "tick_frontend"],
            "kind": "long-running",
            "notes": "Optional browser view at http://127.0.0.1:3000.",
        },
        "7. Run historical worker": {
            "argv": ["cargo", "run", "--bin", "historical_worker"],
            "kind": "long-running",
            "notes": "Optional HTTP worker at http://127.0.0.1:3001 for daily or one-minute backfills.",
        },
        "8. Backfill daily candles": {
            "argv": [
                "curl", "--request", "POST", "http://127.0.0.1:3001/historical/backfill",
                "--header", "Content-Type: application/json",
                "--data", '{"security_ids":["1333","11536"],"timeframe":"daily"}',
            ],
            "kind": "one-shot",
            "notes": "Resolves IDs through master.csv and writes to bar_daily.",
        },
        "9. Backfill one-minute candles": {
            "argv": [
                "curl", "--request", "POST", "http://127.0.0.1:3001/historical/backfill",
                "--header", "Content-Type: application/json",
                "--data", '{"security_ids":["1333"],"timeframe":"1m"}',
            ],
            "kind": "one-shot",
            "notes": "Fetches Dhan intraday data in 90-day windows and writes to bar_1m.",
        },
        "10. Check ClickHouse health": {
            "argv": ["curl", "--fail", "http://127.0.0.1:8123/ping"],
            "kind": "one-shot",
            "notes": "Returns pong when ClickHouse is reachable.",
        },
    }
    return (pipeline_commands,)


@app.cell
def _(mo, pipeline_commands):

    command_name = mo.ui.dropdown(
        options=list(pipeline_commands),
        value="1. Check Rust targets",
        label="Command",
    )
    run_command = mo.ui.run_button(label="Run selected command")
    command_name, run_command
    return command_name, run_command


@app.cell
def _(
    command_name,
    command_timeout_seconds,
    mo,
    pipeline_commands,
    repo_root,
    run_command,
    subprocess,
):
    selected = pipeline_commands[command_name.value]
    command_text = " ".join(subprocess.list2cmdline([argument]) for argument in selected["argv"])
    details = mo.md(
        f"""
        **Working directory:** `{repo_root}`  
        **Mode:** `{selected['kind']}`  
        **Command:** `{command_text}`  
        **Purpose:** {selected['notes']}
        """
    )

    if not run_command.value:
        result = mo.md("Select a command and press **Run selected command**.")
    elif selected["kind"] == "long-running":
        process = subprocess.Popen(
            selected["argv"],
            cwd=repo_root,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
        result = mo.md(
            f"Started process `{process.pid}`. Stop it from the terminal where it is running."
        )
    else:
        completed = subprocess.run(
            selected["argv"],
            cwd=repo_root,
            capture_output=True,
            text=True,
            timeout=command_timeout_seconds,
            check=False,
        )
        output = (completed.stdout + completed.stderr).strip() or "(no output)"
        result = mo.md(
            f"**Exit code:** `{completed.returncode}`\n\n```text\n{output}\n```"
        )

    details, result
    return


@app.cell
def _(mo):
    mo.md("""
    ## Operational notes

    - Set `CLIENT_ID`, `TOTP_KEY`, and `DPIN` in `.env` before starting Dhan-backed services.
    - Keep `master.csv` in the repository root; it resolves security IDs and selects the live universe.
    - Redis retains a bounded replay window controlled by `TICKS_MAXLEN` (default `750000`).
    - The live aggregator closes a minute when the first tick from a later event-time minute arrives.
    - Acknowledged Redis entries are not a permanent raw-tick archive; configure Redis persistence if crash recovery matters.
    - `run-live.sh` is a convenience alternative, but it assumes Docker containers and starts the feed plus bar aggregator together.
    """)
    return


@app.cell
def _():
    return


if __name__ == "__main__":
    app.run()

use std::env;
use std::path::PathBuf;
use std::process;

use rizdb::server::{self, Config};

fn main() {
    let config = match parse_config(env::args().skip(1)) {
        Ok(c) => c,
        Err(msg) => {
            eprintln!("rizdb: {msg}");
            eprintln!("usage: rizdb [--host HOST] [--port PORT] [--data-dir DIR] [--fsync-ms MS]");
            process::exit(2);
        }
    };
    if let Err(err) = server::serve(config) {
        eprintln!("rizdb: failed to serve: {err}");
        process::exit(1);
    }
}

fn parse_config(args: impl IntoIterator<Item = String>) -> Result<Config, String> {
    let mut config = Config::default();
    if let Ok(host) = env::var("RIZDB_HOST") {
        config.host = host;
    }
    if let Ok(port) = env::var("RIZDB_PORT") {
        config.port = port
            .parse()
            .map_err(|_| format!("invalid RIZDB_PORT: {port}"))?;
    }
    if let Ok(dir) = env::var("RIZDB_DATA_DIR") {
        config.data_dir = PathBuf::from(dir);
    }
    if let Ok(ms) = env::var("RIZDB_FSYNC_MS") {
        config.fsync_ms = ms
            .parse()
            .map_err(|_| format!("invalid RIZDB_FSYNC_MS: {ms}"))?;
    }

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--host" => {
                config.host = args.next().ok_or("missing value for --host")?;
            }
            "--port" => {
                let v = args.next().ok_or("missing value for --port")?;
                config.port = v.parse().map_err(|_| format!("invalid --port: {v}"))?;
            }
            "--data-dir" => {
                config.data_dir = PathBuf::from(args.next().ok_or("missing value for --data-dir")?);
            }
            "--fsync-ms" => {
                let v = args.next().ok_or("missing value for --fsync-ms")?;
                config.fsync_ms = v.parse().map_err(|_| format!("invalid --fsync-ms: {v}"))?;
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    if config.fsync_ms == 0 {
        return Err("--fsync-ms must be > 0 (interval fsync; default 1000)".into());
    }
    Ok(config)
}

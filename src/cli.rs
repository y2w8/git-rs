use clap::Parser;

#[derive(Parser)]
#[command(
    name = "rsgit",
    version,
    about = "A lightweight, blazingly fast Git web frontend written in Rust 🦀",
    long_about = "rsgit is a minimalist web interface for scouting and browsing local Git repositories, viewing diffs, and managing user patches natively."
)]
pub struct Args {
    /// The URL where the frontend is hosted (e.g., 127.0.0.1 or 0.0.0.0 for the public)
    #[arg(short, long, value_name = "URL", default_value = "127.0.0.1")]
    pub url: String,

    /// Port for the server to listen on
    #[arg(short, long, value_name = "PORT", default_value_t = 80)]
    pub port: u16,

    /// Repositories directory
    #[arg(short, long, value_name = "DIR", default_value = "/srv/git")]
    pub repos_dir: String,

    /// Install and enable the systemd service (/etc/systemd/system/git-rs.service)
    #[arg(long)]
    pub install_service: bool,
}

use anyhow::Result;
use clap::{Parser, Subcommand};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::Serialize;
use std::fs;

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[arg(long, default_value = "http://127.0.0.1:3010")]
    base_url: String,
    #[arg(long)]
    admin_token: Option<String>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Create {
        #[arg(long)]
        entry_type: String,
        #[arg(long)]
        title: String,
        #[arg(long, default_value = "")]
        narrative: String,
        #[arg(long, default_value = "")]
        narrative_file: String,
        #[arg(long, num_args=0.., default_values_t = Vec::<String>::new())]
        facts: Vec<String>,
        #[arg(long, num_args=0.., default_values_t = Vec::<String>::new())]
        concepts: Vec<String>,
        #[arg(long, num_args=0.., default_values_t = Vec::<String>::new())]
        files_read: Vec<String>,
        #[arg(long, num_args=0.., default_values_t = Vec::<String>::new())]
        files_modified: Vec<String>,
        #[arg(long)]
        author: Option<String>,
    },
    List {
        #[arg(long)]
        entry_type: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Search {
        query: String,
        #[arg(long)]
        entry_type: Option<String>,
        #[arg(long)]
        date_start: Option<String>,
        #[arg(long)]
        date_end: Option<String>,
        #[arg(long, default_value = "relevance")]
        order: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    Timeline {
        #[arg(long)]
        anchor_id: Option<String>,
        #[arg(long)]
        query: Option<String>,
        #[arg(long, default_value_t = 3)]
        depth_before: usize,
        #[arg(long, default_value_t = 3)]
        depth_after: usize,
        #[arg(long)]
        entry_type: Option<String>,
    },
    Fetch {
        ids: Vec<String>,
    },
}

#[derive(Serialize)]
struct CreatePayload<'a> {
    entry_type: &'a str,
    title: &'a str,
    narrative: &'a str,
    facts: &'a [String],
    concepts: &'a [String],
    files_read: &'a [String],
    files_modified: &'a [String],
    author: &'a Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let admin_token = cli
        .admin_token
        .or_else(|| std::env::var("ADMIN_API_TOKEN").ok())
        .expect("admin token missing: pass --admin-token or set ADMIN_API_TOKEN");

    let client = Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", admin_token))?,
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    match cli.command {
        Commands::Create {
            entry_type,
            title,
            narrative,
            narrative_file,
            facts,
            concepts,
            files_read,
            files_modified,
            author,
        } => {
            let narrative_text = if !narrative_file.is_empty() {
                fs::read_to_string(narrative_file)?
            } else {
                narrative
            };
            let payload = CreatePayload {
                entry_type: &entry_type,
                title: &title,
                narrative: &narrative_text,
                facts: &facts,
                concepts: &concepts,
                files_read: &files_read,
                files_modified: &files_modified,
                author: &author,
            };
            let url = format!("{}/memory/observations", cli.base_url);
            let resp = client
                .post(&url)
                .headers(headers.clone())
                .json(&payload)
                .send()?;
            println!("{}", resp.text()?);
        }
        Commands::List { entry_type, limit } => {
            let mut url = format!("{}/memory/observations?limit={}", cli.base_url, limit);
            if let Some(et) = entry_type {
                url.push_str(&format!("&entry_type={}", et));
            }
            let resp = client.get(&url).headers(headers.clone()).send()?;
            println!("{}", resp.text()?);
        }
        Commands::Search {
            query,
            entry_type,
            date_start,
            date_end,
            order,
            limit,
        } => {
            let url = format!("{}/memory/observations/search", cli.base_url);
            let payload = serde_json::json!({
                "query": query,
                "entry_type": entry_type,
                "date_start": date_start,
                "date_end": date_end,
                "order": order,
                "limit": limit,
            });
            let resp = client
                .post(&url)
                .headers(headers.clone())
                .json(&payload)
                .send()?;
            println!("{}", resp.text()?);
        }
        Commands::Timeline {
            anchor_id,
            query,
            depth_before,
            depth_after,
            entry_type,
        } => {
            let url = format!("{}/memory/observations/timeline", cli.base_url);
            let payload = serde_json::json!({
                "anchor_id": anchor_id,
                "query": query,
                "depth_before": depth_before,
                "depth_after": depth_after,
                "entry_type": entry_type,
            });
            let resp = client
                .post(&url)
                .headers(headers.clone())
                .json(&payload)
                .send()?;
            println!("{}", resp.text()?);
        }
        Commands::Fetch { ids } => {
            let url = format!("{}/memory/observations/fetch", cli.base_url);
            let payload = serde_json::json!({"ids": ids});
            let resp = client
                .post(&url)
                .headers(headers.clone())
                .json(&payload)
                .send()?;
            println!("{}", resp.text()?);
        }
    }

    Ok(())
}

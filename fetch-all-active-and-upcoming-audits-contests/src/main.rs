use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Deserialize, Debug, Clone)]
struct Resp {
    items: Vec<Item>,
}

#[derive(Deserialize, Debug, Clone)]
struct Item {
    id: String,
    name: String,
    #[serde(default)]
    kind: String,
    timeframe: Timeframe,
    #[serde(default)]
    status: String,
    #[serde(rename = "currencyCode", default)]
    currency_code: String,
    #[serde(rename = "totalRewardPot", default)]
    total_reward_pot: String,
    #[serde(rename = "totalFindings", default)]
    total_findings: u32,
    #[serde(rename = "assetGroups", default)]
    asset_groups: Vec<AssetGroup>,
}

#[derive(Deserialize, Debug, Clone)]
struct Timeframe {
    start: String,
    end: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
struct AssetGroup {
    #[serde(rename = "outOfScope", default)]
    out_of_scope: bool,
    #[serde(default)]
    rewards: Vec<Reward>,
}

#[derive(Deserialize, Debug, Clone)]
struct Reward {
    severity: String,
    #[serde(rename = "maxReward")]
    max_reward: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "https://api.cantina.xyz/api/v0/opportunities?limit=1000&kind=public_bounty%2Cprivate_bounty%2Cpublic_contest%2Cprivate_contest";
    let resp: Resp = reqwest::blocking::get(url)?.json()?;

    let mut bounties = Vec::new();
    let mut contests = Vec::new();

    for item in &resp.items {
        let kind_lower = item.kind.to_lowercase();
        if kind_lower.contains("bounty") {
            bounties.push(item.clone());
        } else if kind_lower.contains("contest") {
            contests.push(item.clone());
        }
    }

    // Sort bounties: newest first
    bounties.sort_by_key(|i| std::cmp::Reverse(parse_date(&i.timeframe.start)));

    // Sort contests: longest duration first
    contests.sort_by_key(|i| {
        let start = parse_date(&i.timeframe.start);
        let end = i.timeframe.end.as_ref().map(|s| parse_date(s)).unwrap_or(start);
        std::cmp::Reverse(end - start)
    });

    println!("=== BOUNTIES ({})\n", bounties.len());
    for b in &bounties {
        print_item(b, "bounties", None);
    }

    println!("\n=== COMPETITIONS ({})\n", contests.len());
    for c in &contests {
        let remaining = c.timeframe.end.as_ref().map(|end| {
            let end_dt = parse_date(end);
            let now = Utc::now();
            if end_dt > now {
                format_remaining(end_dt - now)
            } else {
                "Ended".to_string()
            }
        });
        print_item(c, "competitions", remaining);
    }

    Ok(())
}

fn parse_date(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn format_remaining(dur: Duration) -> String {
    let days = dur.num_days();
    let hours = dur.num_hours() % 24;
    let minutes = dur.num_minutes() % 60;

    if days > 0 {
        format!("{} days {}h {}m left", days, hours, minutes)
    } else if hours > 0 {
        format!("{}h {}m left", hours, minutes)
    } else {
        format!("{}m left", minutes)
    }
}

fn accepted_severities(groups: &[AssetGroup]) -> Vec<String> {
    let mut set = HashSet::new();
    for g in groups {
        if g.out_of_scope { continue; }
        for r in &g.rewards {
            if let Some(max) = &r.max_reward {
                let max_str = max.trim();
                if !max_str.is_empty() && max_str != "0" && max_str != "0.00" {
                    set.insert(r.severity.clone());
                }
            }
        }
    }
    let mut vec: Vec<_> = set.into_iter().collect();
    vec.sort_unstable();
    vec
}

fn print_item(i: &Item, path: &str, remaining: Option<String>) {
    let url = format!("https://cantina.xyz/{}/{}", path, i.id);
    let sevs = accepted_severities(&i.asset_groups);

    println!("Name: {}", i.name);
    println!("Kind: {}", i.kind);
    println!("Start: {}", i.timeframe.start);
    if let Some(end) = &i.timeframe.end {
        println!("End:   {}", end);
    }
    if let Some(rem) = remaining {
        println!("Time Left: {}", rem);
    }
    println!("URL: {}", url);
    println!("Status: {}", i.status);
    println!("Currency: {}", i.currency_code);
    println!("Total Reward Pot: {}", i.total_reward_pot);
    println!(
        "Accepted Severities: {}",
        if sevs.is_empty() { "None".to_string() } else { sevs.join(", ") }
    );
    println!("Total Findings: {}", i.total_findings);
    println!("---");
}
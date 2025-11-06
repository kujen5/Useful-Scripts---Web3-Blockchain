//! Immunefi Scraper – Scope from Detail JSON + Separate Smart Contract / Web Bounties
//! Run: cargo run --bin immunefi

use reqwest::blocking::get;
use scraper::{Html, Selector};
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;

#[derive(Deserialize, Debug)]
struct ListData {
    props: ListProps,
}
#[derive(Deserialize, Debug)]
struct ListProps {
    #[serde(rename = "pageProps")]
    page_props: ListPageProps,
}
#[derive(Deserialize, Debug)]
struct ListPageProps {
    bounties: Vec<ListBounty>,
}

#[derive(Deserialize, Debug, Clone)]
struct ListBounty {
    id: String,
    project: String,
    #[serde(rename = "maximum_reward")]
    max_bounty: u64,
    #[serde(rename = "vaultBalance")]
    vault_balance: Option<f64>,
    tags: Tags,
}

#[derive(Deserialize, Debug, Clone)]
struct Tags {
    language: Vec<String>,
    #[serde(rename = "productType")]
    product_type: Vec<String>,
    #[serde(rename = "programType")]
    program_type: Vec<String>,
    #[serde(rename = "projectType")]
    project_type: Vec<String>,
}

// === Detail page structs ===
#[derive(Deserialize, Debug)]
struct DetailData {
    props: DetailProps,
}
#[derive(Deserialize, Debug)]
struct DetailProps {
    #[serde(rename = "pageProps")]
    page_props: DetailPageProps,
}
#[derive(Deserialize, Debug)]
struct DetailPageProps {
    bounty: DetailBounty,
}
#[derive(Deserialize, Debug)]
struct DetailBounty {
    assets: Vec<Asset>,
    rewards: Vec<Reward>,
    legacy: Option<LegacyRewards>,
}

#[derive(Deserialize, Debug)]
struct Asset {
    #[serde(rename = "type")]
    asset_type: String,
    url: String,
    description: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Reward {
    severity: String,
    #[serde(rename = "fixedReward")]
    fixed_reward: Option<u64>,
    #[serde(rename = "maxReward")]
    max_reward: Option<u64>,
    #[serde(rename = "rewardModel")]
    reward_model: String,
}

// Legacy reward structure (for programs with split Smart Contract / Web)
#[derive(Deserialize, Debug)]
struct LegacyRewards {
    #[serde(rename = "smartcontract_rewards")]
    smart_contract: Option<Vec<LegacyReward>>,
    #[serde(rename = "web_rewards")]
    web: Option<Vec<LegacyReward>>,
}

#[derive(Deserialize, Debug)]
struct LegacyReward {
    level: String,
    payout: String,
}

#[derive(Default, Debug)]
struct Program {
    name: String,
    max_bounty: String,
    vault_tvl: String,
    detail_url: String,
    severities: Vec<String>,
    // Unified rewards (from modern `rewards` array)
    rewards: HashMap<String, String>,
    // Legacy split rewards
    smart_contract_rewards: HashMap<String, String>,
    web_rewards: HashMap<String, String>,
    github_links: Vec<String>,
    onchain_links: Vec<String>,
    has_github: bool,
    has_web_scope: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let list_url = "https://immunefi.com/bug-bounty/?filter=projectType%3DDefi%26language%3DSolidity%26programType%3DSmart%2BContract%26productType%3DStablecoin&sort=maximum_reward%3Aasc";

    let body = get(list_url)?.text()?;
    let doc = Html::parse_document(&body);

    let script_sel = Selector::parse(r#"script[id="__NEXT_DATA__"]"#).unwrap();
    let script = doc.select(&script_sel).next().ok_or("No __NEXT_DATA__")?;
    let json_text = script.inner_html();
    let list_data: ListData = serde_json::from_str(&json_text)?;

    let mut programs = Vec::new();

    for bounty in &list_data.props.page_props.bounties {
        if !matches_filter(&bounty.tags) {
            continue;
        }

        let detail_url = format!("https://immunefi.com/bounty/{}/", bounty.id);
        let vault_str = bounty.vault_balance
            .map(|v| format!("${:.1}k", v / 1000.0))
            .unwrap_or_else(|| if bounty.max_bounty > 0 { "Private".to_string() } else { "None".to_string() });

        let mut p = Program {
            name: bounty.project.clone(),
            max_bounty: format!("${}", format_number(bounty.max_bounty)),
            vault_tvl: vault_str,
            detail_url: detail_url.clone(),
            ..Default::default()
        };

        // Fetch detail page JSON
        if let Ok(detail_json) = fetch_detail_json(&detail_url) {
            // === Scope ===
            for asset in &detail_json.props.page_props.bounty.assets {
                if asset.asset_type != "smart_contract" && asset.asset_type != "web" {
                    continue;
                }

                let line = asset.description.as_ref()
                    .and_then(|d| if d.trim().is_empty() { None } else { Some(d.trim()) })
                    .map(|d| format!("{} → {}", d, asset.url))
                    .unwrap_or(asset.url.clone());

                if asset.asset_type == "web" {
                    p.web_rewards.insert("scope".to_string(), line.clone());
                    p.has_web_scope = true;
                } else if asset.url.contains("github.com") {
                    p.github_links.push(line);
                    p.has_github = true;
                } else if asset.url.contains("etherscan") || asset.url.contains("bscscan") || asset.url.contains("blockscout") {
                    p.onchain_links.push(line);
                }
            }

            // === Rewards: Modern + Legacy ===
            let bounty_detail = &detail_json.props.page_props.bounty;

            // 1. Try legacy split rewards
            if let Some(legacy) = &bounty_detail.legacy {
                if let Some(sc) = &legacy.smart_contract {
                    for r in sc {
                        if let Some(sev) = map_severity(&r.level) {
                            p.smart_contract_rewards.insert(sev.to_string(), r.payout.clone());
                        }
                    }
                }
                if let Some(web) = &legacy.web {
                    for r in web {
                        if let Some(sev) = map_severity(&r.level) {
                            p.web_rewards.insert(sev.to_string(), r.payout.clone());
                        }
                    }
                }
            }

            // 2. Fall back to modern rewards array
            if p.smart_contract_rewards.is_empty() && p.web_rewards.is_empty() {
                for reward in &bounty_detail.rewards {
                    if let Some(sev) = map_severity(&reward.severity) {
                        let rew = if let Some(fixed) = reward.fixed_reward {
                            format!("${}", format_number(fixed))
                        } else if let Some(max) = reward.max_reward {
                            if reward.reward_model == "up_to" {
                                format!("up to ${}", format_number(max))
                            } else {
                                format!("${}", format_number(max))
                            }
                        } else {
                            continue;
                        };
                        p.rewards.insert(sev.to_string(), rew);
                    }
                }
            }
        }

        programs.push(p);
    }

    if programs.is_empty() {
        println!("No programs match filters.");
        return Ok(());
    }

    programs.sort_by_key(|p| parse_amount(&p.max_bounty));

    println!("=== IMMUNEFI BOUNTY PROGRAMS ({})\n", programs.len());

    for prog in &programs {
        print_summary(prog);
        print_rewards(prog);
        print_scope(prog);
        println!("---\n");
    }

    Ok(())
}

fn matches_filter(tags: &Tags) -> bool {
    tags.language.iter().any(|l| l == "Solidity")
        && tags.project_type.iter().any(|t| t == "Defi")
        && tags.program_type.iter().any(|t| t == "Smart Contract")
        && tags.product_type.iter().any(|t| t == "Stablecoin")
}

fn fetch_detail_json(url: &str) -> Result<DetailData, Box<dyn Error>> {
    let body = get(url)?.text()?;
    let doc = Html::parse_document(&body);
    let script = doc.select(&Selector::parse(r#"script[id="__NEXT_DATA__"]"#).unwrap())
        .next().ok_or("No __NEXT_DATA__ on detail page")?;
    let json_text = script.inner_html();
    Ok(serde_json::from_str(&json_text)?)
}

fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{}M", n / 1_000_000)
    } else if n >= 1_000 {
        format!("{}k", n / 1_000)
    } else {
        n.to_string()
    }
}

fn parse_amount(s: &str) -> i64 {
    let s = s.replace(['$', ','], "").to_lowercase();
    let num: f64 = s.chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect::<String>()
        .parse()
        .unwrap_or(0.0);
    if s.contains('m') {
        (num * 1_000_000.0) as i64
    } else if s.contains('k') {
        (num * 1_000.0) as i64
    } else {
        num as i64
    }
}

fn map_severity(s: &str) -> Option<&'static str> {
    match s.to_lowercase().as_str() {
        "critical" => Some("Critical"),
        "high" => Some("High"),
        "medium" => Some("Medium"),
        "low" => Some("Low"),
        _ => None,
    }
}

// === Printing Functions ===

fn print_summary(p: &Program) {
    println!("Name: {}", p.name);
    println!("Max Bounty: {}", p.max_bounty);
    println!("Vault TVL: {}", p.vault_tvl);
    println!("Detail URL: {}", p.detail_url);
}

fn print_rewards(p: &Program) {
    let has_legacy = !p.smart_contract_rewards.is_empty() || !p.web_rewards.is_empty();
    let has_modern = !p.rewards.is_empty();

    if !has_legacy && !has_modern {
        println!("Rewards: Not specified");
        return;
    }

    if has_legacy {
        if !p.smart_contract_rewards.is_empty() {
            println!("Smart Contract Bounties:");
            for sev in &["Critical", "High", "Medium", "Low"] {
                if let Some(rew) = p.smart_contract_rewards.get(*sev) {
                    println!("  {}: {}", sev, rew);
                }
            }
        }
        if !p.web_rewards.is_empty() {
            println!("Websites & Applications Bounties:");
            for sev in &["Critical", "High", "Medium", "Low"] {
                if let Some(rew) = p.web_rewards.get(*sev) {
                    println!("  {}: {}", sev, rew);
                }
            }
        }
    } else if has_modern {
        println!("Bounties (Unified):");
        for sev in &["Critical", "High", "Medium", "Low"] {
            if let Some(rew) = p.rewards.get(*sev) {
                println!("  {}: {}", sev, rew);
            }
        }
    }
}

fn print_scope(p: &Program) {
    if p.github_links.is_empty() && p.onchain_links.is_empty() && !p.has_web_scope {
        println!("Scope: Not specified");
        return;
    }

    if !p.github_links.is_empty() {
        println!("GitHub: Yes ({} repo(s))", p.github_links.len());
        for link in &p.github_links {
            println!("  - {}", link);
        }
        println!("GitHub scope detected — audit code directly!");
    }

    if !p.onchain_links.is_empty() {
        println!("On-Chain: Yes ({} contract(s))", p.onchain_links.len());
        for link in &p.onchain_links {
            println!("  - {}", link);
        }
    }

    if p.has_web_scope {
        println!("Web Scope: Yes");
        // You can extend to list web URLs if needed
    }

    if p.github_links.is_empty() && !p.onchain_links.is_empty() {
        println!("No GitHub — on-chain audit only.");
    }
}
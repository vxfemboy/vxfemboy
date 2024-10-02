use chrono::prelude::*;
use figlet_rs::FIGfont;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::env;
use std::fs::File;
use std::io::Write;

fn get_github_activity(
    username: &str,
    token: &str,
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let url = format!("https://api.github.com/users/{}/events/public", username);
    let client = Client::new();

    client
        .get(&url)
        .header("Authorization", format!("token {}", token))
        .header("User-Agent", "Rust GitHub Action")
        .send()?
        .json::<Vec<Value>>()
        .map_err(|e| e.into())
}

fn get_all_languages(username: &str, token: &str) -> Vec<(String, f64)> {
    let url = format!("https://api.github.com/users/{}/repos", username);
    let client = Client::new();
    let repos = client
        .get(&url)
        .header("Authorization", format!("token {}", token))
        .header("User-Agent", "Rust GitHub Action")
        .send()
        .expect("Failed to fetch repositories")
        .json::<Vec<Value>>()
        .expect("Failed to parse JSON response for repositories");

    let mut languages: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

    for repo in repos {
        if let Some(lang_url) = repo["languages_url"].as_str() {
            let repo_langs = client
                .get(lang_url)
                .header("Authorization", format!("token {}", token))
                .header("User-Agent", "Rust GitHub Action")
                .send()
                .expect("Failed to fetch languages for a repository")
                .json::<Value>()
                .expect("Failed to parse JSON response for languages");

            if let Some(obj) = repo_langs.as_object() {
                for (lang, bytes) in obj {
                    let count = languages.entry(lang.clone()).or_insert(0);
                    *count += bytes.as_u64().unwrap_or(0);
                }
            }
        }
    }

    let total_bytes: u64 = languages.values().sum();
    let mut language_percentages: Vec<(String, f64)> = languages
        .into_iter()
        .map(|(lang, count)| (lang, (count as f64 / total_bytes as f64) * 100.0))
        .collect();

    language_percentages.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    language_percentages.truncate(10);
    language_percentages
}

fn create_ascii_bar(percentage: f64, width: usize) -> String {
    let filled_width = ((percentage / 100.0) * width as f64).round() as usize;
    let mut bar = String::new();

    for i in 0..width {
        let char = match i.cmp(&filled_width) {
            std::cmp::Ordering::Less => '█',    // Filled portion
            std::cmp::Ordering::Equal => '▓',   // Transition
            std::cmp::Ordering::Greater => '░', // Unfilled portion
        };
        bar.push(char);
    }

    format!("[{}]", bar)
}

fn format_activity(activity: &Value) -> String {
    let event_type = activity["type"].as_str().unwrap_or("").replace("Event", "");
    let repo = activity["repo"]["name"].as_str().unwrap_or("");
    let created_at = activity["created_at"].as_str().unwrap_or("");
    let dt = DateTime::parse_from_rfc3339(created_at).unwrap_or_else(|_| Utc::now().into());
    format!(
        "{:<16} | {:<15} | {}",
        dt.format("%Y-%m-%d %H:%M"),
        event_type,
        repo
    )
}

fn download_font() {
    let font_url = "https://raw.githubusercontent.com/thugcrowd/gangshit/master/gangshit2.flf";
    let client = Client::new();
    let response = client
        .get(font_url)
        .send()
        .expect("Failed to download FIGlet font");
    let mut file = File::create("gangshit1.flf").expect("Failed to create font file");
    file.write_all(&response.bytes().expect("Failed to read font bytes"))
        .expect("Failed to write to font file");
}

fn get_github_stats(username: &str, token: &str) -> serde_json::Value {
    let client = Client::new();

    let query = format!(
        r#"
        query {{
          user(login: "{}") {{
            name
            contributionsCollection {{
              totalCommitContributions
              totalPullRequestContributions
              totalIssueContributions
              restrictedContributionsCount
            }}
            repositories(first: 100, ownerAffiliations: OWNER, isFork: false) {{
              totalCount
              nodes {{
                stargazerCount
              }}
            }}
            repositoriesContributedTo(first: 1, contributionTypes: [COMMIT, ISSUE, PULL_REQUEST, REPOSITORY]) {{
              totalCount
            }}
          }}
        }}
        "#,
        username
    );

    let response = client
        .post("https://api.github.com/graphql")
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "Rust GitHub Action")
        .json(&json!({ "query": query }))
        .send()
        .expect("Failed to send GraphQL request");

    let data: serde_json::Value = response.json().expect("Failed to parse GraphQL response");

    let user = &data["data"]["user"];
    let contributions = &user["contributionsCollection"];
    let repositories = &user["repositories"];

    let total_stars: u64 = repositories["nodes"]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .map(|repo| repo["stargazerCount"].as_u64().unwrap_or(0))
        .sum();

    json!({
        "total_commits": contributions["totalCommitContributions"].as_u64().unwrap_or(0) +
                         contributions["restrictedContributionsCount"].as_u64().unwrap_or(0),
        "total_prs": contributions["totalPullRequestContributions"].as_u64().unwrap_or(0),
        "total_issues": contributions["totalIssueContributions"].as_u64().unwrap_or(0),
        "total_stars": total_stars,
        "repos_owned": repositories["totalCount"].as_u64().unwrap_or(0),
        "contributed_to": user["repositoriesContributedTo"]["totalCount"].as_u64().unwrap_or(0),
    })
}

fn format_github_stats(stats: &Value) -> String {
    format!(
        "+-------------+------------------------+----------------+--------------------------------------+\n\
         |   Metric    |         Value          |     Metric     |                Value                 |\n\
         +-------------+------------------------+----------------+--------------------------------------+\n\
         |   Commits   | {:>22} | Issues opened  | {:>36} |\n\
         | PRs opened  | {:>22} | Stars received | {:>36} |\n\
         | Repos owned | {:>22} | Contributed to | {:>36} |\n\
         +-------------+------------------------+----------------+--------------------------------------+",
        stats["total_commits"].as_u64().unwrap_or(0),
        stats["total_issues"].as_u64().unwrap_or(0),
        stats["total_prs"].as_u64().unwrap_or(0),
        stats["total_stars"].as_u64().unwrap_or(0),
        stats["repos_owned"].as_u64().unwrap_or(0),
        stats["contributed_to"].as_u64().unwrap_or(0)
    )
}

fn create_ascii_badge(label: &str, value: &str, width: usize) -> String {
    let total_width = width.max(label.len() + value.len() + 4);
    let label_width = label.len() + 2;
    let value_width = total_width - label_width;

    let top_bottom = "─".repeat(total_width);
    let label_part = format!(" {:<width$}", label, width = label_width - 2);
    let value_part = format!(" {:<width$} ", value, width = value_width - 2);

    format!(
        "╭{0}╮\n│{1}│{2}│\n╰{0}╯",
        top_bottom,
        label_part,
        value_part
    )
}

fn get_github_followers(username: &str, token: &str) -> u64 {
    let client = Client::new();
    let url = format!("https://api.github.com/users/{}", username);

    client
        .get(&url)
        .header("Authorization", format!("token {}", token))
        .header("User-Agent", "Rust GitHub Action")
        .send()
        .and_then(|response| response.json::<serde_json::Value>())
        .map(|json| json["followers"].as_u64().unwrap_or(0))
        .unwrap_or(0)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    download_font();

    let username = "vxfemboy";
    let token = env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN not set");

    // Step 3: Fetch GitHub data
    let activities = get_github_activity(username, &token)?;
    let top_languages = get_all_languages(username, &token);
    let github_stats = get_github_stats(username, &token);
    let github_followers = get_github_followers(username, &token);
    let github_stars = github_stats["total_stars"].as_u64().unwrap_or(0);

    // Step 4: Generate ASCII art header and badges
    let font = FIGfont::from_file("gangshit1.flf").expect("Failed to load FIGlet font");
    let figure = font.convert("SAD").expect("Failed to create ASCII art");
    let ascii_header = figure.to_string();
    let github_followers_badge = create_ascii_badge("Followers", &github_followers.to_string(), 20);
    let github_stars_badge = create_ascii_badge("Stars", &github_stars.to_string(), 20);

    let mut output = "> [!WARNING]\n> ```".to_string();

    let header_lines: Vec<&str> = ascii_header.lines().collect();
    let badges_string = format!("{}\n\n{}", github_followers_badge, github_stars_badge);
    let badge_lines: Vec<&str> = badges_string.lines().collect();
    let max_header_width = header_lines
        .iter()
        .map(|line| line.len())
        .max()
        .unwrap_or(0);

    let badge_offset = 4;

    for i in 0..header_lines.len().max(badge_lines.len() + badge_offset) {
        let header_part = header_lines.get(i).unwrap_or(&"").to_string();
        let badge_part = if i >= badge_offset {
            badge_lines.get(i - badge_offset).unwrap_or(&"").to_string()
        } else {
            String::new()
        };
        output += &format!(
            "> {:<width$} {}\n",
            header_part,
            badge_part,
            width = max_header_width + 2
        );
    }

    output += "> ```\n";
    output += "> <p>Software by this user may be <b>potentially hazardous</b>. Explore at your own risk.</p>\n\n";
    output += "---\n\n";

    output += "#### 🛠️ Languages\n";
    output += "```css\n";
    for (lang, percentage) in top_languages {
        output += &format!(
            "{:<12} {} {:.1}%\n",
            lang,
            create_ascii_bar(percentage, 20),
            percentage
        );
    }
    output += "```\n\n";

    output += "#### 📊 Stats\n";
    output += "```\n";
    output += &format_github_stats(&github_stats);
    output += "\n```\n\n";

    output += "#### 🔥 Activity\n";
    output += "```\n";
    output += &"-".repeat(60);
    output += "\n";
    for activity in activities.iter().take(5) {
        output += &format_activity(activity);
        output += "\n";
    }
    output += &"-".repeat(60);
    output += "\n\n";
    let now: DateTime<Local> = Local::now();
    output += &format!("Last updated: {}\n", now.format("%Y-%m-%d %H:%M:%S"));
    output += "```\n\n";

    output += "> [!NOTE]\n";
    output +=
        "> <p align=\"center\">This README is <b>auto-generated</b> with Rust and Actions</p>";

    let mut file = File::create("README.md").expect("Failed to create README.md");
    file.write_all(output.as_bytes())
        .expect("Failed to write to README.md");

    println!("✅ README.md has been updated successfully.");
    Ok(())
}

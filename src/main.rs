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
            std::cmp::Ordering::Less => '█',
            std::cmp::Ordering::Equal => '▒',
            std::cmp::Ordering::Greater => '░',
        };
        bar.push(char);
    }

    bar
}

fn format_activity(activity: &Value) -> String {
    let event_type = activity["type"].as_str().unwrap_or("").replace("Event", "");
    let repo = activity["repo"]["name"].as_str().unwrap_or("");
    let repo_short = if repo.len() > 30 { format!("{}...", &repo[..27]) } else { repo.to_string() };
    format!("{:<6} {}", event_type, repo_short)
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

    // Step 4: Generate ASCII art header
    let font = FIGfont::from_file("gangshit1.flf").expect("Failed to load FIGlet font");
    let figure = font.convert("ZOA").expect("Failed to create ASCII art");
    let ascii_header = figure.to_string();

    let mut output = "> [!WARNING]\n> ```\n".to_string();

    // ASCII header (no badges)
    for line in ascii_header.lines() {
        output += &format!("> {}\n", line);
    }

    output += ">\n";
    output += ">  Software may be potentially hazardous. Explore at your own risk.\n";
    output += ">\n";

    // ASCII cat (no indent)
    let cat = [
        r"                          ,",
        r"  ,-.       _,---._ __  / \",
        r" /  )    .-'       `./ /   \",
        r"(  (   ,'            `/    /|",
        r" \  `-'             \'\   / |",
        r"  `.              ,  \ \ /  |",
        r"   /`.          ,'-`----Y   |",
        r"  (            ;        |   '",
        r"  |  ,-.    ,-'         |  /",
        r"  |  | (   |            | /",
        r"  )  |  \  `.___________|/",
        r"  `--'   `--'",
    ];


    // Build language bars
    let lang_bars: Vec<String> = top_languages.iter().take(6).map(|(lang, pct)| {
        format!("{:<6} {} {:>5.1}%", lang, create_ascii_bar(*pct, 10), pct)
    }).collect();


    // Sitting cat ASCII (tail aligned diagonally)
    let cat2 = [
        r"  /\___/\",
        r"  )     (",
        r"  =\   /=",
        r"  )   (",
        r" /     \",
        r" )     (",
        r"/       \",
        r"\       /",
        r" \__ __/",
        r"    ))",
        r"    //",
        r"    ((",
        r"\)",
    ];

    // Cat left | Followers/Stars/Langs right | Sitting cat far right
    output += &format!("> {:<32}┌ Followers ┐ ┌ Stars ────┐  {}\n", cat[0], cat2[0]);
    output += &format!("> {:<32}│ {:^10}│ │ {:^10}│  {}\n", cat[1], github_followers, github_stars, cat2[1]);
    output += &format!("> {:<32}└───────────┘ └───────────┘  {}\n", cat[2], cat2[2]);
    output += &format!("> {:<32}┌ Languages ───────────────┐  {}\n", cat[3], cat2[3]);
    output += &format!("> {:<32}│ {:<25}│  {}\n", cat[4], lang_bars.get(0).unwrap_or(&String::new()), cat2[4]);
    output += &format!("> {:<32}│ {:<25}│  {}\n", cat[5], lang_bars.get(1).unwrap_or(&String::new()), cat2[5]);
    output += &format!("> {:<32}│ {:<25}│  {}\n", cat[6], lang_bars.get(2).unwrap_or(&String::new()), cat2[6]);
    output += &format!("> {:<32}│ {:<25}│  {}\n", cat[7], lang_bars.get(3).unwrap_or(&String::new()), cat2[7]);
    output += &format!("> {:<32}│ {:<25}│  {}\n", cat[8], lang_bars.get(4).unwrap_or(&String::new()), cat2[8]);
    output += &format!("> {:<32}│ {:<25}│  {}\n", cat[9], lang_bars.get(5).unwrap_or(&String::new()), cat2[9]);
    output += &format!("> {:<32}└──────────────────────────┘ {}\n", cat[10], cat2[10]);
    output += &format!("> {:<60}{}\n", cat[11], cat2[11]);

    // Activity (left) and Stats (right) side by side
    let act: Vec<String> = activities.iter().take(3).map(|a| format_activity(a)).collect();
    let commits = github_stats["total_commits"].as_u64().unwrap_or(0);
    let prs = github_stats["total_prs"].as_u64().unwrap_or(0);
    let issues = github_stats["total_issues"].as_u64().unwrap_or(0);
    let repos = github_stats["repos_owned"].as_u64().unwrap_or(0);
    let contrib = github_stats["contributed_to"].as_u64().unwrap_or(0);

    // Activity (43 chars) and Stats (21 chars) boxes
    output += &format!("> ┌ Activity ──────────────────────────────┐ ┌ Stats ────────────┐ {}\n", cat2[12]);
    output += &format!("> │ {:<39}│ │ {:<17} │\n", act.get(0).unwrap_or(&String::new()), format!("Cmt{:>3} Iss{:>3}", commits, issues));
    output += &format!("> │ {:<39}│ │ {:<17} │\n", act.get(1).unwrap_or(&String::new()), format!("PRs{:>3} Rpo{:>3}", prs, repos));
    output += &format!("> │ {:<39}│ │ {:<17} │\n", act.get(2).unwrap_or(&String::new()), format!("Contrib   {:>4}", contrib));
    output += "> └────────────────────────────────────────┘ └───────────────────┘\n";
    output += "> ```";

    let mut file = File::create("README.md").expect("Failed to create README.md");
    file.write_all(output.as_bytes())
        .expect("Failed to write to README.md");

    println!("✅ README.md has been updated successfully.");
    Ok(())
}

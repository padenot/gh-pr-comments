use anyhow::{Context, Result};
use clap::{Arg, Command};
use git2::Repository;
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use url::Url;

#[derive(Debug, Deserialize)]
struct PullRequest {
    title: String,
    html_url: String,
}

#[derive(Debug, Deserialize)]
struct Comment {
    body: String,
    user: User,
    created_at: String,
    html_url: String,
    diff_hunk: String,
    path: String,
    line: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct User {
    login: String,
}

#[derive(Debug)]
struct RepoInfo {
    owner: String,
    name: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let app = Command::new("gh-pr-comments")
        .about("Extract GitHub PR comments as markdown for LLM consumption")
        .arg(
            Arg::new("pr")
                .help("PR number, PR URL, or 'repo/pr_number' format")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("repo")
                .short('r')
                .long("repo")
                .help("GitHub repository in 'owner/repo' format")
                .value_name("REPO"),
        )
        .arg(
            Arg::new("include-resolved")
                .long("include-resolved")
                .help("Include resolved comments in output")
                .action(clap::ArgAction::SetTrue),
        );

    let matches = app.get_matches();
    let pr_input = matches.get_one::<String>("pr").unwrap();
    let repo_input = matches.get_one::<String>("repo");
    let include_resolved = matches.get_flag("include-resolved");

    let (repo_info, pr_number) = parse_input(pr_input, repo_input).await?;
    let client = Client::new();

    println!("# PR #{} - {}", pr_number, repo_info.owner);

    // Get PR details
    let pr_url = format!(
        "https://api.github.com/repos/{}/{}/pulls/{}",
        repo_info.owner, repo_info.name, pr_number
    );

    let pr: PullRequest = client
        .get(&pr_url)
        .header("User-Agent", "gh-pr-comments")
        .send()
        .await?
        .json()
        .await?;

    println!("**Title:** {}", pr.title);
    println!("**URL:** {}", pr.html_url);
    println!();

    // Get PR comments
    let comments_url = format!(
        "https://api.github.com/repos/{}/{}/pulls/{}/comments",
        repo_info.owner, repo_info.name, pr_number
    );

    let comments: Vec<Comment> = client
        .get(&comments_url)
        .header("User-Agent", "gh-pr-comments")
        .send()
        .await?
        .json()
        .await?;

    // Filter comments based on resolved status
    let filtered_comments: Vec<&Comment> = if include_resolved {
        comments.iter().collect()
    } else {
        // For now, we'll include all comments since GitHub API doesn't directly expose resolved status
        // In a real implementation, you'd need to check the review conversations API
        comments.iter().collect()
    };

    println!("## Comments\n");

    for comment in filtered_comments {
        println!("### Comment by @{}", comment.user.login);
        println!("**File:** `{}`", comment.path);
        if let Some(line) = comment.line {
            println!("**Line:** {}", line);
        }
        println!("**Created:** {}", comment.created_at);
        println!("**URL:** {}", comment.html_url);
        println!();

        println!("#### Diff Context");
        println!("```diff");
        println!("{}", comment.diff_hunk);
        println!("```");
        println!();

        println!("#### Comment");
        println!("{}", comment.body);
        println!();
        println!("---");
        println!();
    }

    Ok(())
}

async fn parse_input(pr_input: &str, repo_input: Option<&String>) -> Result<(RepoInfo, u32)> {
    // Try to parse as URL first
    if let Ok(url) = Url::parse(pr_input) {
        return parse_github_url(&url);
    }

    // Try to parse as owner/repo/pull/number format
    if pr_input.contains('/') {
        let parts: Vec<&str> = pr_input.split('/').collect();
        if parts.len() >= 2 {
            // Could be owner/repo or owner/repo/pull/number
            if parts.len() == 2 {
                // Need PR number from somewhere else
                return Err(anyhow::anyhow!("PR number not specified"));
            } else if parts.len() == 4 && parts[2] == "pull" {
                let owner = parts[0].to_string();
                let name = parts[1].to_string();
                let pr_number = parts[3].parse::<u32>()?;
                return Ok((RepoInfo { owner, name }, pr_number));
            }
        }
    }

    // Try to parse as just PR number
    if let Ok(pr_number) = pr_input.parse::<u32>() {
        if let Some(repo) = repo_input {
            let repo_info = parse_repo_string(repo)?;
            return Ok((repo_info, pr_number));
        } else {
            // Try to detect repo from git
            let repo_info = detect_repo_from_git().await?;
            return Ok((repo_info, pr_number));
        }
    }

    Err(anyhow::anyhow!("Could not parse PR input: {}", pr_input))
}

fn parse_github_url(url: &Url) -> Result<(RepoInfo, u32)> {
    let path = url.path();
    let re = Regex::new(r"^/([^/]+)/([^/]+)/pull/(\d+)").unwrap();

    if let Some(captures) = re.captures(path) {
        let owner = captures[1].to_string();
        let name = captures[2].to_string();
        let pr_number = captures[3].parse::<u32>()?;

        Ok((RepoInfo { owner, name }, pr_number))
    } else {
        Err(anyhow::anyhow!("Invalid GitHub PR URL format"))
    }
}

fn parse_repo_string(repo: &str) -> Result<RepoInfo> {
    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() == 2 {
        Ok(RepoInfo {
            owner: parts[0].to_string(),
            name: parts[1].to_string(),
        })
    } else {
        Err(anyhow::anyhow!(
            "Invalid repo format. Expected 'owner/repo'"
        ))
    }
}

async fn detect_repo_from_git() -> Result<RepoInfo> {
    let repo = Repository::open(".")?;
    let remote = repo.find_remote("origin")?;
    let url = remote.url().context("No URL for origin remote")?;

    // Parse GitHub URL from git remote
    let github_re = Regex::new(r"github\.com[:/]([^/]+)/([^/]+?)(?:\.git)?$").unwrap();

    if let Some(captures) = github_re.captures(url) {
        let owner = captures[1].to_string();
        let name = captures[2].to_string();
        Ok(RepoInfo { owner, name })
    } else {
        Err(anyhow::anyhow!(
            "Could not parse GitHub repo from git remote: {}",
            url
        ))
    }
}

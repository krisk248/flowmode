/// Title Parser - Extract meaningful context from window titles
///
/// Parses application window titles to extract useful information like:
/// - Teams: Chat partner, call participant, channel name
/// - Terminal: Project folder, current directory
/// - Browser: Website, page title

use regex::Regex;
use std::sync::LazyLock;

/// Parsed title with context
#[derive(Debug, Clone)]
pub struct ParsedTitle {
    pub display: String,      // Cleaned display title
    pub context_type: String, // "call", "chat", "project", "website", etc.
    pub context: String,      // Extracted context (person name, project, etc.)
}

// Pre-compiled regexes for performance
static TEAMS_CALL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:\(\d+\)\s*)?(?:Call|Meeting)\s*(?:with\s+)?(?:\|\s*)?(.+?)\s*(?:\||$)").unwrap()
});

static TEAMS_CHAT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:\(\d+\)\s*)?Chat\s*\|\s*(.+?)\s*\|\s*Microsoft Teams").unwrap()
});

static TEAMS_CHANNEL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:\(\d+\)\s*)?(.+?)\s*\|\s*Microsoft Teams").unwrap()
});

static PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|/)([^/]+)$").unwrap()
});

static BROWSER_SITE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(.+?)\s*[-–—]\s*(.+?)(?:\s*[-–—]\s*(?:Brave|Chrome|Firefox|Edge))?$").unwrap()
});

/// Parse a Teams window title
pub fn parse_teams_title(title: &str) -> ParsedTitle {
    // Check for call/meeting
    if let Some(caps) = TEAMS_CALL_RE.captures(title) {
        let person = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("Unknown");
        return ParsedTitle {
            display: format!("Call: {}", truncate(person, 30)),
            context_type: "call".to_string(),
            context: person.to_string(),
        };
    }

    // Check for chat
    if let Some(caps) = TEAMS_CHAT_RE.captures(title) {
        let person = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("Unknown");
        return ParsedTitle {
            display: format!("Chat: {}", truncate(person, 30)),
            context_type: "chat".to_string(),
            context: person.to_string(),
        };
    }

    // Check for channel/general Teams
    if let Some(caps) = TEAMS_CHANNEL_RE.captures(title) {
        let channel = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("Teams");
        // Skip if it's just "Microsoft Teams"
        if channel.to_lowercase() == "microsoft teams" || channel.is_empty() {
            return ParsedTitle {
                display: "Teams".to_string(),
                context_type: "app".to_string(),
                context: "Microsoft Teams".to_string(),
            };
        }
        return ParsedTitle {
            display: truncate(channel, 40),
            context_type: "channel".to_string(),
            context: channel.to_string(),
        };
    }

    // Fallback
    ParsedTitle {
        display: truncate(title, 40),
        context_type: "app".to_string(),
        context: title.to_string(),
    }
}

/// Parse a terminal window title (Ghostty, Konsole, etc.)
pub fn parse_terminal_title(title: &str) -> ParsedTitle {
    let cleaned = title.trim();

    // Check for common patterns
    // Pattern: "✱ Project Name" (dirty buffer indicator)
    let cleaned = cleaned.trim_start_matches(['✱', '*', '●', '○', '◉']).trim();

    // Check for path pattern (~/Projects/Office/Something)
    if cleaned.starts_with('~') || cleaned.starts_with('/') {
        // Extract last folder name
        if let Some(caps) = PATH_RE.captures(cleaned) {
            let folder = caps.get(1).map(|m| m.as_str()).unwrap_or(cleaned);
            return ParsedTitle {
                display: format!("Folder: {}", folder),
                context_type: "folder".to_string(),
                context: folder.to_string(),
            };
        }
    }

    // Check for "user@host: path" pattern
    if cleaned.contains('@') && cleaned.contains(':') {
        if let Some(path_start) = cleaned.find(':') {
            let path = cleaned[path_start + 1..].trim();
            if let Some(caps) = PATH_RE.captures(path) {
                let folder = caps.get(1).map(|m| m.as_str()).unwrap_or(path);
                return ParsedTitle {
                    display: format!("Folder: {}", folder),
                    context_type: "folder".to_string(),
                    context: folder.to_string(),
                };
            }
        }
    }

    // Check for editor patterns (vim, nvim, etc.)
    if cleaned.starts_with("nvim ") || cleaned.starts_with("vim ") {
        let file = cleaned.split_whitespace().nth(1).unwrap_or("");
        if let Some(caps) = PATH_RE.captures(file) {
            let filename = caps.get(1).map(|m| m.as_str()).unwrap_or(file);
            return ParsedTitle {
                display: format!("Editing: {}", filename),
                context_type: "file".to_string(),
                context: filename.to_string(),
            };
        }
    }

    // Fallback - use title as project name
    ParsedTitle {
        display: truncate(cleaned, 40),
        context_type: "terminal".to_string(),
        context: cleaned.to_string(),
    }
}

/// Parse a browser window title
pub fn parse_browser_title(title: &str) -> ParsedTitle {
    let cleaned = title.trim();

    // Remove browser suffix (Brave, Chrome, etc.)
    let cleaned = cleaned
        .trim_end_matches(" - Brave")
        .trim_end_matches(" - Google Chrome")
        .trim_end_matches(" - Firefox")
        .trim_end_matches(" - Microsoft Edge")
        .trim();

    // Detect common sites
    let lower = cleaned.to_lowercase();

    // YouTube
    if lower.contains("youtube") {
        // Strip notification counter like "(5) " from start
        let video_title = cleaned
            .trim_start_matches(|c: char| c == '(' || c.is_ascii_digit() || c == ')' || c == ' ')
            .replace("YouTube", "")
            .replace("- YouTube", "")
            .trim()
            .to_string();
        if video_title.is_empty() || video_title == "-" {
            return ParsedTitle {
                display: "YouTube".to_string(),
                context_type: "website".to_string(),
                context: "youtube.com".to_string(),
            };
        }
        return ParsedTitle {
            display: format!("YT: {}", truncate(&video_title, 35)),
            context_type: "video".to_string(),
            context: video_title,
        };
    }

    // GitHub
    if lower.contains("github") {
        return ParsedTitle {
            display: format!("GitHub: {}", truncate(cleaned, 30)),
            context_type: "code".to_string(),
            context: cleaned.to_string(),
        };
    }

    // Stack Overflow
    if lower.contains("stack overflow") {
        let question = cleaned.replace(" - Stack Overflow", "");
        return ParsedTitle {
            display: format!("SO: {}", truncate(&question, 35)),
            context_type: "research".to_string(),
            context: question,
        };
    }

    // Gmail/Email
    if lower.contains("gmail") || lower.contains("inbox") || lower.contains("mail") {
        return ParsedTitle {
            display: "Email".to_string(),
            context_type: "email".to_string(),
            context: cleaned.to_string(),
        };
    }

    // ChatGPT / Claude
    if lower.contains("chatgpt") || lower.contains("claude.ai") {
        return ParsedTitle {
            display: "AI Assistant".to_string(),
            context_type: "ai".to_string(),
            context: cleaned.to_string(),
        };
    }

    // Docs / Sheets / Office
    if lower.contains("docs.google") || lower.contains("sheets.google") || lower.contains("slides.google") {
        return ParsedTitle {
            display: format!("Docs: {}", truncate(cleaned, 30)),
            context_type: "document".to_string(),
            context: cleaned.to_string(),
        };
    }

    // Generic - try to extract site name
    if let Some(caps) = BROWSER_SITE_RE.captures(cleaned) {
        let site = caps.get(2).map(|m| m.as_str()).unwrap_or(cleaned);
        let page = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        return ParsedTitle {
            display: truncate(page, 40),
            context_type: "website".to_string(),
            context: site.to_string(),
        };
    }

    // Fallback
    ParsedTitle {
        display: truncate(cleaned, 40),
        context_type: "website".to_string(),
        context: cleaned.to_string(),
    }
}

/// Parse any window title based on app category
pub fn parse_title(app_name: &str, category: &str, title: &str) -> ParsedTitle {
    match category.to_lowercase().as_str() {
        "communication" => {
            if app_name.to_lowercase().contains("teams") {
                parse_teams_title(title)
            } else {
                ParsedTitle {
                    display: truncate(title, 40),
                    context_type: "communication".to_string(),
                    context: title.to_string(),
                }
            }
        }
        "terminal" => parse_terminal_title(title),
        "browser" => parse_browser_title(title),
        _ => ParsedTitle {
            display: truncate(title, 40),
            context_type: category.to_lowercase(),
            context: title.to_string(),
        },
    }
}

/// Truncate string to max length, adding ellipsis if needed
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_teams_chat() {
        let title = "(2) Chat | Syed Owais Ahmed | Microsoft Teams";
        let parsed = parse_teams_title(title);
        assert_eq!(parsed.context_type, "chat");
        assert!(parsed.display.contains("Syed Owais Ahmed"));
    }

    #[test]
    fn test_parse_teams_call() {
        let title = "Call with John Doe | Microsoft Teams";
        let parsed = parse_teams_title(title);
        assert_eq!(parsed.context_type, "call");
        assert!(parsed.display.contains("John Doe"));
    }

    #[test]
    fn test_parse_terminal_path() {
        let title = "~/Projects/Office/FlowMode";
        let parsed = parse_terminal_title(title);
        assert_eq!(parsed.context_type, "folder");
        assert!(parsed.display.contains("FlowMode"));
    }

    #[test]
    fn test_parse_browser_youtube() {
        let title = "Amazing Video - YouTube - Brave";
        let parsed = parse_browser_title(title);
        assert_eq!(parsed.context_type, "video");
        assert!(parsed.display.starts_with("YT:"));
    }
}

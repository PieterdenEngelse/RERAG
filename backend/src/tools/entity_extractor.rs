// src/tools/entity_extractor.rs
// Feature #15: EntityExtractorTool - Extract named entities from text

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::tools::{Tool, ToolMetadata, ToolResult, ToolType};

/// Entity type
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityType {
    Person,
    Organization,
    Location,
    Date,
    Time,
    Money,
    Percentage,
    Email,
    Url,
    PhoneNumber,
    Product,
    Event,
    Technology,
}

impl EntityType {
    pub fn label(&self) -> &'static str {
        match self {
            EntityType::Person => "PERSON",
            EntityType::Organization => "ORG",
            EntityType::Location => "LOC",
            EntityType::Date => "DATE",
            EntityType::Time => "TIME",
            EntityType::Money => "MONEY",
            EntityType::Percentage => "PERCENT",
            EntityType::Email => "EMAIL",
            EntityType::Url => "URL",
            EntityType::PhoneNumber => "PHONE",
            EntityType::Product => "PRODUCT",
            EntityType::Event => "EVENT",
            EntityType::Technology => "TECH",
        }
    }
}

/// Extracted entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub text: String,
    pub entity_type: EntityType,
    pub start: usize,
    pub end: usize,
    pub confidence: f32,
}

/// Entity extraction result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub entities: Vec<Entity>,
    pub entity_counts: HashMap<String, usize>,
}

pub struct EntityExtractorTool {
    success_rate: f32,
    // Known entities for pattern matching
    known_orgs: HashSet<String>,
    known_locations: HashSet<String>,
    known_tech: HashSet<String>,
    title_prefixes: HashSet<String>,
}

impl EntityExtractorTool {
    pub fn new() -> Self {
        let known_orgs: HashSet<String> = [
            "google",
            "microsoft",
            "apple",
            "amazon",
            "facebook",
            "meta",
            "netflix",
            "twitter",
            "tesla",
            "spacex",
            "nvidia",
            "intel",
            "ibm",
            "oracle",
            "salesforce",
            "adobe",
            "cisco",
            "samsung",
            "sony",
            "lg",
            "huawei",
            "alibaba",
            "tencent",
            "baidu",
            "openai",
            "anthropic",
            "deepmind",
            "github",
            "gitlab",
            "nasa",
            "fbi",
            "cia",
            "nsa",
            "un",
            "nato",
            "who",
            "imf",
            "world bank",
            "european union",
            "eu",
            "usa",
            "uk",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let known_locations: HashSet<String> = [
            "new york",
            "los angeles",
            "chicago",
            "houston",
            "phoenix",
            "san francisco",
            "seattle",
            "boston",
            "miami",
            "denver",
            "london",
            "paris",
            "berlin",
            "tokyo",
            "beijing",
            "shanghai",
            "mumbai",
            "delhi",
            "sydney",
            "melbourne",
            "toronto",
            "vancouver",
            "california",
            "texas",
            "florida",
            "washington",
            "oregon",
            "united states",
            "united kingdom",
            "china",
            "japan",
            "india",
            "germany",
            "france",
            "italy",
            "spain",
            "canada",
            "australia",
            "brazil",
            "mexico",
            "russia",
            "south korea",
            "singapore",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let known_tech: HashSet<String> = [
            "rust",
            "python",
            "javascript",
            "typescript",
            "java",
            "c++",
            "go",
            "ruby",
            "php",
            "swift",
            "kotlin",
            "scala",
            "haskell",
            "react",
            "angular",
            "vue",
            "node.js",
            "django",
            "flask",
            "tensorflow",
            "pytorch",
            "keras",
            "scikit-learn",
            "pandas",
            "docker",
            "kubernetes",
            "aws",
            "azure",
            "gcp",
            "linux",
            "windows",
            "macos",
            "ios",
            "android",
            "postgresql",
            "mysql",
            "mongodb",
            "redis",
            "elasticsearch",
            "kafka",
            "rabbitmq",
            "git",
            "github",
            "gitlab",
            "jenkins",
            "terraform",
            "ansible",
            "chatgpt",
            "gpt-4",
            "claude",
            "llama",
            "bert",
            "transformer",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let title_prefixes: HashSet<String> = [
            "mr",
            "mrs",
            "ms",
            "dr",
            "prof",
            "sir",
            "lord",
            "lady",
            "president",
            "ceo",
            "cto",
            "cfo",
            "director",
            "manager",
            "senator",
            "governor",
            "mayor",
            "judge",
            "general",
            "captain",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        Self {
            success_rate: 0.9,
            known_orgs,
            known_locations,
            known_tech,
            title_prefixes,
        }
    }

    /// Extract entities from text
    pub fn extract(&self, text: &str) -> ExtractionResult {
        let mut entities = Vec::new();
        let lower_text = text.to_lowercase();

        // Extract emails
        entities.extend(self.extract_emails(text));

        // Extract URLs
        entities.extend(self.extract_urls(text));

        // Extract phone numbers
        entities.extend(self.extract_phones(text));

        // Extract dates
        entities.extend(self.extract_dates(text));

        // Extract money
        entities.extend(self.extract_money(text));

        // Extract percentages
        entities.extend(self.extract_percentages(text));

        // Extract known organizations
        for org in &self.known_orgs {
            if let Some(pos) = lower_text.find(org) {
                // Get original case from text
                let original = &text[pos..pos + org.len()];
                entities.push(Entity {
                    text: original.to_string(),
                    entity_type: EntityType::Organization,
                    start: pos,
                    end: pos + org.len(),
                    confidence: 0.85,
                });
            }
        }

        // Extract known locations
        for loc in &self.known_locations {
            if let Some(pos) = lower_text.find(loc) {
                let original = &text[pos..pos + loc.len()];
                entities.push(Entity {
                    text: original.to_string(),
                    entity_type: EntityType::Location,
                    start: pos,
                    end: pos + loc.len(),
                    confidence: 0.85,
                });
            }
        }

        // Extract known technologies
        for tech in &self.known_tech {
            if let Some(pos) = lower_text.find(tech) {
                let original = &text[pos..pos + tech.len()];
                entities.push(Entity {
                    text: original.to_string(),
                    entity_type: EntityType::Technology,
                    start: pos,
                    end: pos + tech.len(),
                    confidence: 0.9,
                });
            }
        }

        // Extract potential person names (capitalized words)
        entities.extend(self.extract_names(text));

        // Remove duplicates and overlapping entities
        entities = self.deduplicate_entities(entities);

        // Count entities by type
        let mut entity_counts: HashMap<String, usize> = HashMap::new();
        for entity in &entities {
            *entity_counts
                .entry(entity.entity_type.label().to_string())
                .or_insert(0) += 1;
        }

        ExtractionResult {
            entities,
            entity_counts,
        }
    }

    fn extract_emails(&self, text: &str) -> Vec<Entity> {
        let mut entities = Vec::new();
        // Simple email pattern
        for (i, _) in text.match_indices('@') {
            // Find start of email
            let start = text[..i]
                .rfind(|c: char| c.is_whitespace() || c == '<' || c == '(')
                .map(|p| p + 1)
                .unwrap_or(0);
            // Find end of email
            let end = text[i..]
                .find(|c: char| c.is_whitespace() || c == '>' || c == ')')
                .map(|p| i + p)
                .unwrap_or(text.len());

            let email = &text[start..end];
            if email.contains('.') && email.len() > 5 {
                entities.push(Entity {
                    text: email.to_string(),
                    entity_type: EntityType::Email,
                    start,
                    end,
                    confidence: 0.95,
                });
            }
        }
        entities
    }

    fn extract_urls(&self, text: &str) -> Vec<Entity> {
        let mut entities = Vec::new();
        for prefix in &["http://", "https://", "www."] {
            for (i, _) in text.match_indices(prefix) {
                let end = text[i..]
                    .find(|c: char| c.is_whitespace() || c == ')' || c == ']' || c == '>')
                    .map(|p| i + p)
                    .unwrap_or(text.len());
                let url = &text[i..end];
                entities.push(Entity {
                    text: url.to_string(),
                    entity_type: EntityType::Url,
                    start: i,
                    end,
                    confidence: 0.95,
                });
            }
        }
        entities
    }

    fn extract_phones(&self, text: &str) -> Vec<Entity> {
        let mut entities = Vec::new();
        // Look for phone-like patterns
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i].is_numeric() || chars[i] == '+' || chars[i] == '(' {
                let start = i;
                let mut digits = 0;
                let mut j = i;
                while j < chars.len()
                    && (chars[j].is_numeric()
                        || chars[j] == '-'
                        || chars[j] == ' '
                        || chars[j] == '('
                        || chars[j] == ')'
                        || chars[j] == '+')
                {
                    if chars[j].is_numeric() {
                        digits += 1;
                    }
                    j += 1;
                }
                if digits >= 10 && digits <= 15 {
                    let phone: String = chars[start..j].iter().collect();
                    entities.push(Entity {
                        text: phone.trim().to_string(),
                        entity_type: EntityType::PhoneNumber,
                        start,
                        end: j,
                        confidence: 0.8,
                    });
                }
                i = j;
            } else {
                i += 1;
            }
        }
        entities
    }

    fn extract_dates(&self, text: &str) -> Vec<Entity> {
        let mut entities = Vec::new();
        let lower = text.to_lowercase();

        // Month names
        let months = [
            "january",
            "february",
            "march",
            "april",
            "may",
            "june",
            "july",
            "august",
            "september",
            "october",
            "november",
            "december",
            "jan",
            "feb",
            "mar",
            "apr",
            "jun",
            "jul",
            "aug",
            "sep",
            "oct",
            "nov",
            "dec",
        ];

        for month in months {
            if let Some(pos) = lower.find(month) {
                // Look for day/year around the month
                let start = pos.saturating_sub(5);
                let end = (pos + month.len() + 10).min(text.len());
                let context = &text[start..end];
                entities.push(Entity {
                    text: context.trim().to_string(),
                    entity_type: EntityType::Date,
                    start,
                    end,
                    confidence: 0.75,
                });
            }
        }
        entities
    }

    fn extract_money(&self, text: &str) -> Vec<Entity> {
        let mut entities = Vec::new();
        let chars: Vec<char> = text.chars().collect();

        for (i, &c) in chars.iter().enumerate() {
            if c == '$' || c == '€' || c == '£' || c == '¥' {
                let start = i;
                let mut j = i + 1;
                while j < chars.len()
                    && (chars[j].is_numeric() || chars[j] == ',' || chars[j] == '.')
                {
                    j += 1;
                }
                // Check for million/billion suffix
                let suffix_start = j;
                while j < chars.len() && chars[j].is_alphabetic() {
                    j += 1;
                }
                let suffix: String = chars[suffix_start..j].iter().collect();
                if ["million", "billion", "trillion", "m", "b", "k"]
                    .contains(&suffix.to_lowercase().as_str())
                {
                    // Include suffix
                } else {
                    j = suffix_start;
                }

                if j > start + 1 {
                    let money: String = chars[start..j].iter().collect();
                    entities.push(Entity {
                        text: money,
                        entity_type: EntityType::Money,
                        start,
                        end: j,
                        confidence: 0.9,
                    });
                }
            }
        }
        entities
    }

    fn extract_percentages(&self, text: &str) -> Vec<Entity> {
        let mut entities = Vec::new();
        for (i, _) in text.match_indices('%') {
            // Look backwards for number
            let start = text[..i]
                .rfind(|c: char| !c.is_numeric() && c != '.' && c != ',')
                .map(|p| p + 1)
                .unwrap_or(0);
            if start < i {
                let pct = &text[start..=i];
                entities.push(Entity {
                    text: pct.to_string(),
                    entity_type: EntityType::Percentage,
                    start,
                    end: i + 1,
                    confidence: 0.95,
                });
            }
        }
        entities
    }

    fn extract_names(&self, text: &str) -> Vec<Entity> {
        let mut entities = Vec::new();
        let words: Vec<&str> = text.split_whitespace().collect();

        let mut i = 0;
        while i < words.len() {
            let word = words[i];
            let lower = word.to_lowercase();

            // Check for title prefix
            let has_title = self.title_prefixes.contains(lower.trim_end_matches('.'));

            // Check if word starts with capital (potential name)
            if word
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
            {
                // Look for consecutive capitalized words
                let start = i;
                let mut j = i;
                while j < words.len() {
                    let w = words[j];
                    if w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                        && !self.known_orgs.contains(&w.to_lowercase())
                        && !self.known_locations.contains(&w.to_lowercase())
                        && !self.known_tech.contains(&w.to_lowercase())
                    {
                        j += 1;
                    } else {
                        break;
                    }
                }

                // If we have 2-4 consecutive capitalized words, likely a name
                let name_len = j - start;
                if name_len >= 2 && name_len <= 4 {
                    let name: String = words[start..j].join(" ");
                    // Find position in original text
                    if let Some(pos) = text.find(&name) {
                        entities.push(Entity {
                            text: name.clone(),
                            entity_type: EntityType::Person,
                            start: pos,
                            end: pos + name.len(),
                            confidence: if has_title { 0.9 } else { 0.7 },
                        });
                    }
                }
                i = j;
            } else {
                i += 1;
            }
        }
        entities
    }

    fn deduplicate_entities(&self, mut entities: Vec<Entity>) -> Vec<Entity> {
        // Sort by start position
        entities.sort_by_key(|e| e.start);

        let mut result: Vec<Entity> = Vec::new();
        for entity in entities {
            // Check if this entity overlaps with the last one
            if let Some(last) = result.last() {
                if entity.start < last.end {
                    // Overlapping - keep the one with higher confidence
                    if entity.confidence > last.confidence {
                        result.pop();
                        result.push(entity);
                    }
                    continue;
                }
            }
            result.push(entity);
        }
        result
    }
}

impl Default for EntityExtractorTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for EntityExtractorTool {
    fn tool_type(&self) -> ToolType {
        ToolType::EntityExtractor
    }

    fn description(&self) -> String {
        "Extract named entities from text including people, organizations, locations, dates, emails, URLs, and more.".to_string()
    }

    fn success_rate(&self) -> f32 {
        self.success_rate
    }

    async fn execute(&self, query: &str) -> Result<ToolResult, String> {
        let start = Instant::now();

        if query.trim().is_empty() {
            return Err("Empty text provided for entity extraction".to_string());
        }

        let extraction = self.extract(query);
        let execution_time = start.elapsed().as_millis() as u64;

        let mut result = String::new();
        result.push_str(&format!("Found {} entities:\n", extraction.entities.len()));

        for entity in &extraction.entities {
            result.push_str(&format!(
                "  [{}] \"{}\" (confidence: {:.0}%)\n",
                entity.entity_type.label(),
                entity.text,
                entity.confidence * 100.0
            ));
        }

        if !extraction.entity_counts.is_empty() {
            result.push_str("\nSummary: ");
            let counts: Vec<String> = extraction
                .entity_counts
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect();
            result.push_str(&counts.join(", "));
        }

        let confidence = if extraction.entities.is_empty() {
            0.5
        } else {
            extraction
                .entities
                .iter()
                .map(|e| e.confidence)
                .sum::<f32>()
                / extraction.entities.len() as f32
        };

        Ok(ToolResult {
            tool: ToolType::EntityExtractor,
            success: true,
            result,
            metadata: ToolMetadata {
                execution_time_ms: execution_time,
                confidence,
                source: Some("rule-based-ner".to_string()),
                cost: Some(0.0),
            },
        })
    }

    fn update_success(&mut self, success: bool) {
        if success {
            self.success_rate = (self.success_rate * 0.95) + 0.05;
        } else {
            self.success_rate = self.success_rate * 0.95;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_extract_email() {
        let tool = EntityExtractorTool::new();
        let result = tool
            .execute("Contact us at support@example.com for help.")
            .await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.result.contains("EMAIL"));
        assert!(r.result.contains("support@example.com"));
    }

    #[tokio::test]
    async fn test_extract_organization() {
        let tool = EntityExtractorTool::new();
        let result = tool.execute("Google announced a new product today.").await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.result.contains("ORG"));
    }

    #[tokio::test]
    async fn test_extract_money() {
        let tool = EntityExtractorTool::new();
        let result = tool
            .execute("The company raised $50 million in funding.")
            .await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.result.contains("MONEY"));
    }

    #[tokio::test]
    async fn test_extract_url() {
        let tool = EntityExtractorTool::new();
        let result = tool
            .execute("Visit https://example.com for more info.")
            .await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.result.contains("URL"));
    }
}

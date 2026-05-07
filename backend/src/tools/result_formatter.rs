// src/tools/result_formatter.rs - PRODUCTION
// Phase 10: Intelligent Result Formatting

pub struct ResultFormatter;

impl ResultFormatter {
    /// Extract key data from tool results for next step
    pub fn extract_key_data(result: &str, next_intent: &str) -> String {
        match next_intent {
            "math" | "Math" | "calculator" => Self::extract_for_math(result),
            "web_search" | "WebSearch" | "websearch" => Self::extract_for_search(result),
            "count" | "Count" => Self::extract_count(result),
            _ => result.to_string(),
        }
    }

    /// Extract numbers from results for math operations
    fn extract_for_math(result: &str) -> String {
        // Look for "Found X" pattern first
        let parts: Vec<&str> = result.split_whitespace().collect();
        for i in 0..parts.len() {
            if parts[i].to_lowercase() == "found"
                && i + 1 < parts.len()
                && parts[i + 1].parse::<i32>().is_ok()
            {
                return parts[i + 1].to_string();
            }
        }

        // Look for any number in the string
        for word in result.split_whitespace() {
            if word.chars().all(|c| c.is_numeric()) {
                return word.to_string();
            }
        }

        result.to_string()
    }

    /// Extract text for search operations
    fn extract_for_search(result: &str) -> String {
        // Take first meaningful sentence
        if let Some(sentence) = result.split('.').next() {
            sentence.trim().to_string()
        } else {
            result.to_string()
        }
    }

    /// Extract count from results
    fn extract_count(result: &str) -> String {
        // Look for "Found X" pattern
        let parts: Vec<&str> = result.split_whitespace().collect();
        for i in 0..parts.len() {
            if parts[i].to_lowercase() == "found"
                && i + 1 < parts.len()
                && parts[i + 1].parse::<i32>().is_ok()
            {
                return parts[i + 1].to_string();
            }
        }

        // Look for any number
        for word in result.split_whitespace() {
            if word.chars().all(|c| c.is_numeric()) {
                return word.to_string();
            }
        }

        result.to_string()
    }

    /// Build next query incorporating previous result
    pub fn build_next_query(previous_result: &str, next_query: &str, next_intent: &str) -> String {
        match next_intent {
            "math" | "Math" | "calculator" => {
                // Convert "count them" to just the number
                if next_query.contains("count") {
                    previous_result.to_string()
                } else if next_query.contains("multiply") {
                    format!("{} * 2", previous_result)
                } else if next_query.contains("divide") {
                    format!("{} / 2", previous_result)
                } else if next_query.contains("add") {
                    format!("{} + 1", previous_result)
                } else {
                    previous_result.to_string()
                }
            }
            _ => next_query.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_for_math_from_search() {
        let result = "Web search results for 'papers': Found 5 relevant pages";
        let extracted = ResultFormatter::extract_for_math(result);
        assert_eq!(extracted, "5");
    }

    #[test]
    fn test_extract_count() {
        let result = "Found 42 papers in the database";
        let extracted = ResultFormatter::extract_count(result);
        assert_eq!(extracted, "42");
    }

    #[test]
    fn test_build_next_query_count() {
        let prev_result = "100";
        let next_query = "count them";
        let result = ResultFormatter::build_next_query(prev_result, next_query, "math");
        assert_eq!(result, "100");
    }

    #[test]
    fn test_build_next_query_multiply() {
        let prev_result = "15";
        let next_query = "multiply by 2";
        let result = ResultFormatter::build_next_query(prev_result, next_query, "math");
        assert_eq!(result, "15 * 2");
    }
}

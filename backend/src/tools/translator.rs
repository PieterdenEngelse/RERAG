// src/tools/translator.rs
// Feature #13: TranslatorTool - Translate text between languages

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

use crate::tools::{Tool, ToolMetadata, ToolResult, ToolType};

/// Supported languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    English,
    Spanish,
    French,
    German,
    Italian,
    Portuguese,
    Russian,
    Chinese,
    Japanese,
    Korean,
    Arabic,
    Hindi,
}

impl Language {
    pub fn code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Spanish => "es",
            Language::French => "fr",
            Language::German => "de",
            Language::Italian => "it",
            Language::Portuguese => "pt",
            Language::Russian => "ru",
            Language::Chinese => "zh",
            Language::Japanese => "ja",
            Language::Korean => "ko",
            Language::Arabic => "ar",
            Language::Hindi => "hi",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code.to_lowercase().as_str() {
            "en" | "english" => Some(Language::English),
            "es" | "spanish" => Some(Language::Spanish),
            "fr" | "french" => Some(Language::French),
            "de" | "german" => Some(Language::German),
            "it" | "italian" => Some(Language::Italian),
            "pt" | "portuguese" => Some(Language::Portuguese),
            "ru" | "russian" => Some(Language::Russian),
            "zh" | "chinese" => Some(Language::Chinese),
            "ja" | "japanese" => Some(Language::Japanese),
            "ko" | "korean" => Some(Language::Korean),
            "ar" | "arabic" => Some(Language::Arabic),
            "hi" | "hindi" => Some(Language::Hindi),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Spanish => "Spanish",
            Language::French => "French",
            Language::German => "German",
            Language::Italian => "Italian",
            Language::Portuguese => "Portuguese",
            Language::Russian => "Russian",
            Language::Chinese => "Chinese",
            Language::Japanese => "Japanese",
            Language::Korean => "Korean",
            Language::Arabic => "Arabic",
            Language::Hindi => "Hindi",
        }
    }
}

pub struct TranslatorTool {
    success_rate: f32,
    // Simple word-based translations for demo
    // In production, this would call an external API
    translations: HashMap<(Language, Language), HashMap<String, String>>,
}

impl TranslatorTool {
    pub fn new() -> Self {
        let mut translations = HashMap::new();

        // English to Spanish common words
        let mut en_es = HashMap::new();
        en_es.insert("hello".to_string(), "hola".to_string());
        en_es.insert("world".to_string(), "mundo".to_string());
        en_es.insert("good".to_string(), "bueno".to_string());
        en_es.insert("morning".to_string(), "mañana".to_string());
        en_es.insert("night".to_string(), "noche".to_string());
        en_es.insert("thank".to_string(), "gracias".to_string());
        en_es.insert("you".to_string(), "tú".to_string());
        en_es.insert("please".to_string(), "por favor".to_string());
        en_es.insert("yes".to_string(), "sí".to_string());
        en_es.insert("no".to_string(), "no".to_string());
        en_es.insert("the".to_string(), "el/la".to_string());
        en_es.insert("is".to_string(), "es".to_string());
        en_es.insert("are".to_string(), "son".to_string());
        en_es.insert("i".to_string(), "yo".to_string());
        en_es.insert("love".to_string(), "amor".to_string());
        translations.insert((Language::English, Language::Spanish), en_es);

        // English to French
        let mut en_fr = HashMap::new();
        en_fr.insert("hello".to_string(), "bonjour".to_string());
        en_fr.insert("world".to_string(), "monde".to_string());
        en_fr.insert("good".to_string(), "bon".to_string());
        en_fr.insert("morning".to_string(), "matin".to_string());
        en_fr.insert("night".to_string(), "nuit".to_string());
        en_fr.insert("thank".to_string(), "merci".to_string());
        en_fr.insert("you".to_string(), "vous".to_string());
        en_fr.insert("please".to_string(), "s'il vous plaît".to_string());
        en_fr.insert("yes".to_string(), "oui".to_string());
        en_fr.insert("no".to_string(), "non".to_string());
        en_fr.insert("the".to_string(), "le/la".to_string());
        en_fr.insert("is".to_string(), "est".to_string());
        en_fr.insert("love".to_string(), "amour".to_string());
        translations.insert((Language::English, Language::French), en_fr);

        // English to German
        let mut en_de = HashMap::new();
        en_de.insert("hello".to_string(), "hallo".to_string());
        en_de.insert("world".to_string(), "welt".to_string());
        en_de.insert("good".to_string(), "gut".to_string());
        en_de.insert("morning".to_string(), "morgen".to_string());
        en_de.insert("night".to_string(), "nacht".to_string());
        en_de.insert("thank".to_string(), "danke".to_string());
        en_de.insert("you".to_string(), "du".to_string());
        en_de.insert("please".to_string(), "bitte".to_string());
        en_de.insert("yes".to_string(), "ja".to_string());
        en_de.insert("no".to_string(), "nein".to_string());
        en_de.insert("the".to_string(), "der/die/das".to_string());
        en_de.insert("is".to_string(), "ist".to_string());
        en_de.insert("love".to_string(), "liebe".to_string());
        translations.insert((Language::English, Language::German), en_de);

        Self {
            success_rate: 0.9,
            translations,
        }
    }

    /// Parse translation request
    /// Format: "translate [text] to [language]" or "[text] -> [language]"
    fn parse_request(&self, query: &str) -> Option<(String, Language, Language)> {
        let q = query.to_lowercase();

        // Try "translate X to Y" format
        if q.starts_with("translate ") {
            let rest = &query[10..];
            if let Some(to_pos) = rest.to_lowercase().find(" to ") {
                let text = rest[..to_pos].trim().to_string();
                let lang_str = rest[to_pos + 4..].trim();
                if let Some(target) = Language::from_code(lang_str) {
                    return Some((text, Language::English, target));
                }
            }
        }

        // Try "X -> Y: text" format
        if let Some(_arrow_pos) = q.find("->") {
            let parts: Vec<&str> = query.split("->").collect();
            if parts.len() == 2 {
                let text = parts[0].trim().to_string();
                let lang_str = parts[1].trim();
                if let Some(target) = Language::from_code(lang_str) {
                    return Some((text, Language::English, target));
                }
            }
        }

        // Try "from X to Y: text" format
        if q.contains(" from ") && q.contains(" to ") {
            // Complex parsing - skip for now
        }

        // Default: assume English to Spanish
        Some((query.to_string(), Language::English, Language::Spanish))
    }

    /// Translate text
    fn translate(&self, text: &str, from: Language, to: Language) -> String {
        if from == to {
            return text.to_string();
        }

        let dict = self.translations.get(&(from, to));

        // Word-by-word translation (simple approach)
        let words: Vec<&str> = text.split_whitespace().collect();
        let translated: Vec<String> = words
            .iter()
            .map(|word| {
                let lower = word.to_lowercase();
                let clean = lower.trim_matches(|c: char| !c.is_alphabetic());

                if let Some(d) = dict {
                    if let Some(trans) = d.get(clean) {
                        // Preserve punctuation
                        let prefix: String =
                            word.chars().take_while(|c| !c.is_alphabetic()).collect();
                        let suffix: String = word
                            .chars()
                            .rev()
                            .take_while(|c| !c.is_alphabetic())
                            .collect::<String>()
                            .chars()
                            .rev()
                            .collect();
                        return format!("{}{}{}", prefix, trans, suffix);
                    }
                }

                // Keep original if no translation found
                word.to_string()
            })
            .collect();

        translated.join(" ")
    }
}

impl Default for TranslatorTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TranslatorTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Translator
    }

    fn description(&self) -> String {
        "Translate text between languages. Supports: English, Spanish, French, German, Italian, Portuguese, Russian, Chinese, Japanese, Korean, Arabic, Hindi.".to_string()
    }

    fn success_rate(&self) -> f32 {
        self.success_rate
    }

    async fn execute(&self, query: &str) -> Result<ToolResult, String> {
        let start = Instant::now();

        let (text, from, to) = self
            .parse_request(query)
            .ok_or_else(|| "Could not parse translation request".to_string())?;

        let translated = self.translate(&text, from, to);
        let execution_time = start.elapsed().as_millis() as u64;

        // Check if translation actually happened
        let confidence = if translated != text { 0.85 } else { 0.5 };

        Ok(ToolResult {
            tool: ToolType::Translator,
            success: true,
            result: format!(
                "Translation ({} → {}): {}",
                from.name(),
                to.name(),
                translated
            ),
            metadata: ToolMetadata {
                execution_time_ms: execution_time,
                confidence,
                source: Some(format!("local-dict-{}-{}", from.code(), to.code())),
                cost: Some(0.0), // Local translation is free
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
    async fn test_translate_to_spanish() {
        let tool = TranslatorTool::new();
        let result = tool.execute("translate hello world to spanish").await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.success);
        assert!(r.result.contains("hola"));
    }

    #[tokio::test]
    async fn test_translate_to_french() {
        let tool = TranslatorTool::new();
        let result = tool.execute("translate hello to french").await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.result.contains("bonjour"));
    }

    #[test]
    fn test_language_codes() {
        assert_eq!(Language::from_code("en"), Some(Language::English));
        assert_eq!(Language::from_code("spanish"), Some(Language::Spanish));
        assert_eq!(Language::from_code("fr"), Some(Language::French));
    }
}

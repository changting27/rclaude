//! Tool result caching and storage.
//! Caches tool results to avoid re-executing identical tool calls.

use std::collections::HashMap;

const MAX_CACHE_SIZE: usize = 100;
const MAX_RESULT_SIZE: usize = 100_000;

/// Cached tool result.
#[derive(Debug, Clone)]
struct CachedResult {
    result: String,
    is_error: bool,
    timestamp: std::time::Instant,
}

/// Tool result cache with LRU eviction.
#[derive(Debug)]
pub struct ToolResultCache {
    cache: HashMap<String, CachedResult>,
    order: Vec<String>,
}

impl ToolResultCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// Generate cache key from tool name + input.
    fn cache_key(tool_name: &str, input: &serde_json::Value) -> String {
        format!("{}:{}", tool_name, input)
    }

    /// Get a cached result if available and not expired.
    pub fn get(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
        max_age_secs: u64,
    ) -> Option<(&str, bool)> {
        let key = Self::cache_key(tool_name, input);
        let entry = self.cache.get(&key)?;
        if entry.timestamp.elapsed().as_secs() > max_age_secs {
            return None;
        }
        Some((&entry.result, entry.is_error))
    }

    /// Store a tool result in the cache.
    pub fn put(
        &mut self,
        tool_name: &str,
        input: &serde_json::Value,
        result: &str,
        is_error: bool,
    ) {
        if result.len() > MAX_RESULT_SIZE {
            return;
        } // Don't cache huge results

        let key = Self::cache_key(tool_name, input);

        // Evict oldest if at capacity
        while self.cache.len() >= MAX_CACHE_SIZE {
            if let Some(oldest) = self.order.first().cloned() {
                self.cache.remove(&oldest);
                self.order.remove(0);
            } else {
                break;
            }
        }

        self.cache.insert(
            key.clone(),
            CachedResult {
                result: result.to_string(),
                is_error,
                timestamp: std::time::Instant::now(),
            },
        );
        self.order.push(key);
    }

    /// Clear the cache.
    pub fn clear(&mut self) {
        self.cache.clear();
        self.order.clear();
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

impl Default for ToolResultCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_put_get() {
        let mut cache = ToolResultCache::new();
        let input = serde_json::json!({"file_path": "test.rs"});
        cache.put("Read", &input, "file content", false);
        let (result, is_error) = cache.get("Read", &input, 60).unwrap();
        assert_eq!(result, "file content");
        assert!(!is_error);
    }

    #[test]
    fn test_cache_miss() {
        let cache = ToolResultCache::new();
        let input = serde_json::json!({"file_path": "test.rs"});
        assert!(cache.get("Read", &input, 60).is_none());
    }

    #[test]
    fn test_cache_eviction() {
        let mut cache = ToolResultCache::new();
        for i in 0..150 {
            cache.put("Read", &serde_json::json!({"i": i}), "result", false);
        }
        assert!(cache.len() <= MAX_CACHE_SIZE);
    }
}

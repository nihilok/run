# Runfile Caching: Code Integration Example

**Date:** January 18, 2026  
**Context:** Example implementation of parse caching

---

## Module Structure

```
src/
├── cache.rs          ← NEW: Cache management
├── parser/
│   └── mod.rs        ← UPDATED: Add cached parsing
├── config.rs         ← UPDATED: Pass source path
└── executor.rs       ← UPDATED: Use cached parsing
```

---

## 1. New Module: `src/cache.rs`

```rust
//! Parse cache management for Runfile AST
//!
//! Caches parsed AST to avoid redundant parsing on repeated invocations.
//! Uses three-tier validation (mtime, size, content hash) to ensure correctness.

use crate::ast::Program;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Cache entry stored on disk
#[derive(Serialize, Deserialize)]
struct CacheEntry {
    version: String,
    runfile_path: PathBuf,
    runfile_mtime: u64,
    runfile_size: u64,
    runfile_hash: String,
    parsed_ast: Program,
    created_at: u64,
}

/// Get cache directory path
fn get_cache_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Try XDG_CACHE_HOME first
    if let Ok(xdg_cache) = std::env::var("XDG_CACHE_HOME") {
        return Ok(PathBuf::from(xdg_cache).join("run").join("parsed"));
    }
    
    // Try ~/.cache
    if let Some(home) = dirs::home_dir() {
        return Ok(home.join(".cache").join("run").join("parsed"));
    }
    
    // Fallback to temp directory
    let uid = unsafe { libc::getuid() };
    Ok(std::env::temp_dir()
        .join(format!("run-cache-{}", uid))
        .join("parsed"))
}

/// Generate cache key from Runfile path
fn cache_key(runfile_path: &Path) -> String {
    // Canonicalize to handle symlinks
    let canonical = runfile_path
        .canonicalize()
        .unwrap_or_else(|_| runfile_path.to_path_buf());
    
    let mut hasher = DefaultHasher::new();
    canonical.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Hash first N bytes of file for content validation
fn hash_file_prefix(path: &Path, size: usize) -> Result<String, std::io::Error> {
    use std::io::Read;
    
    let mut file = std::fs::File::open(path)?;
    let mut buffer = vec![0u8; size];
    let bytes_read = file.read(&mut buffer)?;
    
    let mut hasher = DefaultHasher::new();
    buffer[..bytes_read].hash(&mut hasher);
    Ok(format!("{:x}", hasher.finish()))
}

impl CacheEntry {
    /// Validate cache entry against current file
    fn is_valid(&self, current_path: &Path) -> Result<bool, std::io::Error> {
        let metadata = std::fs::metadata(current_path)?;
        
        // Check 1: Modification time
        let current_mtime = metadata
            .modified()?
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if current_mtime != self.runfile_mtime {
            return Ok(false);
        }
        
        // Check 2: File size
        if metadata.len() != self.runfile_size {
            return Ok(false);
        }
        
        // Check 3: Content hash (first 1KB)
        let current_hash = hash_file_prefix(current_path, 1024)?;
        if current_hash != self.runfile_hash {
            return Ok(false);
        }
        
        Ok(true)
    }
}

/// Write parsed AST to cache
pub fn write_cache(
    runfile_path: &Path,
    ast: &Program,
) -> Result<(), Box<dyn std::error::Error>> {
    // Don't cache if disabled
    if std::env::var("RUN_NO_CACHE").is_ok() {
        return Ok(());
    }
    
    let cache_dir = get_cache_dir()?;
    std::fs::create_dir_all(&cache_dir)?;
    
    let key = cache_key(runfile_path);
    let cache_path = cache_dir.join(format!("{}.msgpack", key));
    
    let metadata = std::fs::metadata(runfile_path)?;
    let mtime = metadata
        .modified()?
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs();
    
    let entry = CacheEntry {
        version: env!("CARGO_PKG_VERSION").to_string(),
        runfile_path: runfile_path.to_path_buf(),
        runfile_mtime: mtime,
        runfile_size: metadata.len(),
        runfile_hash: hash_file_prefix(runfile_path, 1024)?,
        parsed_ast: ast.clone(),
        created_at: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs(),
    };
    
    // Serialize with MessagePack
    let serialized = rmp_serde::to_vec(&entry)?;
    
    // Atomic write (write to temp, then rename)
    let temp_path = cache_path.with_extension("tmp");
    std::fs::write(&temp_path, serialized)?;
    std::fs::rename(temp_path, cache_path)?;
    
    // Cleanup old entries (lazy cleanup)
    let _ = cleanup_cache(&cache_dir);
    
    Ok(())
}

/// Read cached AST if valid
pub fn read_cache(
    runfile_path: &Path,
) -> Result<Option<Program>, Box<dyn std::error::Error>> {
    // Don't use cache if disabled
    if std::env::var("RUN_NO_CACHE").is_ok() {
        return Ok(None);
    }
    
    let cache_dir = get_cache_dir()?;
    let key = cache_key(runfile_path);
    let cache_path = cache_dir.join(format!("{}.msgpack", key));
    
    // Check if cache exists
    if !cache_path.exists() {
        if std::env::var("RUN_CACHE_DEBUG").is_ok() {
            eprintln!("[CACHE] Miss: {} (not found)", runfile_path.display());
        }
        return Ok(None);
    }
    
    // Read cache entry
    let data = std::fs::read(&cache_path)?;
    let entry: CacheEntry = match rmp_serde::from_slice(&data) {
        Ok(e) => e,
        Err(_) => {
            // Corrupt cache, delete and return None
            if std::env::var("RUN_CACHE_DEBUG").is_ok() {
                eprintln!("[CACHE] Miss: {} (corrupt)", runfile_path.display());
            }
            let _ = std::fs::remove_file(&cache_path);
            return Ok(None);
        }
    };
    
    // Validate version
    if entry.version != env!("CARGO_PKG_VERSION") {
        if std::env::var("RUN_CACHE_DEBUG").is_ok() {
            eprintln!("[CACHE] Miss: {} (version mismatch)", runfile_path.display());
        }
        let _ = std::fs::remove_file(&cache_path);
        return Ok(None);
    }
    
    // Validate against current file
    if !entry.is_valid(runfile_path)? {
        if std::env::var("RUN_CACHE_DEBUG").is_ok() {
            eprintln!("[CACHE] Miss: {} (invalidated)", runfile_path.display());
        }
        let _ = std::fs::remove_file(&cache_path);
        return Ok(None);
    }
    
    if std::env::var("RUN_CACHE_DEBUG").is_ok() {
        eprintln!("[CACHE] Hit: {}", runfile_path.display());
    }
    
    Ok(Some(entry.parsed_ast))
}

/// Clean up old cache entries
fn cleanup_cache(cache_dir: &Path) -> Result<(), std::io::Error> {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let max_age_secs = 7 * 24 * 60 * 60; // 7 days
    let max_entries = 100;
    
    let mut entries: Vec<_> = std::fs::read_dir(cache_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                == Some("msgpack")
        })
        .collect();
    
    // Delete old entries
    for entry in &entries {
        if let Ok(data) = std::fs::read(entry.path()) {
            if let Ok(cache_entry) = rmp_serde::from_slice::<CacheEntry>(&data) {
                if now - cache_entry.created_at > max_age_secs {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }
    
    // Enforce size limit
    entries.retain(|e| e.path().exists());
    if entries.len() > max_entries {
        entries.sort_by_key(|e| {
            e.metadata()
                .and_then(|m| m.accessed())
                .unwrap_or(SystemTime::UNIX_EPOCH)
        });
        
        let to_delete = entries.len() - max_entries;
        for entry in entries.iter().take(to_delete) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    
    Ok(())
}

/// Clear all cache entries
pub fn clear_cache() -> Result<(), Box<dyn std::error::Error>> {
    let cache_dir = get_cache_dir()?;
    if cache_dir.exists() {
        std::fs::remove_dir_all(&cache_dir)?;
    }
    Ok(())
}

/// Get cache statistics
pub fn cache_stats() -> Result<CacheStats, Box<dyn std::error::Error>> {
    let cache_dir = get_cache_dir()?;
    
    if !cache_dir.exists() {
        return Ok(CacheStats {
            location: cache_dir,
            entries: 0,
            total_size: 0,
            oldest: None,
            newest: None,
        });
    }
    
    let entries: Vec<_> = std::fs::read_dir(&cache_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                == Some("msgpack")
        })
        .collect();
    
    let total_size: u64 = entries
        .iter()
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum();
    
    let mut timestamps: Vec<u64> = entries
        .iter()
        .filter_map(|e| std::fs::read(e.path()).ok())
        .filter_map(|data| rmp_serde::from_slice::<CacheEntry>(&data).ok())
        .map(|entry| entry.created_at)
        .collect();
    
    timestamps.sort_unstable();
    
    Ok(CacheStats {
        location: cache_dir,
        entries: entries.len(),
        total_size,
        oldest: timestamps.first().copied(),
        newest: timestamps.last().copied(),
    })
}

pub struct CacheStats {
    pub location: PathBuf,
    pub entries: usize,
    pub total_size: u64,
    pub oldest: Option<u64>,
    pub newest: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    
    #[test]
    fn test_cache_key_consistent() {
        let path = Path::new("/tmp/Runfile");
        let key1 = cache_key(path);
        let key2 = cache_key(path);
        assert_eq!(key1, key2);
    }
    
    #[test]
    fn test_cache_invalidation_on_mtime() {
        let temp_dir = tempfile::tempdir().unwrap();
        let runfile_path = temp_dir.path().join("Runfile");
        
        std::fs::write(&runfile_path, "test() echo hello").unwrap();
        
        let ast = crate::parser::parse_script("test() echo hello").unwrap();
        write_cache(&runfile_path, &ast).unwrap();
        
        // Modify file
        thread::sleep(Duration::from_secs(2));
        std::fs::write(&runfile_path, "test() echo goodbye").unwrap();
        
        // Cache should be invalid
        let cached = read_cache(&runfile_path).unwrap();
        assert!(cached.is_none());
    }
}
```

---

## 2. Updated: `src/parser/mod.rs`

```rust
// ...existing code...

/// Parse a Run script with caching
///
/// First checks cache for valid parsed AST. If cache miss or invalid,
/// parses the script and updates cache.
///
/// # Errors
///
/// Returns `Err` if the input contains syntax errors that violate the grammar.
pub fn parse_script_cached(
    input: &str,
    source_path: Option<&Path>,
) -> Result<Program, Box<pest::error::Error<Rule>>> {
    // Try cache if source path available
    if let Some(path) = source_path {
        if let Ok(Some(cached)) = crate::cache::read_cache(path) {
            return Ok(cached);
        }
    }
    
    // Cache miss or no source path - parse normally
    let ast = parse_script(input)?;
    
    // Update cache (don't fail if cache write fails)
    if let Some(path) = source_path {
        let _ = crate::cache::write_cache(path, &ast);
    }
    
    Ok(ast)
}

// ...existing code...
```

---

## 3. Updated: `src/config.rs`

```rust
// ...existing code...

/// Load Runfile content and return both content and source path
pub fn load_config_with_path() -> Option<(String, PathBuf)> {
    if let Some(custom_path) = get_custom_runfile_path() {
        if custom_path.exists() {
            return std::fs::read_to_string(&custom_path)
                .ok()
                .map(|content| (content, custom_path));
        }
        return None;
    }

    // Try current directory and parents
    let current_dir = std::env::current_dir().ok()?;
    for ancestor in current_dir.ancestors() {
        let runfile_path = ancestor.join("Runfile");
        if runfile_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&runfile_path) {
                return Some((content, runfile_path));
            }
        }
    }

    // Try home directory
    if let Some(home) = dirs::home_dir() {
        let runfile_path = home.join(".runfile");
        if runfile_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&runfile_path) {
                return Some((content, runfile_path));
            }
        }
    }

    None
}

// ...existing code...
```

---

## 4. Updated: `src/executor.rs`

```rust
// ...existing code...

/// Execute a function from the Runfile
pub fn execute_function(function_name: &str, args: &[String]) {
    // Load config with path for caching
    let (config_content, runfile_path) = config::load_config_with_path()
        .unwrap_or_else(|| {
            eprintln!("Error: No Runfile found");
            std::process::exit(1);
        });

    // Parse with caching
    let program = match parser::parse_script_cached(&config_content, Some(&runfile_path)) {
        Ok(prog) => prog,
        Err(e) => {
            eprintln!("Error parsing Runfile: {}", e);
            std::process::exit(1);
        }
    };

    // ...existing execution code...
}

/// List all available functions from the Runfile
pub fn list_functions() {
    let (config_content, runfile_path) = config::load_config_with_path()
        .unwrap_or_else(|| {
            eprintln!("Error: No Runfile found");
            std::process::exit(1);
        });

    // Parse with caching
    let program = match parser::parse_script_cached(&config_content, Some(&runfile_path)) {
        Ok(prog) => prog,
        Err(e) => {
            eprintln!("Error parsing Runfile: {}", e);
            std::process::exit(1);
        }
    };

    // ...existing list code...
}

// ...existing code...
```

---

## 5. Updated: `src/main.rs`

Add CLI flags for cache management:

```rust
// ...existing code...

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // ...existing arg parsing...

    // Handle cache management flags
    if args.contains(&"--clear-cache".to_string()) {
        match crate::cache::clear_cache() {
            Ok(()) => {
                println!("Cache cleared successfully");
                // Continue with command if there are more args
                if args.len() == 2 {
                    std::process::exit(0);
                }
            }
            Err(e) => {
                eprintln!("Error clearing cache: {}", e);
                std::process::exit(1);
            }
        }
    }

    if args.contains(&"--cache-stats".to_string()) {
        match crate::cache::cache_stats() {
            Ok(stats) => {
                println!("Cache Statistics");
                println!("────────────────────────────────");
                println!("Location:     {}", stats.location.display());
                println!("Entries:      {}", stats.entries);
                println!("Total Size:   {:.2} MB", stats.total_size as f64 / 1_048_576.0);
                if let Some(oldest) = stats.oldest {
                    let age = (SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() - oldest) / 86400;
                    println!("Oldest:       {} days ago", age);
                }
                if let Some(newest) = stats.newest {
                    let age = (SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() - newest) / 60;
                    println!("Newest:       {} minutes ago", age);
                }
            }
            Err(e) => {
                eprintln!("Error getting cache stats: {}", e);
                std::process::exit(1);
            }
        }
        std::process::exit(0);
    }

    // ...existing code...
}
```

---

## 6. Updated: `Cargo.toml`

Add required dependencies:

```toml
[dependencies]
# ...existing dependencies...

# For cache serialization
rmp-serde = "1.1"

# For home directory detection
dirs = "5.0"
```

---

## 7. Updated: `src/lib.rs`

Add cache module:

```rust
pub mod ast;
pub mod cache;        // NEW
pub mod completion;
pub mod config;
pub mod executor;
pub mod interpreter;
pub mod mcp;
pub mod parser;
pub mod repl;
pub mod transpiler;
pub mod utils;
```

---

## 8. Updated: `src/ast.rs`

Make AST serializable:

```rust
use serde::{Deserialize, Serialize};  // Add this import

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]  // Add Serialize, Deserialize
pub struct Program {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]  // Add Serialize, Deserialize
pub enum Statement {
    // ...existing code...
}

// Add to all AST types: Serialize, Deserialize
```

---

## Usage Examples

### Normal Usage (Transparent)

```bash
# First run - cache miss
$ run test
[CACHE] Miss: /home/user/project/Runfile (not found)
Running tests...

# Second run - cache hit
$ run test
[CACHE] Hit: /home/user/project/Runfile
Running tests...  # Faster!
```

### Debugging Cache

```bash
# Enable cache debugging
$ export RUN_CACHE_DEBUG=1
$ run test
[CACHE] Hit: /home/user/project/Runfile
Running tests...

# Disable cache temporarily
$ run --no-cache test
# or
$ RUN_NO_CACHE=1 run test
```

### Cache Management

```bash
# Show cache statistics
$ run --cache-stats
Cache Statistics
────────────────────────────────
Location:     ~/.cache/run/parsed
Entries:      23
Total Size:   1.2 MB
Oldest:       2 days ago
Newest:       5 minutes ago

# Clear cache
$ run --clear-cache
Cache cleared successfully

# Clear cache and run command
$ run --clear-cache build
Cache cleared successfully
Building...
```

---

## Testing

```bash
# Run cache tests
$ cargo test cache

# Integration test with caching
$ cargo test --test integration_test test_cached_parsing

# Benchmark cache performance
$ cargo bench --bench parse_cache
```

---

*Integration example by GitHub Copilot Agent*  
*January 18, 2026*

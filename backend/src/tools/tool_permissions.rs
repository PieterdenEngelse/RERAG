// src/tools/tool_permissions.rs
// Feature #4: Tool permissions and roles system

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use tracing::{debug, warn};

/// Permission level for a tool
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ToolPermission {
    /// Tool is fully enabled
    #[default]
    Enabled,
    /// Tool is disabled
    Disabled,
    /// Tool requires approval before execution
    RequiresApproval,
}

/// Role with associated tool permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRole {
    pub name: String,
    pub description: String,
    /// Tool permissions for this role (tool_type -> permission)
    pub permissions: HashMap<String, ToolPermission>,
    /// Whether this role can execute tool chains
    pub can_chain: bool,
    /// Maximum chain length allowed
    pub max_chain_length: usize,
    /// Whether this role can execute tools in parallel
    pub can_parallel: bool,
}

impl Default for ToolRole {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            description: "Default role with standard permissions".to_string(),
            permissions: HashMap::new(),
            can_chain: true,
            max_chain_length: 10,
            can_parallel: true,
        }
    }
}

/// Predefined roles
impl ToolRole {
    /// Admin role - full access
    pub fn admin() -> Self {
        Self {
            name: "admin".to_string(),
            description: "Full access to all tools".to_string(),
            permissions: HashMap::new(), // Empty = all enabled
            can_chain: true,
            max_chain_length: 50,
            can_parallel: true,
        }
    }

    /// Standard user role
    pub fn user() -> Self {
        let mut permissions = HashMap::new();
        // Restrict dangerous tools
        permissions.insert(
            "CodeExecution".to_string(),
            ToolPermission::RequiresApproval,
        );
        permissions.insert(
            "DatabaseQuery".to_string(),
            ToolPermission::RequiresApproval,
        );

        Self {
            name: "user".to_string(),
            description: "Standard user with some restrictions".to_string(),
            permissions,
            can_chain: true,
            max_chain_length: 5,
            can_parallel: true,
        }
    }

    /// Read-only role
    pub fn readonly() -> Self {
        let mut permissions = HashMap::new();
        // Disable all write/execute tools
        permissions.insert("CodeExecution".to_string(), ToolPermission::Disabled);
        permissions.insert("DatabaseQuery".to_string(), ToolPermission::Disabled);
        permissions.insert("Notification".to_string(), ToolPermission::Disabled);
        permissions.insert("Scheduler".to_string(), ToolPermission::Disabled);
        permissions.insert("ImageGeneration".to_string(), ToolPermission::Disabled);

        Self {
            name: "readonly".to_string(),
            description: "Read-only access, no execution or side effects".to_string(),
            permissions,
            can_chain: true,
            max_chain_length: 3,
            can_parallel: false,
        }
    }

    /// Restricted role for untrusted contexts
    pub fn restricted() -> Self {
        let mut permissions = HashMap::new();
        // Only allow safe, local tools
        permissions.insert("CodeExecution".to_string(), ToolPermission::Disabled);
        permissions.insert("DatabaseQuery".to_string(), ToolPermission::Disabled);
        permissions.insert("Notification".to_string(), ToolPermission::Disabled);
        permissions.insert("Scheduler".to_string(), ToolPermission::Disabled);
        permissions.insert("ImageGeneration".to_string(), ToolPermission::Disabled);
        permissions.insert("WebSearch".to_string(), ToolPermission::Disabled);
        permissions.insert("URLFetch".to_string(), ToolPermission::Disabled);

        Self {
            name: "restricted".to_string(),
            description: "Highly restricted, only local safe tools".to_string(),
            permissions,
            can_chain: false,
            max_chain_length: 1,
            can_parallel: false,
        }
    }
}

/// API key to role mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyPermissions {
    pub api_key_hash: String,
    pub role_name: String,
    pub custom_permissions: Option<HashMap<String, ToolPermission>>,
    pub created_at: String,
    pub expires_at: Option<String>,
}

/// Global permissions state
struct PermissionsState {
    roles: HashMap<String, ToolRole>,
    api_key_roles: HashMap<String, String>, // api_key_hash -> role_name
    api_key_custom: HashMap<String, HashMap<String, ToolPermission>>,
    global_disabled: HashSet<String>, // Globally disabled tools
}

impl Default for PermissionsState {
    fn default() -> Self {
        let mut roles = HashMap::new();
        roles.insert("admin".to_string(), ToolRole::admin());
        roles.insert("user".to_string(), ToolRole::user());
        roles.insert("readonly".to_string(), ToolRole::readonly());
        roles.insert("restricted".to_string(), ToolRole::restricted());
        roles.insert("default".to_string(), ToolRole::default());

        Self {
            roles,
            api_key_roles: HashMap::new(),
            api_key_custom: HashMap::new(),
            global_disabled: HashSet::new(),
        }
    }
}

static PERMISSIONS: OnceLock<Mutex<PermissionsState>> = OnceLock::new();

fn get_state() -> &'static Mutex<PermissionsState> {
    PERMISSIONS.get_or_init(|| Mutex::new(PermissionsState::default()))
}

/// Hash an API key for storage
fn hash_api_key(api_key: &str) -> String {
    format!("{:x}", seahash::hash(api_key.as_bytes()))
}

/// Check if a tool is allowed for an API key
pub fn check_permission(api_key: Option<&str>, tool_type: &str) -> PermissionCheckResult {
    let state = match get_state().lock() {
        Ok(s) => s,
        Err(_) => return PermissionCheckResult::Allowed, // Fail open
    };

    // Check global disable first
    if state.global_disabled.contains(tool_type) {
        return PermissionCheckResult::Denied {
            reason: "Tool is globally disabled".to_string(),
        };
    }

    // Get role for API key
    let role_name = if let Some(key) = api_key {
        let key_hash = hash_api_key(key);
        state
            .api_key_roles
            .get(&key_hash)
            .cloned()
            .unwrap_or_else(|| "default".to_string())
    } else {
        "default".to_string()
    };

    let role = state.roles.get(&role_name).cloned().unwrap_or_default();

    // Check custom permissions first (override role)
    if let Some(key) = api_key {
        let key_hash = hash_api_key(key);
        if let Some(custom) = state.api_key_custom.get(&key_hash) {
            if let Some(perm) = custom.get(tool_type) {
                return match perm {
                    ToolPermission::Enabled => PermissionCheckResult::Allowed,
                    ToolPermission::Disabled => PermissionCheckResult::Denied {
                        reason: "Tool disabled by custom permission".to_string(),
                    },
                    ToolPermission::RequiresApproval => PermissionCheckResult::RequiresApproval,
                };
            }
        }
    }

    // Check role permissions
    match role.permissions.get(tool_type) {
        Some(ToolPermission::Disabled) => PermissionCheckResult::Denied {
            reason: format!("Tool disabled for role '{}'", role_name),
        },
        Some(ToolPermission::RequiresApproval) => PermissionCheckResult::RequiresApproval,
        _ => PermissionCheckResult::Allowed,
    }
}

/// Check if chaining is allowed
pub fn check_chain_permission(api_key: Option<&str>, chain_length: usize) -> PermissionCheckResult {
    let state = match get_state().lock() {
        Ok(s) => s,
        Err(_) => return PermissionCheckResult::Allowed,
    };

    let role_name = if let Some(key) = api_key {
        let key_hash = hash_api_key(key);
        state
            .api_key_roles
            .get(&key_hash)
            .cloned()
            .unwrap_or_else(|| "default".to_string())
    } else {
        "default".to_string()
    };

    let role = state.roles.get(&role_name).cloned().unwrap_or_default();

    if !role.can_chain {
        return PermissionCheckResult::Denied {
            reason: "Tool chaining not allowed for this role".to_string(),
        };
    }

    if chain_length > role.max_chain_length {
        return PermissionCheckResult::Denied {
            reason: format!(
                "Chain length {} exceeds maximum {} for role '{}'",
                chain_length, role.max_chain_length, role_name
            ),
        };
    }

    PermissionCheckResult::Allowed
}

/// Check if parallel execution is allowed
pub fn check_parallel_permission(api_key: Option<&str>) -> bool {
    let state = match get_state().lock() {
        Ok(s) => s,
        Err(_) => return true,
    };

    let role_name = if let Some(key) = api_key {
        let key_hash = hash_api_key(key);
        state
            .api_key_roles
            .get(&key_hash)
            .cloned()
            .unwrap_or_else(|| "default".to_string())
    } else {
        "default".to_string()
    };

    let role = state.roles.get(&role_name).cloned().unwrap_or_default();
    role.can_parallel
}

/// Result of permission check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PermissionCheckResult {
    Allowed,
    Denied { reason: String },
    RequiresApproval,
}

impl PermissionCheckResult {
    pub fn is_allowed(&self) -> bool {
        matches!(self, PermissionCheckResult::Allowed)
    }
}

/// Set role for an API key
pub fn set_api_key_role(api_key: &str, role_name: &str) {
    if let Ok(mut state) = get_state().lock() {
        let key_hash = hash_api_key(api_key);
        state.api_key_roles.insert(key_hash, role_name.to_string());
        debug!(role = role_name, "API key role set");
    }
}

/// Set custom permission for an API key
pub fn set_api_key_permission(api_key: &str, tool_type: &str, permission: ToolPermission) {
    if let Ok(mut state) = get_state().lock() {
        let key_hash = hash_api_key(api_key);
        state
            .api_key_custom
            .entry(key_hash)
            .or_insert_with(HashMap::new)
            .insert(tool_type.to_string(), permission);
    }
}

/// Globally disable a tool
pub fn disable_tool_globally(tool_type: &str) {
    if let Ok(mut state) = get_state().lock() {
        state.global_disabled.insert(tool_type.to_string());
        warn!(tool = tool_type, "Tool globally disabled");
    }
}

/// Globally enable a tool
pub fn enable_tool_globally(tool_type: &str) {
    if let Ok(mut state) = get_state().lock() {
        state.global_disabled.remove(tool_type);
        debug!(tool = tool_type, "Tool globally enabled");
    }
}

/// Get all roles
pub fn get_roles() -> Vec<ToolRole> {
    let state = match get_state().lock() {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    state.roles.values().cloned().collect()
}

/// Get globally disabled tools
pub fn get_disabled_tools() -> Vec<String> {
    let state = match get_state().lock() {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    state.global_disabled.iter().cloned().collect()
}

/// Create or update a role
pub fn upsert_role(role: ToolRole) {
    if let Ok(mut state) = get_state().lock() {
        state.roles.insert(role.name.clone(), role);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_permission() {
        let result = check_permission(None, "Calculator");
        assert!(result.is_allowed());
    }

    #[test]
    fn test_global_disable() {
        disable_tool_globally("TestTool");
        let result = check_permission(None, "TestTool");
        assert!(!result.is_allowed());
        enable_tool_globally("TestTool");
    }

    #[test]
    fn test_roles() {
        let roles = get_roles();
        assert!(roles.len() >= 4); // admin, user, readonly, restricted
    }
}

use dashmap::DashMap;
use std::sync::{Once, OnceLock};
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct FunctionInfo {
    pub name: &'static str,
    pub module_path: &'static str,
    pub full_name: &'static str,
    pub signature: &'static str,
}

inventory::collect!(FunctionInfo);

static FUNCTION_REGISTRY: OnceLock<DashMap<&'static str, FunctionInfo>> = OnceLock::new();
static FUNCTION_INVENTORY_LOADED: Once = Once::new();

fn function_registry() -> &'static DashMap<&'static str, FunctionInfo> {
    FUNCTION_REGISTRY.get_or_init(DashMap::new)
}

fn load_function_inventory() {
    FUNCTION_INVENTORY_LOADED.call_once(|| {
        for info in inventory::iter::<FunctionInfo> {
            function_registry().entry(info.full_name).or_insert(*info);
        }
    });
}

pub fn register_function(info: FunctionInfo) {
    function_registry().entry(info.full_name).or_insert(info);
}

pub fn function_info(full_name: &str) -> Option<FunctionInfo> {
    load_function_inventory();
    function_registry().get(full_name).map(|entry| *entry)
}

pub fn function_infos() -> Vec<FunctionInfo> {
    load_function_inventory();
    function_registry().iter().map(|entry| *entry).collect()
}

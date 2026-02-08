//! Service definitions, scanning, and lifecycle management (ServiceManager).

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Weak},
    time::Duration,
};

use dashmap::DashMap;
use petgraph::{
    Graph,
    graph::{DiGraph, NodeIndex},
    prelude::StableDiGraph,
    unionfind::UnionFind,
    visit::{EdgeRef, NodeIndexable},
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

/// Unique key for a service: (name, version).
pub type ServiceKey = (Arc<str>, Arc<str>);

/// Configuration for a single service (name, version, program, args, dependencies, workspace).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub version: String,
    pub program: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<(String, String)>,
    pub workspace: Option<PathBuf>,
}

/// Immutable metadata for a service used at runtime (name, version, program, args, workspace).
#[derive(Debug, Clone)]
pub struct ServiceMeta {
    pub name: Arc<str>,
    pub version: Arc<str>,
    pub program: PathBuf,
    pub args: Vec<Arc<str>>,
    pub workspace: Option<PathBuf>,
}

struct ExtractedService {
    meta: ServiceMeta,
    deps: Vec<ServiceKey>,
}

/// Runtime state of a service.
#[derive(Debug, Clone)]
pub enum ServiceState {
    Pending,
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed(String),
    Skipped,
}

impl ToString for ServiceState {
    fn to_string(&self) -> String {
        match self {
            Self::Pending => "Pending".to_string(),
            Self::Starting => "Starting".to_string(),
            Self::Running => "Running".to_string(),
            Self::Stopping => "Stopping".to_string(),
            Self::Stopped => "Stopped".to_string(),
            Self::Failed(reason) => format!("Failed: {}", reason),
            Self::Skipped => "Skipped".to_string(),
        }
    }
}

struct ServiceGroup {
    pub graph: StableDiGraph<ServiceMeta, ()>,
    pub nodeidx_map: HashMap<ServiceKey, NodeIndex>,
}

/// Item in the dead-letter queue: a service that could not be started or was removed (key, reason, meta).
pub struct DeadLetterQueueItem {
    pub key: ServiceKey,
    pub reason: String,
    pub meta: ServiceMeta,
}

fn validate_service_name_unique(
    service_configs: Vec<ServiceConfig>,
    dlq: &mut Vec<DeadLetterQueueItem>,
) -> HashMap<ServiceKey, ExtractedService> {
    let mut ret: HashMap<ServiceKey, ExtractedService> = HashMap::new();
    for config in service_configs.into_iter() {
        let key: ServiceKey = (config.name.clone().into(), config.version.clone().into());
        let meta = ServiceMeta {
            name: key.0.clone(),
            version: key.1.clone(),
            program: config.program.into(),
            args: config.args.into_iter().map(|s| s.into()).collect(),
            workspace: config.workspace,
        };
        if ret.contains_key(&key) {
            let reason = format!("Service {}:v{} is not unique", &*key.0, &*key.1);
            warn!("name" = &*key.0, "version" = &*key.1, "{}", reason.clone());
            dlq.push(DeadLetterQueueItem {
                key: key.clone(),
                reason,
                meta,
            });
            continue;
        }
        let deps = config
            .dependencies
            .into_iter()
            .map(|(n, v)| (n.into(), v.into()))
            .collect();
        ret.insert(key, ExtractedService { meta, deps });
    }
    ret
}

fn validate_service_dependencies(
    service_infos: &mut HashMap<ServiceKey, ExtractedService>,
    dlq: &mut Vec<DeadLetterQueueItem>,
) {
    let mut is_changed = true;
    while is_changed {
        is_changed = false;
        let mut removed_services: Vec<(ServiceKey, ServiceKey)> = Vec::new();
        for (service_key, ExtractedService { deps, .. }) in service_infos.iter() {
            for dep in deps.iter() {
                if !service_infos.contains_key(dep) {
                    removed_services.push((service_key.clone(), dep.clone()));
                    break;
                }
            }
        }
        if !removed_services.is_empty() {
            is_changed = true;
            for (service_key, dep_key) in removed_services.into_iter() {
                let ExtractedService { meta, .. } = match service_infos.remove(&service_key) {
                    Some(extracted_service) => extracted_service,
                    None => {
                        error!(
                            "name" = &*service_key.0,
                            "version" = &*service_key.1,
                            "Service not found"
                        );
                        continue;
                    }
                };
                warn!(
                    "name" = &*service_key.0,
                    "version" = &*service_key.1,
                    "Dependency service {}/{} not found, removing service",
                    &*dep_key.0,
                    &*dep_key.1
                );
                dlq.push(DeadLetterQueueItem {
                    key: service_key.clone(),
                    reason: format!(
                        "Dependency service {}/{} not found",
                        &*dep_key.0, &*dep_key.1
                    ),
                    meta,
                });
            }
        }
    }
}

fn split_services(service_infos: &HashMap<ServiceKey, ExtractedService>) -> Vec<Vec<ServiceKey>> {
    let mut all_nodes_graph: Graph<ServiceKey, ()> = DiGraph::new();
    let mut all_nodes_nodeidx_map: HashMap<ServiceKey, NodeIndex> =
        HashMap::with_capacity(service_infos.len());
    for (service_key, _) in service_infos.iter() {
        let nodeidx = all_nodes_graph.add_node(service_key.clone());
        all_nodes_nodeidx_map.insert(service_key.clone(), nodeidx);
    }
    for (service_key, ExtractedService { deps, .. }) in service_infos.iter() {
        let cur_nodeidx = match all_nodes_nodeidx_map.get(service_key) {
            Some(idx) => *idx,
            None => {
                error!(
                    "name" = &*service_key.0,
                    "version" = &*service_key.1,
                    "Service not found"
                );
                continue;
            }
        };
        for dep in deps.iter() {
            let dep_nodeidx = match all_nodes_nodeidx_map.get(dep) {
                Some(idx) => *idx,
                None => {
                    error!(
                        "name" = &*dep.0,
                        "version" = &*dep.1,
                        "Dependency service not found"
                    );
                    continue;
                }
            };
            all_nodes_graph.add_edge(dep_nodeidx, cur_nodeidx, ());
        }
    }
    let components = get_weakly_connected_components(&all_nodes_graph);
    let mut ret = Vec::with_capacity(components.len());
    for component in components {
        let mut service_keys = Vec::with_capacity(component.len());
        for nodeidx in component {
            let service_key = match all_nodes_graph.node_weight(nodeidx) {
                Some(s) => s,
                None => {
                    error!("nodeidx" = nodeidx.index(), "Node index is not found");
                    continue;
                }
            };
            service_keys.push(service_key.clone());
        }
        ret.push(service_keys);
    }
    ret
}

fn get_weakly_connected_components(graph: &Graph<ServiceKey, ()>) -> Vec<Vec<NodeIndex>> {
    let mut uf = UnionFind::new(graph.node_bound());
    for edge in graph.edge_references() {
        let src = edge.source();
        let dst = edge.target();
        uf.union(src, dst);
    }
    let mut components: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();
    for nodeidx in graph.node_indices() {
        let root = uf.find(nodeidx);
        components.entry(root).or_default().push(nodeidx);
    }
    let ret = components.into_values().collect();
    ret
}

fn build_service_group(
    service_keys: Vec<ServiceKey>,
    service_infos: &mut HashMap<ServiceKey, ExtractedService>,
    dlq: &mut Vec<DeadLetterQueueItem>,
) -> Option<ServiceGroup> {
    let mut extracted_services: Vec<ExtractedService> = Vec::with_capacity(service_keys.len());
    let mut is_all_meta_found = true;
    for service_key in service_keys.into_iter() {
        match service_infos.remove(&service_key) {
            Some(extracted_service) => extracted_services.push(extracted_service),
            None => {
                error!(
                    "name" = &*service_key.0,
                    "version" = &*service_key.1,
                    "Service not found during group build"
                );
                is_all_meta_found = false;
            }
        };
    }
    if !is_all_meta_found {
        warn!("Service group incomplete, rolling back extracted services to DLQ");
        for service in extracted_services {
            let key = (service.meta.name.clone(), service.meta.version.clone());
            dlq.push(DeadLetterQueueItem {
                key,
                reason: "Graph build failed because sibling services in the group were missing"
                    .into(),
                meta: service.meta,
            });
        }
        return None;
    }

    let mut graph: StableDiGraph<ServiceMeta, ()> = StableDiGraph::new();
    let mut nodeidx_map: HashMap<ServiceKey, NodeIndex> =
        HashMap::with_capacity(extracted_services.len());
    let mut edge_construction_data: Vec<(NodeIndex, Vec<ServiceKey>)> =
        Vec::with_capacity(extracted_services.len());
    for ExtractedService { meta, deps } in extracted_services {
        let key = (meta.name.clone(), meta.version.clone());
        let nodeidx = graph.add_node(meta);
        nodeidx_map.insert(key.clone(), nodeidx);
        edge_construction_data.push((nodeidx, deps));
    }
    for (cur_nodeidx, deps) in edge_construction_data {
        for dep in deps {
            let dep_nodeidx = match nodeidx_map.get(&dep) {
                Some(idx) => *idx,
                None => {
                    error!(
                        "name" = &*dep.0,
                        "version" = &*dep.1,
                        "Dependency service not found"
                    );
                    continue;
                }
            };
            graph.add_edge(dep_nodeidx, cur_nodeidx, ());
        }
    }

    if petgraph::algo::is_cyclic_directed(&graph) {
        let (nodes, _) = graph.into_nodes_edges_iters();
        for node in nodes {
            let meta = node.weight;
            let key = (meta.name.clone(), meta.version.clone());
            warn!(
                "name" = %meta.name,
                "version" = %meta.version,
                "Service group dependency is cyclic"
            );
            dlq.push(DeadLetterQueueItem {
                key,
                reason: "Service group dependency is cyclic".into(),
                meta,
            });
        }
        None
    } else {
        Some(ServiceGroup { graph, nodeidx_map })
    }
}

fn build_groups_from_configs(
    service_configs: Vec<ServiceConfig>,
    dlq: &mut Vec<DeadLetterQueueItem>,
) -> Vec<ServiceGroup> {
    let mut ret = Vec::new();
    let mut service_infos = validate_service_name_unique(service_configs, dlq);
    validate_service_dependencies(&mut service_infos, dlq);
    let service_key_groups = split_services(&service_infos);
    for service_keys in service_key_groups {
        if let Some(group) = build_service_group(service_keys, &mut service_infos, dlq) {
            ret.push(group);
        }
    }
    ret
}

fn build_service_groupidx_map(groups: &[ServiceGroup]) -> HashMap<ServiceKey, usize> {
    let mut ret = HashMap::new();
    for (groupidx, group) in groups.iter().enumerate() {
        for service_key in group.nodeidx_map.keys() {
            ret.insert(service_key.clone(), groupidx);
        }
    }
    ret
}

fn build_service_state_map(groups: &[ServiceGroup]) -> DashMap<ServiceKey, ServiceState> {
    let ret = DashMap::new();
    for group in groups {
        for service_key in group.nodeidx_map.keys() {
            ret.insert(service_key.clone(), ServiceState::Pending);
        }
    }
    ret
}

#[derive(Debug)]
enum ServiceManagerEvent {
    ServiceStarted {
        service_key: ServiceKey,
    },
    ServiceStopped {
        service_key: ServiceKey,
    },
    ServiceCrashed {
        service_key: ServiceKey,
        reason: String,
    },
}

async fn service_task(
    meta: ServiceMeta,
    event_tx: mpsc::Sender<ServiceManagerEvent>,
    cancel_token: CancellationToken,
) {
    let mut cmd = tokio::process::Command::new(&*meta.program);
    let service_key: ServiceKey = (meta.name.clone(), meta.version.clone());
    cmd.args(meta.args.iter().map(|s| &**s));
    if let Some(workspace) = meta.workspace {
        if workspace.exists() {
            cmd.current_dir(workspace);
        } else {
            warn!("workspace" = ?workspace, "Workspace not found, using current directory");
        }
    }
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            let event = ServiceManagerEvent::ServiceCrashed {
                service_key: service_key.clone(),
                reason: format!("Failed to spawn service: {e}"),
            };
            warn!("Failed to spawn service: {e}");
            if let Err(e) = event_tx.send(event).await {
                warn!("error" = ?e, "name" = &*service_key.0, "version" = &*service_key.1, "Failed to send ServiceCrashed event");
            }
            return;
        }
    };
    info!(
        "name" = &*service_key.0,
        "version" = &*service_key.1,
        "Service task running"
    );
    let event = ServiceManagerEvent::ServiceStarted {
        service_key: service_key.clone(),
    };
    if let Err(e) = event_tx.send(event).await {
        warn!("error" = ?e, "name" = &*service_key.0, "version" = &*service_key.1, "Failed to send ServiceStarted event");
    }
    tokio::select! {
        _ = cancel_token.cancelled() => {
            info!("name" = &*service_key.0, "version" = &*service_key.1, "Service task cancelled");
            match child.kill().await {
                Ok(_) => {
                    info!("name" = &*service_key.0, "version" = &*service_key.1, "Service task killed");
                    let event = ServiceManagerEvent::ServiceStopped {
                        service_key: service_key.clone(),
                    };
                    if let Err(e) = event_tx.send(event).await {
                        warn!("error" = ?e, "name" = &*service_key.0, "version" = &*service_key.1, "Failed to send ServiceStopped event");
                    }
                }
                Err(e) => {
                    warn!("name" = &*service_key.0, "version" = &*service_key.1, "error" = ?e, "Failed to kill service");
                    let event = ServiceManagerEvent::ServiceCrashed {
                        service_key: service_key.clone(),
                        reason: format!("Service task killed with error: {e}"),
                    };
                    if let Err(e) = event_tx.send(event).await {
                        warn!("error" = ?e, "name" = &*service_key.0, "version" = &*service_key.1, "Failed to send ServiceCrashed event");
                    }
                }
            }
        }
        exit_status_rs = child.wait() => {
            warn!("name" = &*service_key.0, "version" = &*service_key.1, "Service task exited unexpectedly");
            let reason = match exit_status_rs {
                Ok(exit_status) => format!("Service task exited with status: {exit_status}"),
                Err(e) => format!("Service task exited with error: {e}"),
            };
            let event = ServiceManagerEvent::ServiceCrashed {
                service_key: service_key.clone(),
                reason,
            };
            if let Err(e) = event_tx.send(event).await {
                warn!("error" = ?e, "name" = &*service_key.0, "version" = &*service_key.1, "Failed to send ServiceCrashed event");
            }
        }
    }
    info!(
        "name" = &*service_key.0,
        "version" = &*service_key.1,
        "Service task exited"
    );
}

/// Manages service groups, lifecycle (launch/stop), and state; built from a list of [ServiceConfig].
pub struct ServiceManager {
    service_groups: Vec<ServiceGroup>,
    service_groupidx_map: HashMap<ServiceKey, usize>,
    service_state_map: DashMap<ServiceKey, ServiceState>,
    dlq: Vec<DeadLetterQueueItem>,
    service_canceltoken_map: DashMap<ServiceKey, CancellationToken>,
    cancel_token: CancellationToken,
    event_tx: mpsc::Sender<ServiceManagerEvent>,
}

impl ServiceManager {
    /// Builds a new [ServiceManager] from the given configs (validates, builds groups, starts event handler).
    ///
    /// # Arguments
    ///
    /// * `service_configs` - List of service configs to load.
    ///
    /// # Returns
    ///
    /// An [Arc] to the new [ServiceManager].
    pub fn from_configs(service_configs: Vec<ServiceConfig>) -> Arc<Self> {
        let mut dlq = Vec::new();
        let groups = build_groups_from_configs(service_configs, &mut dlq);
        let service_groupidx_map = build_service_groupidx_map(&groups);
        let service_state_map = build_service_state_map(&groups);
        let (event_tx, event_rx) = mpsc::channel(16);
        let manager = Self {
            service_groups: groups,
            service_groupidx_map,
            service_state_map,
            dlq,
            service_canceltoken_map: DashMap::new(),
            cancel_token: CancellationToken::new(),
            event_tx,
        };
        let manager_arc = Arc::new(manager);
        tokio::spawn(handle_service_manager_event(
            event_rx,
            Arc::downgrade(&manager_arc),
        ));
        manager_arc
    }

    /// Returns the current [ServiceState] for the service (name, version).
    ///
    /// # Arguments
    ///
    /// * `name` - Service name.
    /// * `version` - Service version.
    ///
    /// # Returns
    ///
    /// `Some(state)` if the service is known, else `None`.
    pub fn service_state(&self, name: &str, version: &str) -> Option<ServiceState> {
        let key: ServiceKey = (name.into(), version.into());
        self.service_state_map.get(&key).map(|s| s.clone())
    }

    /// Sets the [ServiceState] for the service (name, version).
    ///
    /// # Arguments
    ///
    /// * `name` - Service name.
    /// * `version` - Service version.
    /// * `state` - New state to set.
    pub fn set_service_state(&self, name: &str, version: &str, state: ServiceState) {
        let key: ServiceKey = (name.into(), version.into());
        self.service_state_map.insert(key, state);
    }

    /// Returns the dead-letter queue: services that could not be started or were removed.
    ///
    /// # Returns
    ///
    /// Slice of [DeadLetterQueueItem].
    pub fn dead_letter_queue(&self) -> &[DeadLetterQueueItem] {
        &self.dlq
    }

    /// Returns [ServiceMeta] for the service (name, version) if it exists.
    ///
    /// # Arguments
    ///
    /// * `name` - Service name.
    /// * `version` - Service version.
    ///
    /// # Returns
    ///
    /// `Some(meta)` if the service is in a group, else `None`.
    pub fn service_meta(&self, name: &str, version: &str) -> Option<ServiceMeta> {
        let key: ServiceKey = (name.into(), version.into());
        let groupidx = match self.service_groupidx_map.get(&key) {
            Some(groupidx) => *groupidx,
            None => return None,
        };
        let group = &self.service_groups[groupidx];
        let nodeidx = match group.nodeidx_map.get(&key) {
            Some(nodeidx) => *nodeidx,
            None => return None,
        };
        match group.graph.node_weight(nodeidx) {
            Some(meta) => Some(meta.clone()),
            None => None,
        }
    }

    fn deps_running(&self, key: &ServiceKey) -> bool {
        let groupidx = match self.service_groupidx_map.get(key) {
            Some(groupidx) => *groupidx,
            None => {
                warn!(
                    "name" = &*key.0,
                    "version" = &*key.1,
                    "service groupidx not found"
                );
                return false;
            }
        };
        let group = &self.service_groups[groupidx];
        let cur_nodeidx = match group.nodeidx_map.get(key) {
            Some(nodeidx) => *nodeidx,
            None => {
                warn!(
                    "name" = &*key.0,
                    "version" = &*key.1,
                    "service nodeidx not found"
                );
                return false;
            }
        };
        for dep_nodeidx in group
            .graph
            .neighbors_directed(cur_nodeidx, petgraph::Incoming)
        {
            let dep_meta = match group.graph.node_weight(dep_nodeidx) {
                Some(meta) => meta,
                None => {
                    error!("dep_nodeidx" = dep_nodeidx.index(), "dep meta not found");
                    return false;
                }
            };
            let dep_state = match self.service_state(&dep_meta.name, &dep_meta.version) {
                Some(state) => state,
                None => {
                    error!(
                        "name" = &*dep_meta.name,
                        "version" = &*dep_meta.version,
                        "dep state not found"
                    );
                    return false;
                }
            };
            if let ServiceState::Running = dep_state {
                continue;
            } else {
                return false;
            }
        }
        true
    }

    /// Starts a single service if its dependencies are running.
    ///
    /// # Arguments
    ///
    /// * `meta` - [ServiceMeta] of the service to launch.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success or if already running / deps not running (no error); `Err` on invalid state.
    pub async fn launch_service(&self, meta: &ServiceMeta) -> anyhow::Result<()> {
        let service_key: ServiceKey = (meta.name.clone(), meta.version.clone());
        if !self.deps_running(&service_key) {
            warn!(
                "name" = &*service_key.0,
                "version" = &*service_key.1,
                "Dependencies not running"
            );
            return Ok(());
        }
        let mut entry = match self.service_state_map.get_mut(&service_key) {
            Some(entry) => entry,
            None => {
                error!(
                    "name" = &*service_key.0,
                    "version" = &*service_key.1,
                    "service state not found"
                );
                anyhow::bail!("service state not found");
            }
        };
        let service_state = entry.value_mut();
        match service_state {
            ServiceState::Running => {
                info!(
                    "name" = &*service_key.0,
                    "version" = &*service_key.1,
                    "Service is already running"
                );
                return Ok(());
            }
            ServiceState::Pending
            | ServiceState::Stopped
            | ServiceState::Failed(_)
            | ServiceState::Skipped => {
                *service_state = ServiceState::Starting;
            }
            ServiceState::Starting | ServiceState::Stopping => {
                warn!(
                    "name" = &*service_key.0,
                    "version" = &*service_key.1,
                    "state" = ?service_state,
                    "Service is in a mid-state, ignoring launch request"
                );
                anyhow::bail!(
                    "Service is in a mid-state, ignoring launch request: {:?}",
                    service_state
                );
            }
        }
        drop(entry);

        let event_tx = self.event_tx.clone();
        let cancel_token = self.cancel_token.child_token();
        self.service_canceltoken_map
            .insert(service_key.clone(), cancel_token.clone());
        let meta = meta.clone();
        info!(
            "name" = &*service_key.0,
            "version" = &*service_key.1,
            "Starting service"
        );
        tokio::spawn(service_task(meta, event_tx, cancel_token));
        return Ok(());
    }

    async fn wait_service_running(&self, name: &str, version: &str) -> anyhow::Result<()> {
        const POLLING_INTERVAL: Duration = Duration::from_millis(100);
        let mut interval = tokio::time::interval(POLLING_INTERVAL);
        loop {
            interval.tick().await;
            let state = match self.service_state(name, version) {
                Some(state) => state,
                None => {
                    error!(
                        "name" = name,
                        "version" = version,
                        "service state not found"
                    );
                    return Err(anyhow::anyhow!("service state not found"));
                }
            };
            match state {
                ServiceState::Running => return Ok(()),
                ServiceState::Starting => (),
                _ => {
                    warn!(
                        "name" = name,
                        "version" = version,
                        "state" = ?state,
                        "service is not running or starting and will not be considered as running"
                    );
                    return Err(anyhow::anyhow!(
                        "service is not running or starting and will not be considered as running: {:?}",
                        state
                    ));
                }
            }
        }
    }

    /// Launches all services in a group in dependency order, waiting up to `service_start_timeout` per service.
    ///
    /// # Arguments
    ///
    /// * `groupidx` - Index of the group to launch.
    /// * `service_start_timeout` - Max duration to wait for each service to reach Running.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success; `Err` if group index is invalid or toposort fails.
    pub async fn launch_group(
        &self,
        groupidx: usize,
        service_start_timeout: Duration,
    ) -> anyhow::Result<()> {
        let group = self
            .service_groups
            .get(groupidx)
            .ok_or_else(|| anyhow::anyhow!("Group index out of bounds"))?;

        let sorted_nodes = petgraph::algo::toposort(&group.graph, None).map_err(|e| {
            warn!("groupidx" = groupidx, "error" = ?e, "Failed to get toposort");
            anyhow::anyhow!("Failed to get toposort: {:?}", e)
        })?;

        let start_meta_order: Vec<&ServiceMeta> = sorted_nodes
            .into_iter()
            .map(|idx| {
                group.graph.node_weight(idx).ok_or_else(|| {
                    error!("idx" = idx.index(), "Graph node weight missing");
                    anyhow::anyhow!("Graph node weight missing: {idx:?}")
                })
            })
            .collect::<Result<_, _>>()?;

        for meta in start_meta_order {
            self.launch_service(meta).await?;
            let start_rs = tokio::time::timeout(
                service_start_timeout,
                self.wait_service_running(&meta.name, &meta.version),
            )
            .await;
            match start_rs {
                Ok(Ok(_)) => (),
                Ok(Err(e)) => {
                    warn!("name" = &*meta.name, "version" = &*meta.version, "error" = ?e, "Failed to wait for service to start");
                }
                Err(_) => {
                    warn!(
                        "name" = &*meta.name,
                        "version" = &*meta.version,
                        "Service start timeout"
                    );
                }
            }
        }
        Ok(())
    }

    fn rev_dep_keys(&self, name: &str, version: &str) -> anyhow::Result<Vec<ServiceKey>> {
        let key: ServiceKey = (name.into(), version.into());
        let groupidx = match self.service_groupidx_map.get(&key) {
            Some(groupidx) => *groupidx,
            None => {
                error!(
                    "name" = name,
                    "version" = version,
                    "service groupidx not found"
                );
                anyhow::bail!("service groupidx not found: {name}/{version}");
            }
        };
        let group = &self.service_groups[groupidx];
        let cur_nodeidx = match group.nodeidx_map.get(&key) {
            Some(nodeidx) => *nodeidx,
            None => {
                error!(
                    "name" = name,
                    "version" = version,
                    "service nodeidx not found"
                );
                anyhow::bail!("service nodeidx not found: {name}/{version}");
            }
        };
        let mut ret = Vec::new();
        for rev_dep_nodeidx in group
            .graph
            .neighbors_directed(cur_nodeidx, petgraph::Outgoing)
        {
            let rev_dep_meta = match group.graph.node_weight(rev_dep_nodeidx) {
                Some(meta) => meta,
                None => {
                    error!(
                        "rev_dep_nodeidx" = rev_dep_nodeidx.index(),
                        "rev_dep meta not found"
                    );
                    anyhow::bail!("rev_dep meta not found: {rev_dep_nodeidx:?}");
                }
            };
            ret.push((rev_dep_meta.name.clone(), rev_dep_meta.version.clone()));
        }
        Ok(ret)
    }

    /// Stops a service and its reverse dependencies (dependents) in order.
    ///
    /// # Arguments
    ///
    /// * `name` - Service name.
    /// * `version` - Service version.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success; `Err` if state or cancel token is invalid.
    pub async fn stop_service(&self, name: &str, version: &str) -> anyhow::Result<()> {
        let key: ServiceKey = (name.into(), version.into());
        let mut entry = match self.service_state_map.get_mut(&key) {
            Some(entry) => entry,
            None => {
                error!(
                    "name" = name,
                    "version" = version,
                    "service state not found"
                );
                anyhow::bail!("service state not found");
            }
        };
        let service_state = entry.value_mut();
        match service_state {
            ServiceState::Stopped
            | ServiceState::Failed(_)
            | ServiceState::Pending
            | ServiceState::Skipped => {
                info!(
                    "name" = name,
                    "version" = version,
                    "Service is already stopped, ignoring stop request"
                );
                return Ok(());
            }
            ServiceState::Starting | ServiceState::Stopping => {
                warn!(
                    "name" = name,
                    "version" = version,
                    "state" = ?service_state,
                    "Service is in a mid-state, ignoring stop request"
                );
                anyhow::bail!(
                    "Service is in a mid-state, ignoring stop request: {:?}",
                    service_state
                );
            }
            ServiceState::Running => {
                *service_state = ServiceState::Stopping;
            }
        }
        drop(entry);

        let rev_dep_keys = self.rev_dep_keys(name, version)?;
        for (dep_name, dep_version) in rev_dep_keys {
            Box::pin(self.stop_service(&dep_name, &dep_version)).await?;
        }

        let canceltoken = match self.service_canceltoken_map.remove(&key) {
            Some((_, canceltoken)) => canceltoken,
            None => {
                error!(
                    "name" = name,
                    "version" = version,
                    "service cancel token not found"
                );
                anyhow::bail!("service cancel token not found");
            }
        };
        canceltoken.cancel();
        Ok(())
    }

    /// Returns the number of service groups.
    ///
    /// # Returns
    ///
    /// Number of groups.
    pub fn group_num(&self) -> usize {
        self.service_groups.len()
    }

    /// Returns (name, version) for all services in the group.
    ///
    /// # Arguments
    ///
    /// * `group_idx` - Index of the group.
    ///
    /// # Returns
    ///
    /// Vector of (name, version); empty if `group_idx` is invalid.
    pub fn group_service_keys(&self, group_idx: usize) -> Vec<(String, String)> {
        let group = match self.service_groups.get(group_idx) {
            Some(group) => group,
            None => {
                warn!("group_idx" = group_idx, "group not found");
                return Vec::new();
            }
        };
        let mut ret = Vec::with_capacity(group.graph.node_count());
        for meta in group.graph.node_weights() {
            ret.push((meta.name.to_string(), meta.version.to_string()));
        }
        ret
    }

    /// Returns (name, version) for services in the group that have in-degree 0 (no dependencies within the group).
    /// Stopping these roots will cascade-stop the whole group via [Self::stop_service].
    ///
    /// # Arguments
    ///
    /// * `group_idx` - Index of the group.
    ///
    /// # Returns
    ///
    /// Vector of (name, version); empty if `group_idx` is invalid.
    pub fn group_root_service_keys(&self, group_idx: usize) -> Vec<(String, String)> {
        let group = match self.service_groups.get(group_idx) {
            Some(group) => group,
            None => {
                warn!("group_idx" = group_idx, "group not found");
                return Vec::new();
            }
        };
        let mut ret = Vec::new();
        for node_idx in group.graph.node_indices() {
            let has_incoming = group
                .graph
                .neighbors_directed(node_idx, petgraph::Incoming)
                .next()
                .is_some();
            if !has_incoming {
                if let Some(meta) = group.graph.node_weight(node_idx) {
                    ret.push((meta.name.to_string(), meta.version.to_string()));
                }
            }
        }
        ret
    }
}

impl Drop for ServiceManager {
    /// Cancels the root token so all child service tasks receive cancellation and kill their subprocesses.
    fn drop(&mut self) {
        info!("Service manager dropped, cancelling root token");
        self.cancel_token.cancel();
    }
}

async fn handle_service_manager_event(
    mut event_rx: mpsc::Receiver<ServiceManagerEvent>,
    manager: Weak<ServiceManager>,
) {
    while let Some(event) = event_rx.recv().await {
        let manager = match manager.upgrade() {
            Some(manager) => manager,
            None => {
                warn!("Manager weak pointer is invalid, stopping event handler");
                break;
            }
        };
        match event {
            ServiceManagerEvent::ServiceStarted { service_key } => {
                let mut entry = match manager.service_state_map.get_mut(&service_key) {
                    Some(entry) => entry,
                    None => {
                        error!(
                            "name" = &*service_key.0,
                            "version" = &*service_key.1,
                            "service state not found"
                        );
                        continue;
                    }
                };
                if let ServiceState::Starting = entry.value() {
                    *entry.value_mut() = ServiceState::Running;
                } else {
                    warn!(
                        "name" = &*service_key.0,
                        "version" = &*service_key.1,
                        "state" = ?entry.value(),
                        "service is not starting, ignoring event"
                    );
                }
            }
            ServiceManagerEvent::ServiceStopped { service_key } => {
                let mut entry = match manager.service_state_map.get_mut(&service_key) {
                    Some(entry) => entry,
                    None => {
                        error!(
                            "name" = &*service_key.0,
                            "version" = &*service_key.1,
                            "service state not found"
                        );
                        continue;
                    }
                };
                if let ServiceState::Stopping = entry.value() {
                    *entry.value_mut() = ServiceState::Stopped;
                } else {
                    warn!(
                        "name" = &*service_key.0,
                        "version" = &*service_key.1,
                        "state" = ?entry.value(),
                        "service is not stopping, ignoring event"
                    );
                }
            }
            ServiceManagerEvent::ServiceCrashed {
                service_key,
                reason,
            } => {
                let mut entry = match manager.service_state_map.get_mut(&service_key) {
                    Some(entry) => entry,
                    None => {
                        error!(
                            "name" = &*service_key.0,
                            "version" = &*service_key.1,
                            "service state not found"
                        );
                        continue;
                    }
                };
                manager.service_canceltoken_map.remove(&service_key);
                warn!(
                    "name" = &*service_key.0,
                    "version" = &*service_key.1,
                    "current_state" = ?entry.value(),
                    "reason" = reason,
                    "service crashed"
                );
                *entry.value_mut() = ServiceState::Failed(reason);
                let manager_clone = manager.clone();
                let (name, version) = (service_key.0.to_string(), service_key.1.to_string());
                let fut = async move {
                    let rev_dep_keys = match manager_clone.rev_dep_keys(&name, &version) {
                        Ok(keys) => keys,
                        Err(e) => {
                            error!("name" = name, "version" = version, "error" = ?e, "Failed to get rev dep keys");
                            return;
                        }
                    };
                    for (dep_name, dep_version) in rev_dep_keys {
                        if let Err(e) = manager_clone.stop_service(&dep_name, &dep_version).await {
                            warn!("name" = %dep_name, "version" = %dep_version, "error" = ?e, "Failed to stop rev dep service");
                        }
                    }
                };
                tokio::spawn(fut);
            }
        }
    }
    info!("Service manager event handler stopped");
}

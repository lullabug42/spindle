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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub version: String,
    pub description: String,
    pub program: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    pub workspace: Option<PathBuf>,
}

pub async fn scan_services(service_dir: &str) -> anyhow::Result<Vec<(ServiceConfig, String)>> {
    let mut entries = tokio::fs::read_dir(service_dir).await?;
    let mut ret = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.metadata().await?;
        if metadata.is_file() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                continue;
            }
            let file_type = match entry.file_type().await {
                Ok(ft) => ft,
                Err(e) => {
                    warn!(path = ?path, error = ?e, "Failed to get file type");
                    continue;
                }
            };
            if !file_type.is_file() {
                continue;
            }
            let file_str = match tokio::fs::read_to_string(&path).await {
                Ok(s) => s,
                Err(e) => {
                    warn!(path = ?path, error = ?e, "Failed to read service config");
                    continue;
                }
            };
            let config = match toml::from_str(&file_str) {
                Ok(config) => config,
                Err(e) => {
                    warn!(path = ?path, error = ?e, "Failed to parse service config");
                    continue;
                }
            };
            ret.push((config, path.to_string_lossy().to_string()));
        }
    }
    Ok(ret)
}

#[derive(Debug, Clone)]
pub struct ServiceMeta {
    pub name: Arc<str>,
    pub version: Arc<str>,
    pub description: Arc<str>,
    pub program: PathBuf,
    pub args: Vec<Arc<str>>,
    pub config_path: Arc<str>,
    pub workspace: Option<PathBuf>,
}

struct ExtractedService {
    meta: ServiceMeta,
    deps: Vec<Arc<str>>,
}

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

struct ServiceGroup {
    pub graph: StableDiGraph<ServiceMeta, ()>,
    pub nodeidx_map: HashMap<Arc<str>, NodeIndex>,
}

pub struct DeadLetterQueueItem {
    pub name: Arc<str>,
    pub reason: String,
    pub meta: ServiceMeta,
}

fn validate_service_name_unique(
    service_configs: Vec<(ServiceConfig, String)>,
    dlq: &mut Vec<DeadLetterQueueItem>,
) -> HashMap<Arc<str>, ExtractedService> {
    let mut ret: HashMap<Arc<str>, ExtractedService> = HashMap::new();
    for (config, path) in service_configs.into_iter() {
        let key: Arc<str> = config.name.into();
        let meta = ServiceMeta {
            name: key.clone(),
            version: config.version.into(),
            description: config.description.into(),
            program: config.program.into(),
            args: config.args.into_iter().map(|s| s.into()).collect(),
            config_path: path.into(),
            workspace: config.workspace,
        };
        if ret.contains_key(&key) {
            warn!("name" = &*key, "Service name is not unique");
            dlq.push(DeadLetterQueueItem {
                name: key.clone(),
                reason: "Service name is not unique".to_string(),
                meta,
            });
            continue;
        }
        let deps = config.dependencies.into_iter().map(|s| s.into()).collect();
        ret.insert(key, ExtractedService { meta, deps });
    }
    ret
}

fn validate_service_dependencies(
    service_infos: &mut HashMap<Arc<str>, ExtractedService>,
    dlq: &mut Vec<DeadLetterQueueItem>,
) {
    let mut is_changed = true;
    while is_changed {
        is_changed = false;
        let mut removed_services: Vec<(Arc<str>, Arc<str>)> = Vec::new();
        for (service_name, ExtractedService { deps, .. }) in service_infos.iter() {
            for dep in deps.iter() {
                if !service_infos.contains_key(dep) {
                    removed_services.push((service_name.clone(), dep.clone()));
                    break;
                }
            }
        }
        if !removed_services.is_empty() {
            is_changed = true;
            for (service_name, dep_name) in removed_services.into_iter() {
                let ExtractedService { meta, .. } = match service_infos.remove(&service_name) {
                    Some(extracted_service) => extracted_service,
                    None => {
                        error!("name" = &*service_name, "Service not found");
                        continue;
                    }
                };
                warn!(
                    "name" = &*service_name,
                    "Dependency service {} not found, removing service", &*dep_name
                );
                dlq.push(DeadLetterQueueItem {
                    name: service_name.clone(),
                    reason: format!("Dependency service {} not found", &*dep_name),
                    meta,
                });
            }
        }
    }
}

fn split_services(service_infos: &HashMap<Arc<str>, ExtractedService>) -> Vec<Vec<Arc<str>>> {
    let mut all_nodes_graph: Graph<Arc<str>, ()> = DiGraph::new();
    let mut all_nodes_nodeidx_map: HashMap<Arc<str>, NodeIndex> =
        HashMap::with_capacity(service_infos.len());
    for (service_name, _) in service_infos.iter() {
        let nodeidx = all_nodes_graph.add_node(service_name.clone());
        all_nodes_nodeidx_map.insert(service_name.clone(), nodeidx);
    }
    for (service_name, ExtractedService { deps, .. }) in service_infos.iter() {
        let cur_nodeidx = match all_nodes_nodeidx_map.get(service_name) {
            Some(idx) => *idx,
            None => {
                error!("name" = &**service_name, "Service not found");
                continue;
            }
        };
        for dep in deps.iter() {
            let dep_nodeidx = match all_nodes_nodeidx_map.get(dep) {
                Some(idx) => *idx,
                None => {
                    error!("name" = &**dep, "Dependency service not found");
                    continue;
                }
            };
            all_nodes_graph.add_edge(dep_nodeidx, cur_nodeidx, ());
        }
    }
    let components = get_weakly_connected_components(&all_nodes_graph);
    let mut ret = Vec::with_capacity(components.len());
    for component in components {
        let mut service_names = Vec::with_capacity(component.len());
        for nodeidx in component {
            let service_name = match all_nodes_graph.node_weight(nodeidx) {
                Some(s) => s,
                None => {
                    error!("nodeidx" = nodeidx.index(), "Node index is not found");
                    continue;
                }
            };
            service_names.push(service_name.clone());
        }
        ret.push(service_names);
    }
    ret
}

fn get_weakly_connected_components(graph: &Graph<Arc<str>, ()>) -> Vec<Vec<NodeIndex>> {
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
    service_names: Vec<Arc<str>>,
    service_infos: &mut HashMap<Arc<str>, ExtractedService>,
    dlq: &mut Vec<DeadLetterQueueItem>,
) -> Option<ServiceGroup> {
    let mut extracted_services: Vec<ExtractedService> = Vec::with_capacity(service_names.len());
    let mut is_all_meta_found = true;
    for service_name in service_names.into_iter() {
        match service_infos.remove(&service_name) {
            Some(extracted_service) => extracted_services.push(extracted_service),
            None => {
                error!(
                    "name" = &*service_name,
                    "Service not found during group build"
                );
                is_all_meta_found = false;
            }
        };
    }
    if !is_all_meta_found {
        warn!("Service group incomplete, rolling back extracted services to DLQ");
        for service in extracted_services {
            dlq.push(DeadLetterQueueItem {
                name: service.meta.name.clone(),
                reason: "Graph build failed because sibling services in the group were missing"
                    .into(),
                meta: service.meta,
            });
        }
        return None;
    }

    let mut graph: StableDiGraph<ServiceMeta, ()> = StableDiGraph::new();
    let mut nodeidx_map: HashMap<Arc<str>, NodeIndex> =
        HashMap::with_capacity(extracted_services.len());
    let mut edge_construction_data: Vec<(NodeIndex, Vec<Arc<str>>)> =
        Vec::with_capacity(extracted_services.len());
    for ExtractedService { meta, deps } in extracted_services {
        let name = meta.name.clone();
        let nodeidx = graph.add_node(meta);
        nodeidx_map.insert(name, nodeidx);
        edge_construction_data.push((nodeidx, deps));
    }
    for (cur_nodeidx, deps) in edge_construction_data {
        for dep in deps {
            let dep_nodeidx = match nodeidx_map.get(&dep) {
                Some(idx) => *idx,
                None => {
                    error!("name" = &*dep, "Dependency service not found");
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
            let name = meta.name.clone();
            warn!(
                "name" = name.to_string(),
                "Service group dependency is cyclic"
            );
            dlq.push(DeadLetterQueueItem {
                name,
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
    service_configs: Vec<(ServiceConfig, String)>,
    dlq: &mut Vec<DeadLetterQueueItem>,
) -> Vec<ServiceGroup> {
    let mut ret = Vec::new();
    let mut service_infos = validate_service_name_unique(service_configs, dlq);
    validate_service_dependencies(&mut service_infos, dlq);
    let service_names_groups = split_services(&service_infos);
    for service_names in service_names_groups {
        if let Some(group) = build_service_group(service_names, &mut service_infos, dlq) {
            ret.push(group);
        }
    }
    ret
}

fn build_service_groupidx_map(groups: &[ServiceGroup]) -> HashMap<Arc<str>, usize> {
    let mut ret = HashMap::new();
    for (groupidx, group) in groups.iter().enumerate() {
        for service_name in group.nodeidx_map.keys() {
            ret.insert(service_name.clone(), groupidx);
        }
    }
    ret
}

fn build_service_state_map(groups: &[ServiceGroup]) -> DashMap<Arc<str>, ServiceState> {
    let ret = DashMap::new();
    for group in groups {
        for service_name in group.nodeidx_map.keys() {
            ret.insert(service_name.clone(), ServiceState::Pending);
        }
    }
    ret
}

#[derive(Debug)]
enum ServiceManagerEvent {
    ServiceStarted {
        service_name: Arc<str>,
    },
    ServiceStopped {
        service_name: Arc<str>,
    },
    ServiceCrashed {
        service_name: Arc<str>,
        reason: String,
    },
}

async fn service_task(
    meta: ServiceMeta,
    event_tx: mpsc::Sender<ServiceManagerEvent>,
    cancel_token: CancellationToken,
) {
    let mut cmd = tokio::process::Command::new(&*meta.program);
    let service_name = meta.name.clone();
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
                service_name: service_name.clone(),
                reason: format!("Failed to spawn service: {e}"),
            };
            warn!("Failed to spawn service: {e}");
            if let Err(e) = event_tx.send(event).await {
                warn!("error" = ?e, "service_name" = &*service_name, "Failed to send ServiceCrashed event");
            }
            return;
        }
    };
    info!("service_name" = &*service_name, "Service task running");
    let event = ServiceManagerEvent::ServiceStarted {
        service_name: service_name.clone(),
    };
    if let Err(e) = event_tx.send(event).await {
        warn!("error" = ?e, "service_name" = &*service_name, "Failed to send ServiceStarted event");
    }
    tokio::select! {
        _ = cancel_token.cancelled() => {
            info!("service_name" = &*service_name, "Service task cancelled");
            match child.kill().await {
                Ok(_) => {
                    info!("service_name" = &*service_name, "Service task killed");
                    let event = ServiceManagerEvent::ServiceStopped {
                        service_name: service_name.clone(),
                    };
                    if let Err(e) = event_tx.send(event).await {
                        warn!("error" = ?e, "service_name" = &*service_name, "Failed to send ServiceStopped event");
                    }
                }
                Err(e) => {
                    warn!("service_name" = &*service_name, "error" = ?e, "Failed to kill service");
                    let event = ServiceManagerEvent::ServiceCrashed {
                        service_name: service_name.clone(),
                        reason: format!("Service task killed with error: {e}"),
                    };
                    if let Err(e) = event_tx.send(event).await {
                        warn!("error" = ?e, "service_name" = &*service_name, "Failed to send ServiceCrashed event");
                    }
                }
            }
        }
        exit_status_rs = child.wait() => {
            warn!("service_name" = &*service_name, "Service task exited unexpectedly");
            let reason = match exit_status_rs {
                Ok(exit_status) => format!("Service task exited with status: {exit_status}"),
                Err(e) => format!("Service task exited with error: {e}"),
            };
            let event = ServiceManagerEvent::ServiceCrashed {
                service_name: service_name.clone(),
                reason,
            };
            if let Err(e) = event_tx.send(event).await {
                warn!("error" = ?e, "service_name" = &*service_name, "Failed to send ServiceCrashed event");
            }
        }
    }
    info!("service_name" = &*service_name, "Service task exited");
}

pub struct ServiceManager {
    service_groups: Vec<ServiceGroup>,
    service_groupidx_map: HashMap<Arc<str>, usize>,
    service_state_map: DashMap<Arc<str>, ServiceState>,
    dlq: Vec<DeadLetterQueueItem>,
    service_canceltoken_map: DashMap<Arc<str>, CancellationToken>,
    cancel_token: CancellationToken,
    event_tx: mpsc::Sender<ServiceManagerEvent>,
}

impl ServiceManager {
    pub fn from_configs(service_configs: Vec<(ServiceConfig, String)>) -> Arc<Self> {
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

    pub fn service_state(&self, service_name: &str) -> Option<ServiceState> {
        self.service_state_map.get(service_name).map(|s| s.clone())
    }

    pub fn set_service_state(&self, service_name: &str, state: ServiceState) {
        self.service_state_map.insert(service_name.into(), state);
    }

    pub fn dead_letter_queue(&self) -> &[DeadLetterQueueItem] {
        &self.dlq
    }

    pub fn service_meta(&self, service_name: &str) -> Option<ServiceMeta> {
        let groupidx = match self.service_groupidx_map.get(service_name) {
            Some(groupidx) => *groupidx,
            None => return None,
        };
        let group = &self.service_groups[groupidx];
        let nodeidx = match group.nodeidx_map.get(service_name) {
            Some(nodeidx) => *nodeidx,
            None => return None,
        };
        match group.graph.node_weight(nodeidx) {
            Some(meta) => Some(meta.clone()),
            None => None,
        }
    }

    fn deps_running(&self, service_name: &str) -> bool {
        let groupidx = match self.service_groupidx_map.get(service_name) {
            Some(groupidx) => *groupidx,
            None => {
                warn!("service_name" = service_name, "service groupidx not found");
                return false;
            }
        };
        let group = &self.service_groups[groupidx];
        let cur_nodeidx = match group.nodeidx_map.get(service_name) {
            Some(nodeidx) => *nodeidx,
            None => {
                warn!("service_name" = service_name, "service nodeidx not found");
                return false;
            }
        };
        for dep_nodeidx in group
            .graph
            .neighbors_directed(cur_nodeidx, petgraph::Incoming)
        {
            let dep_name = match group.graph.node_weight(dep_nodeidx) {
                Some(meta) => &meta.name,
                None => {
                    error!("dep_nodeidx" = dep_nodeidx.index(), "dep meta not found");
                    return false;
                }
            };
            let dep_state = match self.service_state(dep_name) {
                Some(state) => state,
                None => {
                    error!("dep_name" = &**dep_name, "dep state not found");
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

    pub async fn launch_service(&self, meta: &ServiceMeta) -> anyhow::Result<()> {
        let service_name = meta.name.clone();
        if !self.deps_running(&service_name) {
            warn!("service_name" = &*service_name, "Dependencies not running");
            return Ok(());
        }
        let mut entry = match self.service_state_map.get_mut(&service_name) {
            Some(entry) => entry,
            None => {
                error!("service_name" = &*service_name, "service state not found");
                anyhow::bail!("service state not found");
            }
        };
        let service_state = entry.value_mut();
        match service_state {
            ServiceState::Running => {
                info!(
                    "service_name" = &*service_name,
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
                    "service_name" = &*service_name,
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
            .insert(service_name.clone(), cancel_token.clone());
        let meta = meta.clone();
        info!("service_name" = &*service_name, "Starting service");
        tokio::spawn(service_task(meta, event_tx, cancel_token));
        return Ok(());
    }

    async fn wait_service_running(&self, service_name: &str) -> anyhow::Result<()> {
        const POLLING_INTERVAL: Duration = Duration::from_millis(100);
        let mut interval = tokio::time::interval(POLLING_INTERVAL);
        loop {
            interval.tick().await;
            let state = match self.service_state(service_name) {
                Some(state) => state,
                None => {
                    error!("service_name" = service_name, "service state not found");
                    return Err(anyhow::anyhow!("service state not found"));
                }
            };
            match state {
                ServiceState::Running => return Ok(()),
                ServiceState::Starting => (),
                _ => {
                    warn!(
                        "service_name" = service_name,
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
            self.launch_service(&meta).await?;
            let start_rs =
                tokio::time::timeout(service_start_timeout, self.wait_service_running(&meta.name))
                    .await;
            match start_rs {
                Ok(Ok(_)) => (),
                Ok(Err(e)) => {
                    warn!("service_name" = &*meta.name, "error" = ?e, "Failed to wait for service to start");
                }
                Err(e) => {
                    warn!("service_name" = &*meta.name, "error" = ?e, "Service start timeout");
                }
            }
        }
        Ok(())
    }

    fn rev_dep_names(&self, service_name: &str) -> anyhow::Result<Vec<Arc<str>>> {
        let groupidx = match self.service_groupidx_map.get(service_name) {
            Some(groupidx) => *groupidx,
            None => {
                error!("service_name" = service_name, "service groupidx not found");
                anyhow::bail!("service groupidx not found: {service_name}");
            }
        };
        let group = &self.service_groups[groupidx];
        let cur_nodeidx = match group.nodeidx_map.get(service_name) {
            Some(nodeidx) => *nodeidx,
            None => {
                error!("service_name" = service_name, "service nodeidx not found");
                anyhow::bail!("service nodeidx not found: {service_name}");
            }
        };
        let mut ret = Vec::new();
        for rev_dep_nodeidx in group
            .graph
            .neighbors_directed(cur_nodeidx, petgraph::Outgoing)
        {
            let rev_dep_name = match group.graph.node_weight(rev_dep_nodeidx) {
                Some(meta) => &meta.name,
                None => {
                    error!(
                        "rev_dep_nodeidx" = rev_dep_nodeidx.index(),
                        "rev_dep meta not found"
                    );
                    anyhow::bail!("rev_dep meta not found: {rev_dep_nodeidx:?}");
                }
            };
            ret.push(rev_dep_name.clone());
        }
        Ok(ret)
    }

    pub async fn stop_service(&self, service_name: &str) -> anyhow::Result<()> {
        let mut entry = match self.service_state_map.get_mut(service_name) {
            Some(entry) => entry,
            None => {
                error!("service_name" = service_name, "service state not found");
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
                    "service_name" = service_name,
                    "Service is already stopped, ignoring stop request"
                );
                return Ok(());
            }
            ServiceState::Starting | ServiceState::Stopping => {
                warn!(
                    "service_name" = service_name,
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

        let rev_dep_names = self.rev_dep_names(service_name)?;
        for rev_dep_name in rev_dep_names {
            Box::pin(self.stop_service(&rev_dep_name)).await?;
        }

        let canceltoken = match self.service_canceltoken_map.remove(service_name) {
            Some((_, canceltoken)) => canceltoken,
            None => {
                error!(
                    "service_name" = service_name,
                    "service cancel token not found"
                );
                anyhow::bail!("service cancel token not found");
            }
        };
        canceltoken.cancel();
        Ok(())
    }

    pub fn group_num(&self) -> usize {
        self.service_groups.len()
    }

    pub fn group_service_infos(&self) -> Vec<GroupServiceInfo> {
        let mut ret = Vec::with_capacity(self.service_groupidx_map.len());
        for (group_idx, group) in self.service_groups.iter().enumerate() {
            for node_idx in group.graph.node_indices() {
                let meta = match group.graph.node_weight(node_idx) {
                    Some(meta) => meta,
                    None => {
                        error!("nodeidx" = node_idx.index(), "node weight not found");
                        continue;
                    }
                };
                let dep_idxs = group
                    .graph
                    .neighbors_directed(node_idx, petgraph::Incoming)
                    .map(|neighbor_idx| neighbor_idx.index())
                    .collect();
                let info = GroupServiceInfo::new(meta, group_idx, node_idx.index(), dep_idxs);
                ret.push(info);
            }
        }
        ret
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupServiceInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub program: String,
    pub args: Vec<String>,
    pub config_path: String,
    pub workspace: Option<String>,
    pub group_idx: usize,
    pub node_idx: usize,
    pub dep_idxs: Vec<usize>,
}

impl GroupServiceInfo {
    pub fn new(
        meta: &ServiceMeta,
        group_idx: usize,
        node_idx: usize,
        dep_idxs: Vec<usize>,
    ) -> Self {
        Self {
            name: meta.name.to_string(),
            version: meta.version.to_string(),
            description: meta.description.to_string(),
            program: meta.program.to_string_lossy().to_string(),
            args: meta.args.iter().map(|s| s.to_string()).collect(),
            config_path: meta.config_path.to_string(),
            workspace: meta
                .workspace
                .clone()
                .map(|p| p.to_string_lossy().to_string()),
            group_idx,
            node_idx,
            dep_idxs,
        }
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
            ServiceManagerEvent::ServiceStarted { service_name } => {
                let mut entry = match manager.service_state_map.get_mut(&service_name) {
                    Some(entry) => entry,
                    None => {
                        error!("service_name" = &*service_name, "service state not found");
                        continue;
                    }
                };
                if let ServiceState::Starting = entry.value() {
                    *entry.value_mut() = ServiceState::Running;
                } else {
                    warn!(
                        "service_name" = &*service_name,
                        "state" = ?entry.value(),
                        "service is not starting, ignoring event"
                    );
                }
            }
            ServiceManagerEvent::ServiceStopped { service_name } => {
                let mut entry = match manager.service_state_map.get_mut(&service_name) {
                    Some(entry) => entry,
                    None => {
                        error!("service_name" = &*service_name, "service state not found");
                        continue;
                    }
                };
                if let ServiceState::Stopping = entry.value() {
                    *entry.value_mut() = ServiceState::Stopped;
                } else {
                    warn!(
                        "service_name" = &*service_name,
                        "state" = ?entry.value(),
                        "service is not stopping, ignoring event"
                    );
                }
            }
            ServiceManagerEvent::ServiceCrashed {
                service_name,
                reason,
            } => {
                let mut entry = match manager.service_state_map.get_mut(&service_name) {
                    Some(entry) => entry,
                    None => {
                        error!("service_name" = &*service_name, "service state not found");
                        continue;
                    }
                };
                manager.service_canceltoken_map.remove(&service_name);
                warn!(
                    "service_name" = &*service_name,
                    "current_state" = ?entry.value(),
                    "reason" = reason,
                    "service crashed"
                );
                *entry.value_mut() = ServiceState::Failed(reason);
                let manager_clone = manager.clone();
                let fut = async move {
                    let rev_dep_names = match manager_clone.rev_dep_names(&service_name) {
                        Ok(rev_dep_names) => rev_dep_names,
                        Err(e) => {
                            error!("service_name" = &*service_name, "error" = ?e, "Failed to get rev dep names");
                            return;
                        }
                    };
                    for rev_dep_name in rev_dep_names {
                        if let Err(e) = manager_clone.stop_service(&rev_dep_name).await {
                            warn!("service_name" = &*rev_dep_name, "error" = ?e, "Failed to stop rev dep service");
                        }
                    }
                };
                tokio::spawn(fut);
            }
        }
    }
    info!("Service manager event handler stopped");
}

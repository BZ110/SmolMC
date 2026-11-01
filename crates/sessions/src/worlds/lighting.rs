//! One shared, lazy lighting worker. Transactions publish blocks+lighting atomically.
use super::*;
use smolmc_world::{OverlayBudget, WorldLimits, terrain::ImportedTerrain};
use std::io;
use tokio::sync::{Notify, oneshot};
pub(crate) enum Reply {
    Client {
        sender: mpsc::Sender<crate::gameplay::Delivery>,
        closed: watch::Sender<bool>,
        wake: Arc<Notify>,
        positions: Vec<[i32; 3]>,
        sequence: i32,
        completion: Option<crate::replication::items::Completion>,
    },
    Control(oneshot::Sender<Result<(), WorldError>>),
}
pub(super) struct Job {
    at: tokio::time::Instant,
    handle: WorldHandle,
    original: InstanceOverlay,
    prepared: InstanceOverlay,
    base: Arc<ImportedTerrain>,
    edits: Vec<([i32; 3], Material)>,
    positions: Vec<[i32; 3]>,
    limits: WorldLimits,
    budget: Arc<OverlayBudget>,
    reply: Reply,
}
pub(super) type Sender = mpsc::Sender<Option<Job>>;
impl Worlds {
    fn light_sender(self: &Arc<Self>) -> &Sender {
        self.light_sender.get_or_init(|| {
            let (tx, mut rx) = mpsc::channel::<Option<Job>>(8);
            let weak = Arc::downgrade(self);
            let task = tokio::spawn(async move {
                while let Some(Some(job)) = rx.recv().await {
                    let Job {
                        at,
                        handle,
                        original,
                        prepared,
                        base,
                        edits,
                        positions,
                        limits,
                        budget,
                        reply,
                    } = job;
                    let compute = tokio::task::spawn_blocking(move || {
                        let patch = smolmc_world::lighting::recompute(&base, &prepared, &positions)
                            .map_err(|_| WorldError::WorldBudgetExceeded)?;
                        WorldManager::finish_lighting(&base, &prepared, patch, limits, &budget)
                    })
                    .await
                    .unwrap_or(Err(WorldError::WorldBudgetExceeded));
                    if let Some(worlds) = weak.upgrade() {
                        let result = {
                            let mut c = worlds.catalog.lock().unwrap();
                            let occupied = edits
                                .iter()
                                .filter(|(_, m)| !m.info().boxes.is_empty())
                                .any(|(pos, _)| {
                                    c.players
                                        .values()
                                        .filter(|p| p.world == handle.id)
                                        .filter_map(|p| p.game.as_ref())
                                        .any(|g| {
                                            g.try_lock()
                                                .map_or(true, |g| !g.dead && g.occupies(*pos))
                                        })
                                });
                            let result = if occupied {
                                Err(WorldError::Busy)
                            } else {
                                compute.and_then(|overlay| {
                                    c.manager.commit_prepared(handle, &original, overlay)
                                })
                            };
                            if c.light_pending.get(&handle.id) == Some(&handle) {
                                c.light_pending.remove(&handle.id);
                            }
                            if result.is_ok() {
                                if let Some(runtime) = c.worlds.get(&handle.id) {
                                    runtime.terrain.send_replace(
                                        c.manager.validate(handle).unwrap().overlay.clone(),
                                    );
                                }
                                worlds.emit(&mut c, 5, handle, None, String::new(), None);
                            }
                            result
                        };
                        #[cfg(test)]
                        crate::perf::BLOCK_COMMIT.record(at.elapsed().as_micros() as u64);
                        match reply {
                            Reply::Control(tx) => {
                                let _ = tx.send(result);
                            }
                            Reply::Client {
                                sender,
                                closed,
                                wake,
                                positions,
                                sequence,
                                completion,
                            } => {
                                if let Some(completion) = completion {
                                    completion.finish(result.is_ok());
                                }
                                let mut packets = Vec::new();
                                if let Ok(materials) = worlds.read(handle, &positions) {
                                    for (pos, material) in positions.into_iter().zip(materials) {
                                        packets.push(smolmc_protocol_777::play::block_correction(
                                            pos,
                                            material.state(),
                                        ));
                                    }
                                }
                                packets.push(smolmc_protocol_777::play::block_ack(sequence));
                                if sender
                                    .try_send(crate::gameplay::Delivery::TerrainCommitted {
                                        world: handle,
                                        at,
                                        packets: packets.into(),
                                    })
                                    .is_err()
                                {
                                    closed.send_replace(true);
                                    wake.notify_one();
                                }
                            }
                        }
                    } else if let Reply::Control(tx) = reply {
                        let _ = tx.send(Err(WorldError::UnknownInstance));
                    }
                }
            });
            *self.light_task.lock().unwrap() = Some(task);
            tx
        })
    }
    pub(crate) fn submit_terrain(
        self: &Arc<Self>,
        handle: WorldHandle,
        edits: Vec<([i32; 3], Material)>,
        client: bool,
        expected: Option<Vec<Material>>,
        reply: Reply,
    ) -> Result<(), WorldError> {
        let sender = self.light_sender();
        let permit = sender.try_reserve().map_err(|_| WorldError::Busy)?;
        let mut c = self.catalog.lock().unwrap();
        if c.light_pending.contains_key(&handle.id) {
            return Err(WorldError::Busy);
        }
        let i = c.manager.validate(handle)?;
        if client && !i.profile.enabled(smolmc_world::Feature::BlockInteractions) {
            return Err(WorldError::FeatureDisabled);
        }
        let base = i
            .template
            .terrain
            .clone()
            .ok_or(WorldError::UnsupportedFeature)?;
        if let Some(expected) = expected
            && (edits.len() != expected.len()
                || edits.iter().zip(expected).any(|((pos, _), expected)| {
                    i.overlay
                        .override_at(*pos)
                        .unwrap_or_else(|| i.template.material(*pos))
                        != expected
                }))
        {
            return Err(WorldError::Busy);
        }
        let original = i.overlay.clone();
        let limits = i.profile.limits;
        let prepared = c.manager.prepare_blocks(handle, &edits)?;
        let budget = c.manager.overlay_budget();
        c.light_pending.insert(handle.id, handle);
        permit.send(Some(Job {
            at: tokio::time::Instant::now(),
            handle,
            original,
            prepared,
            base,
            positions: edits.iter().map(|e| e.0).collect(),
            edits,
            limits,
            budget,
            reply,
        }));
        Ok(())
    }
    pub(crate) async fn edit_terrain(
        self: &Arc<Self>,
        handle: WorldHandle,
        edits: Vec<([i32; 3], Material)>,
    ) -> Result<(), WorldError> {
        let (tx, rx) = oneshot::channel();
        self.submit_terrain(handle, edits, false, None, Reply::Control(tx))?;
        rx.await.map_err(|_| WorldError::Busy)?
    }
    pub(crate) async fn stop_lighting(&self) -> io::Result<()> {
        let task = self.light_task.lock().unwrap().take();
        if let Some(task) = task {
            if let Some(sender) = self.light_sender.get() {
                let _ = sender.send(None).await;
            }
            task.await.map_err(io::Error::other)?;
        }
        Ok(())
    }
}

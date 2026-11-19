// Mission Library peer sharing (REM #358) over RPP 0.3 RC1 (`rpp_core::rns_share`).
// Transfers start only on an explicit operator action (Share / Download), run
// as background tasks outside the SOS and acknowledgement command lanes, and
// report progress through the Library projection.

const LIBRARY_SHARE_TIMEOUT: Duration = Duration::from_secs(45);

/// Live sharing service for the running node.
#[derive(Clone)]
pub(crate) struct LibraryShare {
    node: Arc<rpp_core::rns_share::ShareNode>,
    handle: tokio::runtime::Handle,
    app_state: AppStateStore,
    bus: EventBus,
    known_destinations: Arc<TokioMutex<HashMap<AddressHash, DestinationDesc>>>,
}

// shortcut: one REM node per process, so the running service lives in a
// process-wide slot; move it into the Node handle if REM ever runs several.
static LIBRARY_SHARE: Mutex<Option<LibraryShare>> = Mutex::new(None);

pub(crate) fn library_share() -> Result<LibraryShare, NodeError> {
    LIBRARY_SHARE
        .lock()
        .map_err(|error| crate::error_context::contextual_node_error(NodeError::InternalError {}, error))?
        .clone()
        .ok_or(NodeError::NotRunning {})
}

fn library_share_changed(app_state: &AppStateStore, bus: &EventBus, key: Option<String>) {
    match app_state.bump_projection_revision(
        ProjectionScope::Library {},
        key,
        Some("library-share".to_string()),
    ) {
        Ok(invalidation) => bus.emit(NodeEvent::ProjectionInvalidated { invalidation }),
        Err(error) => log::warn!("[library] projection revision failed: {error:?}"),
    }
}

/// Starts the `rpp.receive` inbox and republishes this device's shares.
async fn start_library_share(state: &NodeRuntimeState, bus: &EventBus) {
    let app_state = state.app_state.clone();
    let offers_state = app_state.clone();
    let offers_bus = bus.clone();
    let node = rpp_core::rns_share::ShareNode::start(
        state.transport.clone(),
        state.identity.clone(),
        rpp_core::rns::system_clock,
        move |offer| {
            let package = hex::encode(offer.package_destination);
            let recorded = offers_state.library_record_offer(
                &package,
                &hex::encode(offer.sender.hash()),
                &hex::encode(offer.manifest_hash),
                crate::numeric::u64_to_i64_saturating(now_ms()),
            );
            match recorded {
                Ok(()) => library_share_changed(
                    &offers_state,
                    &offers_bus,
                    Some(crate::app_state::library_offer_item_id(&package)),
                ),
                Err(error) => log::warn!("[library] could not record offer: {error:?}"),
            }
        },
    )
    .await;
    match app_state.library_share_records() {
        Ok(records) => {
            for record in records {
                let path = app_state
                    .library_entry(&record.item_id)
                    .ok()
                    .flatten()
                    .and_then(|entry| entry.path);
                let Some(path) = path else { continue };
                let recipients = record
                    .recipients
                    .iter()
                    .filter_map(|hex_id| hex::decode(hex_id).ok()?.try_into().ok());
                if let Err(error) = node.serve(&record.shared, path, recipients).await {
                    log::warn!("[library] could not republish {}: {error}", record.item_id);
                }
            }
        }
        Err(error) => log::warn!("[library] could not read shares: {error:?}"),
    }
    if let Ok(mut slot) = LIBRARY_SHARE.lock() {
        *slot = Some(LibraryShare {
            node,
            handle: tokio::runtime::Handle::current(),
            app_state,
            bus: bus.clone(),
            known_destinations: state.known_destinations.clone(),
        });
    }
}

fn stop_library_share() {
    if let Ok(mut slot) = LIBRARY_SHARE.lock() {
        *slot = None;
    }
}

impl LibraryShare {
    /// Publishes (once) and offers a local item to one saved peer.
    pub(crate) fn share(&self, item_id: String, destination_hex: String) -> Result<(), NodeError> {
        let entry = self.app_state.library_entry(&item_id)?.ok_or(NodeError::InvalidConfig {})?;
        let path = entry.path.ok_or(NodeError::InvalidConfig {})?;
        let destination = AddressHash::new(
            hex::decode(&destination_hex)
                .ok()
                .and_then(|bytes| bytes.try_into().ok())
                .ok_or(NodeError::InvalidConfig {})?,
        );
        let this = self.clone();
        self.handle.spawn(async move {
            let Some(peer) = this.known_destinations.lock().await.get(&destination).copied() else {
                log::warn!("[library] share target {destination_hex} has not announced");
                return;
            };
            let recipient = rpp_core::rns::public_identity(&peer.identity);
            let mut record = match this.app_state.library_share_record(&item_id) {
                Ok(Some(record)) => record,
                Ok(None) => {
                    let published = this
                        .node
                        .publish_file(&path, &entry.item.name, entry.item.media_type.as_deref())
                        .await;
                    match published {
                        Ok(shared) => crate::app_state::LibraryShareRecord {
                            item_id: item_id.clone(),
                            shared,
                            recipients: Vec::new(),
                        },
                        Err(error) => {
                            log::warn!("[library] publish failed: {error}");
                            return;
                        }
                    }
                }
                Err(error) => {
                    log::warn!("[library] share lookup failed: {error:?}");
                    return;
                }
            };
            let recipient_hex = hex::encode(recipient.hash());
            if !record.recipients.contains(&recipient_hex) {
                record.recipients.push(recipient_hex);
            }
            if let Err(error) = this.app_state.library_save_share(&record) {
                log::warn!("[library] could not save share: {error:?}");
            }
            let inbox = rpp_core::rns_share::ShareNode::inbox_of(peer.identity);
            match this.node.offer(&record.shared, recipient, inbox, LIBRARY_SHARE_TIMEOUT).await {
                Ok(decision) => log::info!("[library] offer to {destination_hex}: {decision:?}"),
                Err(error) => log::warn!("[library] offer to {destination_hex} failed: {error}"),
            }
            library_share_changed(&this.app_state, &this.bus, Some(item_id));
        });
        Ok(())
    }

    /// Pulls an offered Package, verifies it and adds it to the Library.
    pub(crate) fn download(&self, item_id: String) -> Result<(), NodeError> {
        let package_hex = crate::app_state::library_offer_package(&item_id)
            .ok_or(NodeError::InvalidConfig {})?
            .to_string();
        let package: [u8; 16] = hex::decode(&package_hex)
            .ok()
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(NodeError::InvalidConfig {})?;
        self.app_state.library_set_offer_state(&package_hex, "downloading")?;
        library_share_changed(&self.app_state, &self.bus, Some(item_id.clone()));
        let this = self.clone();
        self.handle.spawn(async move {
            let store = this.app_state.library_dir().join("rpp");
            let result = this.node.fetch(package, &store, LIBRARY_SHARE_TIMEOUT).await;
            let outcome = match result {
                Ok(fetched) => this.app_state.library_receive(&package_hex, &fetched).map(|_| ()),
                Err(error) => {
                    log::warn!("[library] download {package_hex} failed: {error}");
                    this.app_state.library_set_offer_state(&package_hex, "failed")
                }
            };
            if let Err(error) = outcome {
                log::warn!("[library] download bookkeeping failed: {error:?}");
            }
            library_share_changed(&this.app_state, &this.bus, Some(item_id));
        });
        Ok(())
    }

    /// Pulls a Package the hub prepared for `hub_item_id` and imports it.
    pub(crate) fn download_from_hub(&self, hub_item_id: String, package: [u8; 16]) {
        let this = self.clone();
        self.handle.spawn(async move {
            let store = this.app_state.library_dir().join("rpp");
            let outcome = match this.node.fetch(package, &store, LIBRARY_SHARE_TIMEOUT).await {
                Ok(fetched) => this.app_state.library_receive_from_hub(&hub_item_id, &fetched).map(|_| ()),
                Err(error) => {
                    log::warn!("[library] hub download {hub_item_id} failed: {error}");
                    this.app_state.library_set_hub_item_state(&hub_item_id, "failed")
                }
            };
            if let Err(error) = outcome {
                log::warn!("[library] hub download bookkeeping failed: {error:?}");
            }
            library_share_changed(&this.app_state, &this.bus, Some(hub_item_id));
        });
    }
}

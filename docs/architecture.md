# Event Flow Architecture

This diagram shows the end-to-end mobile event replication flow over LXMF, including local creation, peer LXMF destination resolution, Community Hub-compatible mission payload transport, receiver-side application, and acknowledgement handling.

```mermaid
sequenceDiagram
    autonumber
    actor User as "User on S8"
    participant S8UI as "Events UI / eventsStore (S8)"
    participant S8Node as "nodeStore (S8)"
    participant S8RT as "Rust runtime + LXMF-rs (S8)"
    participant RNS as "Reticulum Network"
    participant PRT as "Rust runtime + LXMF-rs (Pixel/Poco)"
    participant PNode as "nodeStore (Pixel/Poco)"
    participant PUI as "Events UI / eventsStore (Pixel/Poco)"

    Note over S8Node,PNode: Peer discovery correlates app announce and lxmf/delivery announce by identity.

    User->>S8UI: Create Event(type, summary)
    S8UI->>S8UI: Normalize EventRecord\nassign uid / entryUid / timestamps
    S8UI->>S8UI: Persist locally

    S8UI->>S8Node: Read connected event peer routes
    alt No connected peers
        S8UI->>S8Node: Log warning\n"event stored locally, no connected peers"
    else Connected peers found
        loop For each connected peer
            alt No tracked LXMF delivery destination
                S8UI->>S8Node: Log warning\n"skipped peer, no LXMF delivery destination"
            else LXMF delivery destination available
                S8UI->>S8Node: sendBytes(destination=lxfm/delivery,\nfieldsBase64=mission.registry.*,\nbytes=EMPTY)
                S8Node->>S8RT: Native send request
                S8RT->>S8RT: Build LXMF message\nmission.registry.mission.upsert (ensure mission)\nmission.registry.log_entry.upsert (event payload)\nextract commandId / correlationId / eventUid
                S8RT->>RNS: Send LXMF wire message

                alt Transport send failed
                    RNS-->>S8RT: Send outcome failure
                    S8RT-->>S8Node: packetSent + lxmfDelivery(Failed)
                    S8Node-->>S8UI: UI log\n"delivery failed"
                else Transport send succeeded
                    RNS-->>PRT: Deliver LXMF message
                    S8RT-->>S8Node: lxmfDelivery(Sent)
                    S8Node-->>S8UI: UI log\n"event sent"

                    PRT->>PRT: Decode LXMF fields\nparse mission.registry.log_entry.upsert
                    PRT-->>PNode: packetReceived(fieldsBase64)
                    PNode-->>PUI: Deliver mission payload to eventsStore
                    PUI->>PUI: Normalize EventRecord
                    PUI->>PUI: Upsert event locally
                    PUI->>PNode: UI log\n"event received via LXMF"
                    PUI-->>User: Event appears on receiver

                    PUI->>PNode: Send accepted/result response
                    PNode->>PRT: Native send response
                    PRT->>RNS: Send LXMF response with same correlation
                    RNS-->>S8RT: Deliver accepted/result response
                    S8RT->>S8RT: Match pending delivery by correlationId / commandId

                    alt Acknowledgement matched
                        S8RT-->>S8Node: lxmfDelivery(Acknowledged)
                        S8Node-->>S8UI: UI log\n"event acknowledged"
                    else Response missing or timeout
                        S8RT-->>S8Node: lxmfDelivery(TimedOut)
                        S8Node-->>S8UI: UI log\n"acknowledgement timed out"
                    end
                end
            end
        end
    end
```

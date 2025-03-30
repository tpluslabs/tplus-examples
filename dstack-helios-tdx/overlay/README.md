# Overlay + Abstraction

The overlay is our p2p networking layer. This is a message-based implementation of a p2p network where the transport protocol is abstracted.

Current supported transport protocols:
- QUIC
- TPC


# Mutual Attestation

The P2P layer is based on mutual attestation between peers. When nodes join the network they will mututally attestate with the provided peers
by sharing quotes and expecting one back. The mutual attestation process is similar to the vTPM <> TEE mutual attestation process. We establish
a safe encrypted channel first by establishing a pubkey-encrypted connection between the two nodes, this channel deals with session keys and nonces
to ensure messages cannot be replayed. If either peer hasn't yet obtained the dstack replication key they will request it to their peer while
onboarding. If the peer does hold the shared key it will try to share it with the peer through a dedicated message.

Once this connection is established the app implementation can choose how to encrypt messages, generally either by using the shared key or directly
to the peer's pubkey. The networking layer also externs an API to optionally retrieve the peer forwarding the message to the application layer
and vice-versa.

## Network discovery

A feature I'm not sure we need is peers sharing their connected peers while onboarding. This is fairly trivial to add.

## Bracha broadcasting

Another feature I'm probably looking to implement is automatically forward received messages to the other peers.

## TODOs

There's a few things that don't make much sense currently which I plan on fixing;

- message types should be implementor-inferred besides replication ones. just need to generalize the same
way i'm generalizing inventories and orders.
- the message we pass to the app implementor should not contain maybeencrypted, it will always be encrypted. the only plain message
we plan on receiving is when sending the shared secret from the overlay to the app implementor. There's a bunch of ways we can make it better.
- there are some hotfixes that make maintenance and overall code quality worse, mainly around gossiping the shared secret. Need to take a second look at the code.
- the way messaging is organized, the comms channel and the inventory channel both need shared secrets to operate which is also counter-intuitive.

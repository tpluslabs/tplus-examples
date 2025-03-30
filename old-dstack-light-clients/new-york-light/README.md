# Stripped-down NY

This is a fork of the new-york example for rs-modular-dstack. 

For surpluser we don't actually need replicable behavior. There is no private state that needs to be maintained, just public state that needs to be attested. As such, we can simply post quotes containing the node's public key and sign the posted data. There is no need for the replication thread or onboarding.

In fact, this is not really a dstack module, but we still use the sdk for ease when attesting and take advantage of the sdk's endpoints for associated keys.


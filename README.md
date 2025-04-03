# Examples and MVPs

Collection of prototype standalone projects taken from a subset of the internal tplus infrastructure. 

# Dstack Network of Helios Light Clients

[`dstack-helios-tdx`](./dstack-helios-tdx/README.md) contains the codebase for tplus's first public prototype. At its core it's a really simple program not too different from `old-dstack-light-client` but there are some interesting additions:

1. It uses a prototype of the networking implementation we're internally developing to create a P2P network of connected TEEs. More about the networking layer will be shared in our research write ups when talking about censorship resistance and MPC.

2. We explore helios and do some considerations about light clients' affinity with a trusted execution environemnt. Read more in the [writeup](https://heytdep.github.io/post/31/post.html)

# `old-dstack-light-clients`

Very minimal, uses rs-modular-dstack. It's what [this post](https://heytdep.github.io/post/28/post.html) was using.

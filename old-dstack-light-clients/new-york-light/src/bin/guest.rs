use std::sync::Arc;
use dstack_core::{guest_paths, GuestServiceInner};
use new_york_light::GuestServices;

#[tokio::main]
async fn main() {
    let guest_internal = GuestServices::new();

    let threadsafe = Arc::new(guest_internal);
    let replication_reference = threadsafe.clone();

    let handle_replication =
        tokio::spawn(async move { let res = replication_reference.replicate_thread().await;
        println!("{:?}", res) });

    let guest_paths: guest_paths::GuestPaths<GuestServices> =
        guest_paths::GuestPaths::new(threadsafe);

    // NB: this endpoint is sensitive since it allows anyone who can reach it to construct a valid signature over the data.
    // It's important the implementor makes sure that this connection is only available within the deployed pod.
    // This allows for the quote to hold the measurements of the expected pod config and prevents new pods or
    // the host environment to retrieve the shared secret.
    let internal = warp::serve(guest_paths.get_associated_key()).run(([127, 0, 0, 1], 3031));

    // this needs to be reachable by the host.
    let host_exposed = warp::serve(guest_paths.status())
        .run(([0, 0, 0, 0], 3030));

    let surpluser_run = tokio::spawn(surpluser::run());

    let _ = tokio::join!(handle_replication, internal, host_exposed, surpluser_run);
}

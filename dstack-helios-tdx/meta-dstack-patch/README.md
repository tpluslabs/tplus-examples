# How to Patch tpluslabs/meta-dstack

### Disclaimer: this is not how dstack images should generally be built and deployed.

For simplicity, our image is based off the experiments we conducted with https://github.com/tpluslabs/meta-dstack. 

That image embeds the inner application as a system measurement which is bad for reproducibility, audits, and more. We're simply using this fairly rudimental approach as we're awaiting to find an image that supports gcp and our dstack requirements (which is in the works with the Phala team). 

## How to reproduce?

You need to follow everything contained in the insturctions at https://github.com/tpluslabs/meta-dstack/. Then, within your image builder instance, you simply need to change the guest binary in `srcs/poky/meta-dstack/recipes-core/dstack-sync/` with the musl build of the light client (`cargo build --target=x86_64-unknown-linux-musl --release --no-default-features  --features quic --features tdx`, make sure to have all the requirements to build musl targets). Also make sure to rename light-client to `guest` within the core recipe. 

Once you're set with that, the file needs to change initially to provide port number and peers. 

### Peers reproducibility

With this approach changes the image when the listening port and peers change (this is not something we generally want! Normally we'd want to infer then as environment variables that don't impact measurements. For example, virtualize the application and exclude said env variables from rtmr3 measurement). 

This means that as we change `srcs/poky/meta-dstack/recipes-core/dstack-sync/init` with the appropriate data for launching our `guest`:

```
#!/bin/sh
### BEGIN INIT INFO
# Provides:             dstack-sync
# Required-Start:       $remote_fs $syslog $networking
# Required-Stop:        $remote_fs $syslog
# Default-Start:        2 3 4 5
# Default-Stop:         1
# Short-Description:    Start and stop the dstack-sync daemon
### END INIT INFO

PATH=/usr/local/sbin:/usr/local/bin:/sbin:/bin:/usr/sbin:/usr/bin
NAME=dstack-sync
DESC="Dstack Sync"
DAEMON="/usr/bin/guest 5000"
LOGFILE=/var/log/dstack-sync.log
LOGFILE_MONITOR=/var/log/dstack-sync_monitor.log
PIDFILE=/var/run/dstack-sync.pid
PIDFILE_MONITOR=/var/run/dstack-sync_monitor.pid
SYSTEM_API_FIFO=/var/volatile/system-api.fifo


start() {
    exec $DAEMON
}

stop() {
}

case "$1" in
  start)
        start
        ;;
  stop) 
        stop
        ;;
  restart)
        stop
        start
        ;;
  *)
        echo "Usage: $0 {start|stop|restart}" >&2
        exit 1
        ;;
esac

exit 0
```

The above bootstraps the node and has its overlay listen on port 5000. If we want to add a peer:

```
DAEMON="/usr/bin/guest 5000 someipaddress:someportnumber"
```

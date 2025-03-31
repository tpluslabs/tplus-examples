# How to Patch tpluslabs/meta-dstack

### Disclaimer: this is not how dstack images should generally be built and deployed.

For simplicity, our image is based off the experiments we conducted with https://github.com/tpluslabs/meta-dstack. 

That image embeds the inner application as a system measurement which is bad for reproducibility, audits, and more. We're simply using this fairly rudimental approach as we're awaiting to find an image that supports gcp and our dstack requirements (which is in the works). 

## How to reproduce?

You need to follow everything contained in the insturctions at https://github.com/tpluslabs/meta-dstack/. Then, within your image builder instance, you simply need to change the guest binary in `srcs/poky/meta-dstack/recipes-core/dstack-sync/` with the musl build of the light client (`cargo build --target=x86_64-unknown-linux-musl --release --no-default-features  --features quic --features tdx`, make sure to have all the requirements to build musl targets). Also make sure to rename light-client to `guest` within the core recipe. 

Before booting the node, `guest` uses an abstraction API around "environemnt variables" (peers, listen port, api keys, etc) thus doesn't require any arguments when booting. Leaving the init file as is suffices.

You can now build the os image and upload it to gcp:

```
# On os image builder

bitbake core-image-minimal

cd build/tmp/deploy/images/tdx-gcp/

gsutil cp core-image-minimal-tdx-gcp.rootfs.wic.tar.gz gs://tdx-gcp


# Can be any machine

gcloud compute images create "yourimagename" \
      --source-uri="gs://tdx-gcp/core-image-minimal-tdx-gcp.rootfs.wic.tar.gz" \
      --guest-os-features=UEFI_COMPATIBLE,VIRTIO_SCSI_MULTIQUEUE,GVNIC,TDX_CAPABLE

gcloud compute instances create tdxgcplightclient \
      --zone=us-west1-b \
      --machine-type=c3-standard-4 \
      --network=default \
      --image=yourimagename \
      --confidential-compute-type=TDX \
      --maintenance-policy=TERMINATE --no-shielded-secure-boot --no-shielded-vtpm --no-shielded-integrity-monitoring
```

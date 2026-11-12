# Prior art

An appendix to [the milestone](../1091-filesystem-fixture-outside-review.md).

How other projects keep the test data that several implementations must agree on.

| project | what is shared | with a harness | where it lives | what a reader coming from it will miss here |
|---|---|---|---|---|
| xfstests | the fixtures, expected outputs and test scripts every Linux filesystem (ext4, XFS, Btrfs, F2FS, overlayfs) runs | yes, the whole suite | its own repository, consumed by every filesystem's developers | a runnable suite: this crate ships the vocabulary and the shapes, and the harness stays in nife's `system_tests` |
| pjdfstest | POSIX filesystem conformance tests, used by FreeBSD, ZFS, GlusterFS and others | yes | its own repository | the same: here a third party gets names and verdict words, not tests that drive an implementation |
| wasi-testsuite | test programs and expected results shared by several WebAssembly runtimes | yes, with per-runtime adapters | its own repository | an adapter layer; here every consumer is written against `filesystem_contract` directly |
| `tokio-test`, `wasmtime-test-util` | test doubles and helpers shared by a workspace's crates | partly | inside the workspace; `tokio-test` publishes, `wasmtime-test-util` does not | a reason to publish: this crate never does, and the template's `allow-unpublished` input exists for exactly that |
| Linux kselftest, LTP | fixtures live beside the tests that use them | yes | with the tests | a crate at all: in those trees a fixture is not a unit of release |

Where this crate differs: it is vocabulary only. Each witness reports a bitmap of single bits and
the test asserts an exact set, so a witness that reached nothing and one that reached everything
both fail, and the crate's own tests prove the bits distinct. On each side of a QEMU run, three
programs (the host tool that writes the image, the in-guest client, the kernel test that reads the
report) compile the same constants. `redoxfs_server/src/block.rs` leans on that when it asserts at
compile time that `throughput::RECORD` is a whole number of its records. After the
`redoxfs` repository leaves (1017 step 9), that assertion is a cross-repository agreement, which is
the case for a crate rather than a copied constant.

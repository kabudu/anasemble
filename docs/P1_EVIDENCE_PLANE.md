# P1 Production Identity and Evidence Plane

P1 replaces shared-secret identity in production evidence workflows with asymmetric issuer and store identities. Legacy HMAC remains readable only for the M0 through M3 research fixtures; `EvidencePlaneConfig` rejects it.

## Identity and lifecycle

Each production issuer has an Ed25519 policy with one to sixteen uniquely identified public keys, a validity interval, optional revocation, an issuer failure domain, and a minimum accepted sequence. Envelopes carry the algorithm and key ID in the signature encoding. Verification binds canonical envelope bytes, checks the content digest, requires issuance inside the selected key interval, rejects any revoked key, and rejects sequences below the configured replay floor. Two fragments from one issuer at one sequence are equivocation even when both signatures are valid.

Key rotation adds a new policy key and moves signing to its key ID. Recovery-key rotation adds a new XChaCha key mapping while retaining old keys only for evidence that must remain recoverable. Removing a key makes its evidence unavailable and is therefore an operator-controlled cryptographic deletion action.

`create-signing-key` and `create-recovery-key` create new files exclusively with owner-only permissions on Unix. The CLI refuses signing or recovery key files accessible to group or other users. Secret bytes are never written to receipts, fragments, bundles, audit events, or standard output.

## Store transport and provenance

A store exposes one bounded `fragment-store-v1` bundle. The bundle declares a store ID, administrative failure domain, monotonic generation, encrypted evidence, and an Ed25519 signature. Store configuration pins the public key, identity, domain, and minimum generation. IDs and administrative domains must be unique.

Local-directory stores read one bounded regular `bundle.json`. Remote stores require an `https://` URL and use Rustls/WebPKI validation. An optional bearer token is read only from a validated environment-variable name. Reads execute in batches of at most eight workers, each with a global timeout no greater than sixty seconds and no more than three retries. There is no unbounded retry, fan-out, or per-fragment network loop.

Quorum counts only bundles whose configured identity, domain, generation, signature, shape, size, and sealed records validate. Each accepted fragment must also appear in the configured number of independent stores; store count alone is insufficient. A provenance receipt retains each accepted bundle digest and generation plus failed store IDs. Signed envelopes replicated across stores are deduplicated only after per-fragment copy quorum; conflicting issuer sequences remain distinct so equivocation is detected.

## Confidentiality, retention, and deletion

`evidence-seal-v1` uses XChaCha20-Poly1305 with a random 192-bit nonce and authenticated version/key context. The protected plaintext contains the signed envelope, creation time, and deletion deadline. Decryption fails on ciphertext or metadata substitution and rejects evidence at or after its deletion deadline.

`retrieve-evidence` authenticates store bundles, decrypts retained evidence, verifies issuer policies and quorum, then writes a new owner-only temporary root containing a recovery-compatible `fragments/` directory and a separate `receipt.json`. Plaintext exists there and must be deleted after the recovery operation with `delete-evidence`; the command requires exactly that topology and rejects links, extra directories, non-JSON fragments, oversized entries, and excessive entry counts before deleting anything. `delete-store-bundle` removes one explicitly named, parsed regular bundle and reports its prior digest. Neither command claims secure erasure from journaling filesystems, snapshots, backups, SSD remapping, or remote provider retention.

HTTPS protects evidence in transit to a remote adapter. Bundle signatures and AEAD remain mandatory because TLS endpoints and stores are untrusted evidence sources. Local-directory transport has no network transit claim.

## Trust, failure, and compatibility boundaries

The host, OS random source, Rust crypto implementations, configured Ed25519 public keys, recovery keys, TLS roots, system clock supplied as the registered verification time, and administrator-domain assertions remain trusted. A process with equal filesystem authority can replace configuration or keys. P3 OS isolation is still required before executing reconstructed components.

The materialized fragments are compatible with the existing reconstruction kernel. P1 does not add database recovery, a hosted control plane, secret management service, remote deletion API, or arbitrary organizational-independence proof.

## Verification

`tests/p1_evidence_plane.rs` executes rotation, audit, store loss, signed quorum, materialization/deletion, revocation, replay floor, equivocation, ciphertext tampering, retention expiry, and HTTPS-only enforcement. Existing suites retain the legacy HMAC compatibility path. The authoritative gate remains `./scripts/ci-local.sh`.

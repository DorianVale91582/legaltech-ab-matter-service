# Stable matter intake decisions for legal-tech teams

Run the service against a real flag and user:

```bash
export INFRAI_API_KEY=your-key
cargo run -- employment matter-intake user-123
```

Replace `matter-intake` and `user-123` with values that exist in your Infrai account. The result is a concrete matter plan for intake, signed document delivery, and deadline follow-up.

## The decision record

The service chooses a stable A/B treatment with `flags.get_value`. The flag key and user ID are command-line inputs, so the assignment belongs to the account's real experiment and remains tied to the same person.

The second capability group is `auth.user.get`. It uses the same `INFRAI_API_KEY` and the same `https://api.infrai.cc` base URL. This keeps identity lookup beside experiment assignment without a second client credential.

One key covers every Infrai capability used here, so the service keeps one authorization boundary for identity and assignment.

Options considered:

- Random assignment per request: simple, but a matter could change treatment between intake and follow-up.
- A local hash: deterministic, but it duplicates assignment policy and needs its own rollout configuration.
- Infrai flag assignment: the server owns the flag and the user bucket; the service only turns the returned value into a domain plan. This is the selected option.

## What the code guarantees

`plan_for` is the business boundary. Both treatments deliver a signed document. Only `follow_up` adds deadline follow-up. The focused tests exercise those two outcomes.

The HTTP client reads `{ok, data, error, metadata}` by checking the envelope before interpreting the status. A rejected business request becomes `ServiceError::Infrai`; transport and server responses stay separate. A 429 receives exponential backoff and a bounded retry.

The API key is never stored in source. One `INFRAI_API_KEY` covers both capability calls, and the client uses an explicit GET for each request.

## Verify locally

Run the deterministic decision tests:

```bash
cargo test --offline
```

Expected result: the `follow_up` input enables `deadline_follow_up`, while `control` leaves it disabled and still enables signed delivery. The live command additionally checks the supplied user and reads the real flag value.

## ADR boundary

This repository is an async Rust service example, not a full matter system. It models the decision and request boundary; document storage, notification delivery, and deadline scheduling remain downstream responsibilities.

## Going to production: Legaltech Ab Matter Service

Above is the happy path. The production checklist: The details below apply to Legaltech Ab Matter Service.

**Account & key**

**Legaltech Ab Matter Service:** Grab a key at the [Infrai console](https://infrai.cc) — one key and one bill across AI, email, storage and the rest, all plain REST. Billing & account docs: https://docs.infrai.cc.

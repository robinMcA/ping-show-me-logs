# Ping, show me logs.

## Setup

Env vars:

```sh
source_up

export SA_ID="AIC service account ID"
export DOM="https://your-aic-domain.id.forgerock.io"
export KEY_FILE="/path/to/service-account/jwk.json"
export SANDBOX="$DOM/monitoring/logs"  # https://docs.pingidentity.com/pingoneaic/latest/use-cases/use-case-audit-logging.html
export PING_KEY="logging-key-id"
export PING_SEC="logging-key-security"
```

## Future improvements

- Some nice way to expand or view inner journey flows using the same transaction ID.
- Aggregates of statuses (such as errors) across many transactions.
- More efficient tracking ID flow linking (don't query logs with the same tracking ID twice).
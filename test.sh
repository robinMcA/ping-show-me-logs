#!/bin/bash

SERVICE_ACCOUNT_ID=$(op read op://Personal/Openam-ndia-aus-sandbox/SA_id)
AUD='https://openam-ndia-aus-sandbox.id.forgerock.io:443/am/oauth2/access_token'
EXP=$(($(date -u +%s) + 899))
JTI=$(openssl rand -base64 16)

echo -n "{
    \"iss\":\"${SERVICE_ACCOUNT_ID}\",
    \"sub\":\"${SERVICE_ACCOUNT_ID}\",
    \"aud\":\"${AUD}\",
    \"exp\":${EXP},
    \"jti\":\"${JTI}\"
}" > payload.json

jose jws sig -I payload.json -k key.json -s '{"alg":"RS256"}' -c -o jwt.txt

curl \
    --request POST ${AUD} \
    --data "client_id=service-account" \
    --data "grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer" \
    --data "assertion=$(< jwt.txt)" \
    --data "scope=fr:idm:*"


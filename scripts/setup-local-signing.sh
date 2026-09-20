#!/usr/bin/env bash
set -euo pipefail

IDENTITY="MacClean Local Development"
KEYCHAIN="$(security default-keychain -d user | sed -e 's/^[[:space:]]*"//' -e 's/"[[:space:]]*$//')"

if [[ ! -f "$KEYCHAIN" ]]; then
  echo "Configured login Keychain does not exist: $KEYCHAIN" >&2
  exit 1
fi

if security find-identity -v -p codesigning 2>/dev/null | grep -Fq "\"$IDENTITY\""; then
  echo "$IDENTITY is already installed and valid."
  exit 0
fi

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/macclean-signing.XXXXXX")"
P12_PASSWORD="$(openssl rand -hex 24)"
cleanup() {
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

openssl req -x509 -newkey rsa:2048 -sha256 -days 3650 -nodes \
  -subj "/CN=$IDENTITY/O=MacClean Development" \
  -addext "basicConstraints=critical,CA:TRUE" \
  -addext "keyUsage=critical,digitalSignature,keyCertSign" \
  -addext "extendedKeyUsage=codeSigning" \
  -keyout "$WORK_DIR/key.pem" \
  -out "$WORK_DIR/cert.pem"

openssl pkcs12 -export -legacy \
  -name "$IDENTITY" \
  -inkey "$WORK_DIR/key.pem" \
  -in "$WORK_DIR/cert.pem" \
  -passout "pass:$P12_PASSWORD" \
  -out "$WORK_DIR/identity.p12"

security import "$WORK_DIR/identity.p12" \
  -k "$KEYCHAIN" -P "$P12_PASSWORD" -T /usr/bin/codesign -T /usr/bin/security
security add-trusted-cert -r trustRoot -p codeSign -k "$KEYCHAIN" \
  "$WORK_DIR/cert.pem"

if ! security find-identity -v -p codesigning | grep -Fq "\"$IDENTITY\""; then
  echo "The identity was imported but is not valid for code signing." >&2
  exit 1
fi

echo "Installed trusted local identity: $IDENTITY"
echo "Rebuild MacClean, then grant Full Disk Access to that rebuilt app once."

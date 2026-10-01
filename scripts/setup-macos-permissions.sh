#!/usr/bin/env bash
set -e

# ==============================================================================
# CelerTerm - macOS Permission & Code Signing Setup Script
#
# Generates a trusted local code-signing identity ("CelerTerm-Local") in the
# user's macOS Keychain. This binds CelerTerm's TCC permissions (Full Disk Access)
# to a persistent certificate authority rather than a volatile binary hash (cdhash).
# ==============================================================================

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "❌ This script is only intended for macOS."
    exit 1
fi

CERT_NAME="CelerTerm-Local"

echo "🔍 Checking for existing code signing identity: ${CERT_NAME}..."

if security find-identity -p codesigning -v 2>/dev/null | grep -q "${CERT_NAME}"; then
    echo "✅ Identity '${CERT_NAME}' is already installed and valid in your Keychain."
else
    echo "⚙️ Creating self-signed code signing certificate: ${CERT_NAME}..."
    TMP_DIR=$(mktemp -d /tmp/celerterm-cert.XXXXXX)
    trap 'rm -rf "${TMP_DIR}"' EXIT

    # 1. Generate key and X.509 certificate with code signing extensions
    openssl req -new -x509 -newkey rsa:2048 -nodes \
        -keyout "${TMP_DIR}/cert.key" \
        -out "${TMP_DIR}/cert.crt" \
        -days 3650 \
        -subj "/CN=${CERT_NAME}" \
        -addext "keyUsage = critical, digitalSignature" \
        -addext "extendedKeyUsage = critical, codeSigning" >/dev/null 2>&1

    # 2. Package into PKCS#12 archive using legacy compatibility for macOS security CLI
    openssl pkcs12 -export -legacy \
        -inkey "${TMP_DIR}/cert.key" \
        -in "${TMP_DIR}/cert.crt" \
        -out "${TMP_DIR}/cert.p12" \
        -passout pass:secret >/dev/null 2>&1

    # 3. Import private key and certificate into the user's login keychain
    TARGET_KEYCHAIN="${HOME}/Library/Keychains/login.keychain-db"
    if [[ ! -f "${TARGET_KEYCHAIN}" ]]; then
        TARGET_KEYCHAIN="${HOME}/Library/Keychains/login.keychain"
    fi

    security import "${TMP_DIR}/cert.p12" \
        -k "${TARGET_KEYCHAIN}" \
        -P secret \
        -T /usr/bin/codesign >/dev/null 2>&1

    # 4. Mark certificate as trusted for code signing
    echo "🔐 Trusting certificate for code signing (macOS may prompt for confirmation)..."
    security add-trusted-cert -r trustRoot -p codeSign "${TMP_DIR}/cert.crt"

    echo "✅ Successfully created and trusted '${CERT_NAME}'."
fi

# 5. If CelerTerm.app is currently installed, sign it now
APP_PATH="/Applications/CelerTerm.app"
if [[ -d "${APP_PATH}" ]]; then
    echo "📦 Found ${APP_PATH}. Re-signing with '${CERT_NAME}'..."
    xattr -cr "${APP_PATH}" 2>/dev/null || true
    codesign --force --deep -s "${CERT_NAME}" "${APP_PATH}"
    echo "✅ Successfully signed ${APP_PATH}."
fi

echo ""
echo "🎉 Setup complete!"
echo "👉 Next step (one-time only):"
echo "   1. Open 'System Settings' > 'Privacy & Security' > 'Full Disk Access'."
echo "   2. Remove any existing CelerTerm entry with '-'."
echo "   3. Add '/Applications/CelerTerm.app' back with '+'."
echo ""
echo "✨ From now on, CelerTerm's auto-updater will automatically re-sign new versions"
echo "   with '${CERT_NAME}', keeping Full Disk Access intact across all future updates!"

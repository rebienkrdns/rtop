#!/bin/sh
set -e

# Repository configuration
REPO="rebienkrdns/rtop"
GITHUB_URL="https://github.com/$REPO"

# Detect OS
OS="$(uname -s)"
case "$OS" in
    Linux)
        OS_TYPE="unknown-linux-musl"
        ;;
    Darwin)
        OS_TYPE="apple-darwin"
        ;;
    *)
        echo "Error: Operating system $OS is not supported." >&2
        exit 1
        ;;
esac

# Detect Architecture
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64|amd64)
        ARCH_TYPE="x86_64"
        ;;
    aarch64|arm64)
        ARCH_TYPE="aarch64"
        ;;
    *)
        echo "Error: CPU architecture $ARCH is not supported." >&2
        exit 1
        ;;
esac

TARGET="${ARCH_TYPE}-${OS_TYPE}"

echo "Detectada plataforma: $TARGET"

# Get the latest version from GitHub Releases
echo "Obteniendo la última versión de $REPO..."
LATEST_VERSION=""
if command -v curl >/dev/null 2>&1; then
    LATEST_VERSION=$(curl -sSf "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
elif command -v wget >/dev/null 2>&1; then
    LATEST_VERSION=$(wget -qO- "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
fi

# Fallback version if GitHub API fails
if [ -z "$LATEST_VERSION" ]; then
    LATEST_VERSION="v0.1.0"
    echo "Advertencia: No se pudo obtener la última versión desde la API de GitHub. Usando fallback $LATEST_VERSION."
else
    echo "Última versión encontrada: $LATEST_VERSION"
fi

# Download URL
DOWNLOAD_URL="$GITHUB_URL/releases/download/$LATEST_VERSION/rtop-$TARGET.tar.gz"

# Create a temporary directory for download
TMP_DIR=$(mktemp -d)
cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

echo "Descargando rtop desde $DOWNLOAD_URL..."
TAR_FILE="$TMP_DIR/rtop.tar.gz"

if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$DOWNLOAD_URL" -o "$TAR_FILE"
elif command -v wget >/dev/null 2>&1; then
    wget -qO "$TAR_FILE" "$DOWNLOAD_URL"
else
    echo "Error: Se requiere curl o wget para descargar rtop." >&2
    exit 1
fi

echo "Extrayendo archivo..."
tar -xzf "$TAR_FILE" -C "$TMP_DIR"

INSTALL_DIR="/usr/local/bin"
echo "Instalando rtop en $INSTALL_DIR/rtop..."

# Check write permissions for INSTALL_DIR
if [ -w "$INSTALL_DIR" ]; then
    mv "$TMP_DIR/rtop" "$INSTALL_DIR/rtop"
    chmod +x "$INSTALL_DIR/rtop"
else
    echo "Se necesitan privilegios de administrador para instalar en $INSTALL_DIR. Intentando con sudo..."
    sudo mv "$TMP_DIR/rtop" "$INSTALL_DIR/rtop"
    sudo chmod +x "$INSTALL_DIR/rtop"
fi

echo "¡rtop se ha instalado correctamente en $INSTALL_DIR/rtop!"
echo "Puedes ejecutarlo escribiendo 'rtop'"

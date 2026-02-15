#!/usr/bin/env bash
set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Get the version (either from argument or latest git tag)
if [ -z "$1" ]; then
    VERSION=$(git describe --tags --abbrev=0)
else
    VERSION="$1"
fi

# Remove 'v' prefix if present
VERSION=${VERSION#v}

echo -e "${BLUE}=== Updating Scoop Manifest ===${NC}"
echo -e "${YELLOW}Version: v$VERSION${NC}"

# Calculate SHA256 for the Windows release binary
echo -e "${YELLOW}Calculating SHA256 for Windows binary...${NC}"
SHA256=$(curl -sL "https://github.com/nihilok/run/releases/download/v${VERSION}/run-x86_64-pc-windows-msvc.zip" | shasum -a 256 | awk '{print $1}')
echo -e "${GREEN}SHA256: $SHA256${NC}"

# Update the manifest file
MANIFEST_FILE="scoop-bucket/bucket/runtool.json"
echo -e "${YELLOW}Updating $MANIFEST_FILE...${NC}"

# Use sed to update version and hash
# macOS sed needs different syntax
if [[ "$OSTYPE" == "darwin"* ]]; then
    sed -i '' "s/\"version\": \".*\"/\"version\": \"${VERSION}\"/" "$MANIFEST_FILE"
    sed -i '' "s/\"hash\": \".*\"/\"hash\": \"${SHA256}\"/" "$MANIFEST_FILE"
else
    sed -i "s/\"version\": \".*\"/\"version\": \"${VERSION}\"/" "$MANIFEST_FILE"
    sed -i "s/\"hash\": \".*\"/\"hash\": \"${SHA256}\"/" "$MANIFEST_FILE"
fi

echo -e "${GREEN}✓ Updated Scoop manifest${NC}"
echo ""
echo -e "${YELLOW}Changes:${NC}"
git --no-pager -C scoop-bucket diff bucket/runtool.json

echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo "  1. Review the changes above"
echo "  2. Commit and push: cd scoop-bucket && git add bucket/runtool.json && git commit -m 'Update to v${VERSION}' && git push"
echo "  3. Test installation: scoop install https://raw.githubusercontent.com/nihilok/scoop-bucket/main/bucket/runtool.json"

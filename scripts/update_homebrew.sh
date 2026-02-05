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

echo -e "${BLUE}=== Updating Homebrew Formula ===${NC}"
echo -e "${YELLOW}Version: v$VERSION${NC}"

# Calculate SHA256 for the source tarball
echo -e "${YELLOW}Calculating SHA256 for source tarball...${NC}"
SHA256=$(curl -sL "https://github.com/nihilok/run/archive/refs/tags/v${VERSION}.tar.gz" | shasum -a 256 | awk '{print $1}')
echo -e "${GREEN}SHA256: $SHA256${NC}"

# Update the formula file
FORMULA_FILE="homebrew-tap/Formula/runtool.rb"
echo -e "${YELLOW}Updating $FORMULA_FILE...${NC}"

# Use sed to update version and sha256
sed -i.bak "s|url \"https://github.com/nihilok/run/archive/refs/tags/v.*\.tar\.gz\"|url \"https://github.com/nihilok/run/archive/refs/tags/v${VERSION}.tar.gz\"|" "$FORMULA_FILE"
sed -i.bak "s|sha256 \".*\"|sha256 \"$SHA256\"|" "$FORMULA_FILE"
rm "${FORMULA_FILE}.bak"

echo -e "${GREEN}✓ Updated Homebrew formula${NC}"
echo ""
echo -e "${YELLOW}Changes:${NC}"
git -C homebrew-tap diff Formula/runtool.rb

echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo "  1. Review the changes above"
echo "  2. Commit and push: cd homebrew-tap && git add Formula/runtool.rb && git commit -m 'Update to v${VERSION}' && git push"
echo "  3. Test installation: brew install --build-from-source nihilok/tap/runtool"

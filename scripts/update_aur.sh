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

echo -e "${BLUE}=== Updating AUR PKGBUILD ===${NC}"
echo -e "${YELLOW}Version: v$VERSION${NC}"

# Calculate SHA256 for the source tarball
echo -e "${YELLOW}Calculating SHA256 for source tarball...${NC}"
SHA256=$(curl -sL "https://github.com/nihilok/run/archive/refs/tags/v${VERSION}.tar.gz" | shasum -a 256 | awk '{print $1}')
echo -e "${GREEN}SHA256: $SHA256${NC}"

# Update the PKGBUILD file
PKGBUILD_FILE="aur-pkgbuild/PKGBUILD"
echo -e "${YELLOW}Updating $PKGBUILD_FILE...${NC}"

sed -i.bak "s/^pkgver=.*/pkgver=${VERSION}/" "$PKGBUILD_FILE"
sed -i.bak "s/^pkgrel=.*/pkgrel=1/" "$PKGBUILD_FILE"
sed -i.bak "s/^sha256sums=.*/sha256sums=('${SHA256}')/" "$PKGBUILD_FILE"
rm -f "${PKGBUILD_FILE}.bak"

# Update .SRCINFO
SRCINFO_FILE="aur-pkgbuild/.SRCINFO"
echo -e "${YELLOW}Updating $SRCINFO_FILE...${NC}"

cat > "$SRCINFO_FILE" << EOF
pkgbase = runtool
	pkgdesc = A.K.A. run - the bridge between human and AI tooling
	pkgver = ${VERSION}
	pkgrel = 1
	url = https://github.com/nihilok/run
	arch = x86_64
	arch = aarch64
	license = MIT
	makedepends = cargo
	source = runtool-${VERSION}.tar.gz::https://github.com/nihilok/run/archive/refs/tags/v${VERSION}.tar.gz
	sha256sums = ${SHA256}

pkgname = runtool
EOF

echo -e "${GREEN}✓ Updated AUR PKGBUILD${NC}"
echo ""
echo -e "${YELLOW}Changes:${NC}"
git --no-pager -C aur-pkgbuild diff

echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo "  1. Review the changes above"
echo "  2. Commit and push to GitHub"
echo "  3. When you have an AUR account, push to aur.archlinux.org:runtool.git"

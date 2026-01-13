#!/usr/bin/env bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Get current version from Cargo.toml
CURRENT_VERSION=$(grep -m1 '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')

echo -e "${YELLOW}Current version: $CURRENT_VERSION${NC}"

# Ask for new version or provide increment type
if [ -z "$1" ]; then
    echo -e "${YELLOW}Usage: $0 <new_version|patch|minor|major>${NC}"
    echo "Examples:"
    echo "  $0 0.1.7        # Set specific version"
    echo "  $0 patch        # 0.1.6 -> 0.1.7"
    echo "  $0 minor        # 0.1.6 -> 0.2.0"
    echo "  $0 major        # 0.1.6 -> 1.0.0"
    exit 1
fi

# Parse version parts
IFS='.' read -r -a version_parts <<< "$CURRENT_VERSION"
MAJOR="${version_parts[0]}"
MINOR="${version_parts[1]}"
PATCH="${version_parts[2]}"

# Calculate new version
case "$1" in
    patch)
        PATCH=$((PATCH + 1))
        NEW_VERSION="$MAJOR.$MINOR.$PATCH"
        ;;
    minor)
        MINOR=$((MINOR + 1))
        PATCH=0
        NEW_VERSION="$MAJOR.$MINOR.$PATCH"
        ;;
    major)
        MAJOR=$((MAJOR + 1))
        MINOR=0
        PATCH=0
        NEW_VERSION="$MAJOR.$MINOR.$PATCH"
        ;;
    *)
        NEW_VERSION="$1"
        ;;
esac

echo -e "${GREEN}New version: $NEW_VERSION${NC}"
echo -e "${YELLOW}Proceed with release? (y/n)${NC}"
read -r confirm

if [ "$confirm" != "y" ]; then
    echo -e "${RED}Aborted${NC}"
    exit 1
fi

# Refuse to proceed if the tag already exists
if git rev-parse --verify --quiet "refs/tags/v$NEW_VERSION" >/dev/null; then
    echo -e "${RED}Tag v$NEW_VERSION already exists. Aborting to avoid double publish.${NC}"
    exit 1
fi
if git ls-remote --exit-code --tags origin "refs/tags/v$NEW_VERSION" >/dev/null; then
    echo -e "${RED}Remote tag v$NEW_VERSION already exists on origin. Aborting to avoid double publish.${NC}"
    exit 1
fi

# Ensure the worktree is clean before proceeding
if [ -n "$(git status --porcelain)" ]; then
    echo -e "${RED}Working tree is dirty. Please commit or stash changes before releasing.${NC}"
    exit 1
fi

# Update Cargo.toml
echo -e "${YELLOW}Updating Cargo.toml...${NC}"
perl -pi -e "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml
UPDATED_VERSION=$(grep -m1 '^version = ' Cargo.toml | sed 's/version = \"\(.*\)\"/\1/')
if [ "$UPDATED_VERSION" != "$NEW_VERSION" ]; then
    echo -e "${RED}Version update failed (expected $NEW_VERSION, saw $UPDATED_VERSION).${NC}"
    exit 1
fi

# Update Cargo.lock
echo -e "${YELLOW}Updating Cargo.lock...${NC}"
cargo build --release

# Commit version bump
echo -e "${YELLOW}Committing version bump...${NC}"
git add Cargo.toml Cargo.lock
git commit -m "Bump version to $NEW_VERSION"

# Run tests
echo -e "${YELLOW}Running tests...${NC}"
cargo test

# Publish to crates.io
echo -e "${YELLOW}Publishing to crates.io...${NC}"
cargo publish

# Wait a moment for crates.io to process
echo -e "${YELLOW}Waiting for crates.io to process...${NC}"
sleep 10

# Push commit
echo -e "${YELLOW}Pushing commit...${NC}"
git push

# Create and push tag
echo -e "${YELLOW}Creating and pushing tag v$NEW_VERSION...${NC}"
git tag "v$NEW_VERSION"
git push origin "v$NEW_VERSION"

echo -e "${GREEN}✓ Successfully released version $NEW_VERSION!${NC}"
echo -e "${GREEN}✓ GitHub Actions will now build release binaries${NC}"
echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo "  1. Wait for GitHub Actions to complete"
echo "  2. Update Homebrew formula with new hash"
echo "  3. Update Scoop manifest with new hash"
echo "  4. Update AUR PKGBUILD"

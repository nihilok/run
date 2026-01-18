#!/usr/bin/env bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Get current version from workspace Cargo.toml
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')

echo -e "${BLUE}=== Dual-Package Release for run + runtool ===${NC}"
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

# Update workspace Cargo.toml
echo -e "${YELLOW}Updating workspace Cargo.toml...${NC}"
sed -i.bak "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml && rm Cargo.toml.bak
UPDATED_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
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
cargo test --all

# Publish run crate first (runtool depends on it)
echo -e "${BLUE}=== Publishing 'run' crate ===${NC}"
cd run
cargo publish
cd ..

# Wait for crates.io to index the run crate
echo -e "${YELLOW}Waiting for crates.io to index 'run' crate (5 minutes)...${NC}"
echo -e "${YELLOW}This is necessary because 'runtool' depends on 'run'${NC}"
sleep 300

# Publish runtool crate
echo -e "${BLUE}=== Publishing 'runtool' crate ===${NC}"
cd runtool
cargo publish
cd ..

# Wait a moment for final processing
echo -e "${YELLOW}Waiting for final crates.io processing...${NC}"
sleep 10

# Push commit
echo -e "${YELLOW}Pushing commit...${NC}"
git push

# Create and push tag
echo -e "${YELLOW}Creating and pushing tag v$NEW_VERSION...${NC}"
git tag "v$NEW_VERSION"
git push origin "v$NEW_VERSION"

echo -e "${GREEN}✓ Successfully released version $NEW_VERSION!${NC}"
echo -e "${GREEN}✓ Published 'run' v$NEW_VERSION to crates.io${NC}"
echo -e "${GREEN}✓ Published 'runtool' v$NEW_VERSION to crates.io${NC}"
echo -e "${GREEN}✓ GitHub Actions will now build release binaries${NC}"
echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo "  1. Wait for GitHub Actions to complete"
echo "  2. Verify both packages on crates.io:"
echo "     - https://crates.io/crates/run"
echo "     - https://crates.io/crates/runtool"
echo "  3. Update Homebrew formula with new hash"
echo "  4. Update Scoop manifest with new hash"
echo "  5. Update AUR PKGBUILD"

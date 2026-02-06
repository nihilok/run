#!/usr/bin/env bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check for flags
DRY_RUN=false
SKIP_CONFIRM=false

for arg in "$@"; do
    case "$arg" in
        --dry-run)
            DRY_RUN=true
            ;;
        --yes|-y)
            SKIP_CONFIRM=true
            ;;
    esac
done

if [ "$DRY_RUN" = true ]; then
    echo -e "${BLUE}=== DRY RUN MODE ===${NC}"
    echo -e "${YELLOW}No changes will be committed or published${NC}"
    echo ""
fi

# Get current version from workspace Cargo.toml
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')

echo -e "${BLUE}=== Dual-Package Release for run + runtool ===${NC}"
echo -e "${YELLOW}Current version: $CURRENT_VERSION${NC}"

# Ask for new version or provide increment type
if [ -z "$1" ] || [ "$1" = "--dry-run" ] || [ "$1" = "--yes" ] || [ "$1" = "-y" ]; then
    echo -e "${YELLOW}Usage: $0 <new_version|patch|minor|major> [--dry-run] [--yes]${NC}"
    echo "Examples:"
    echo "  $0 0.1.7        # Set specific version"
    echo "  $0 patch        # 0.1.6 -> 0.1.7"
    echo "  $0 minor        # 0.1.6 -> 0.2.0"
    echo "  $0 major        # 0.1.6 -> 1.0.0"
    echo "  $0 patch --dry-run    # Test run without changes"
    echo "  $0 patch --yes        # Skip confirmation"
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

# Check for --yes flag to skip confirmation
if [ "$SKIP_CONFIRM" = false ]; then
    if [ "$DRY_RUN" = true ]; then
        echo -e "${YELLOW}Proceed with dry run? (y/n)${NC}"
    else
        echo -e "${YELLOW}Proceed with release? (y/n)${NC}"
    fi
    read -r confirm
    if [ "$confirm" != "y" ]; then
        echo -e "${RED}Aborted${NC}"
        exit 1
    fi
fi

if [ "$DRY_RUN" = false ]; then
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

    # Store commit hash before version bump for potential rollback
    COMMIT_BEFORE_BUMP=$(git rev-parse HEAD)
    VERSION_BUMP_COMMITTED=false

    # Rollback function to undo version bump if deployment fails
    rollback_version_bump() {
        if [ "$VERSION_BUMP_COMMITTED" = true ]; then
            echo -e "${RED}Deployment failed! Rolling back version bump...${NC}"
            git reset --hard "$COMMIT_BEFORE_BUMP"
            echo -e "${YELLOW}Version bump commit has been rolled back${NC}"
        fi
    }

    # Set trap to rollback on error
    trap rollback_version_bump ERR EXIT
fi

# Update workspace Cargo.toml
echo -e "${YELLOW}Updating workspace Cargo.toml...${NC}"
if [ "$DRY_RUN" = true ]; then
    echo -e "${BLUE}[DRY RUN] Would update version to $NEW_VERSION${NC}"
else
    sed -i.bak "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml && rm Cargo.toml.bak
    UPDATED_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
    if [ "$UPDATED_VERSION" != "$NEW_VERSION" ]; then
        echo -e "${RED}Version update failed (expected $NEW_VERSION, saw $UPDATED_VERSION).${NC}"
        exit 1
    fi
fi

# Update runtool's run dependency version
echo -e "${YELLOW}Updating runtool/Cargo.toml run dependency version...${NC}"
if [ "$DRY_RUN" = true ]; then
    echo -e "${BLUE}[DRY RUN] Would update run dependency version${NC}"
else
    sed -i.bak "s/run = { path = \"..\/run\", version = \".*\" }/run = { path = \"..\/run\", version = \"$NEW_VERSION\" }/" runtool/Cargo.toml && rm runtool/Cargo.toml.bak
fi

# Update Cargo.lock
echo -e "${YELLOW}Updating Cargo.lock...${NC}"
if [ "$DRY_RUN" = true ]; then
    echo -e "${BLUE}[DRY RUN] Would run: cargo build --release${NC}"
else
    cargo build --release
fi

# Commit version bump
echo -e "${YELLOW}Committing version bump...${NC}"
if [ "$DRY_RUN" = true ]; then
    echo -e "${BLUE}[DRY RUN] Would commit: Bump version to $NEW_VERSION${NC}"
else
    git add Cargo.toml Cargo.lock runtool/Cargo.toml
    git commit -m "Bump version to $NEW_VERSION"
    VERSION_BUMP_COMMITTED=true
fi

# Run tests
echo -e "${YELLOW}Running tests...${NC}"
if [ "$DRY_RUN" = true ]; then
    echo -e "${BLUE}[DRY RUN] Would run: cargo test --all${NC}"
else
    cargo test --all
fi

# Publish run crate first (runtool depends on it)
echo -e "${BLUE}=== Publishing 'run' crate ===${NC}"
if [ "$DRY_RUN" = true ]; then
    echo -e "${BLUE}[DRY RUN] Would run: cargo publish (in run/)${NC}"
else
    cd run
    cargo publish
    cd ..
fi

# Wait for crates.io to index the run crate
if [ "$DRY_RUN" = true ]; then
    echo -e "${BLUE}[DRY RUN] Would wait 30 seconds for crates.io indexing${NC}"
else
    echo -e "${YELLOW}Waiting for crates.io to index 'run' crate (30 seconds)...${NC}"
    echo -e "${YELLOW}This is necessary because 'runtool' depends on 'run'${NC}"
    sleep 30
fi

# Publish runtool crate
echo -e "${BLUE}=== Publishing 'runtool' crate ===${NC}"
if [ "$DRY_RUN" = true ]; then
    echo -e "${BLUE}[DRY RUN] Would run: cargo publish (in runtool/)${NC}"
else
    cd runtool
    cargo publish
    cd ..
fi

# Wait a moment for final processing
if [ "$DRY_RUN" = true ]; then
    echo -e "${BLUE}[DRY RUN] Would wait 10 seconds for final processing${NC}"
else
    echo -e "${YELLOW}Waiting for final crates.io processing...${NC}"
    sleep 10
fi

# Push commit
echo -e "${YELLOW}Pushing commit...${NC}"
if [ "$DRY_RUN" = true ]; then
    echo -e "${BLUE}[DRY RUN] Would run: git push${NC}"
else
    git push
fi

# Create and push tag
echo -e "${YELLOW}Creating and pushing tag v$NEW_VERSION...${NC}"
if [ "$DRY_RUN" = true ]; then
    echo -e "${BLUE}[DRY RUN] Would run: git tag v$NEW_VERSION && git push origin v$NEW_VERSION${NC}"
else
    git tag "v$NEW_VERSION"
    git push origin "v$NEW_VERSION"

    # Disable trap since we succeeded
    trap - ERR EXIT
fi

if [ "$DRY_RUN" = true ]; then
    echo ""
    echo -e "${GREEN}✓ Dry run completed successfully!${NC}"
    echo -e "${YELLOW}No changes were made. Run without --dry-run to perform actual release.${NC}"
else
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
fi

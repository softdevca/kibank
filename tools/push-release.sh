#!/bin/sh
# 
# Prepare and push the next release in one step.
#
# REQUIRES: 
# cargo install git-cliff
# cargo install toml-cli

VERSION="$1"
if [ "$VERSION" == "" ]; then
	echo "USAGE: push-release <version>"
	echo ""
	echo "<version> - 1.2.3 format. A tag will be created as v1.2.3."
	exit 1
fi

# Check that Cargo has the same version.
PACKAGE_VERSION=`toml get Cargo.toml package.version | tr -d '"'`
if [ "$PACKAGE_VERSION" != "$VERSION" ]; then
	echo "Version mismatch with Cargo.toml, has $PACKAGE_VERSION not $VERSION"
	exit 1
fi

# Use git-cliff to generate a changelog that includes the next
# released version.  
#
# This solves a chicken-and-egg problem where the changelog
# should include information about the next release and yet
# still be included in it.

commit_msg="Update CHANGELOG for $VERSION"

TAG="v$VERSION"
git cliff --with-commit "$commit_msg" -o CHANGELOG.md

git add CHANGELOG.md && git commit -m "$commit_msg"


# Tag the version

git tag "$TAG"


# Push the commit and tag in one step so it only triggers one 
# run of the GitHub actions that build the release assets.

echo "Pushing version $VERSION.... CTRL-C to abort"
sleep 5
git push --atomic origin main "$TAG"

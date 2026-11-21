#!/usr/bin/env bash
# Invoke from the verified bundle directory; requires GH_TOKEN and GH_REPO.
set -euo pipefail
tag="v$VERSION"
verify_tag() {
  local refs
  refs=$(git ls-remote "$GITHUB_SERVER_URL/$GH_REPO.git" "refs/tags/$tag" "refs/tags/$tag^{}")
  test "$(awk '$2 ~ /\^\{\}$/ { peeled=$1 } $2 !~ /\^\{\}$/ { raw=$1 } END { print peeled ? peeled : raw }' <<< "$refs")" = "$COMMIT"
}
verify_tag
test "$(<SOURCE-COMMIT)" = "$COMMIT"
sha256sum --check --strict SHA256SUMS
# Include drafts in collision detection. Never upload to an existing release.
existing=$(gh api --paginate "repos/$GH_REPO/releases" --jq ".[] | select(.tag_name == \"$tag\") | .id")
test -z "$existing" || { echo "Release or draft already exists for $tag; refusing to overwrite." >&2; exit 1; }
prerelease=false
latest=legacy
if [[ "$VERSION" == *-* ]]; then prerelease=true; latest=false; fi
id=$(gh api --method POST "repos/$GH_REPO/releases" \
  -f tag_name="$tag" -f target_commitish="$COMMIT" -f name="LinRGB $tag" \
  -F draft=true -F prerelease="$prerelease" -F generate_release_notes=true --jq '.id')
[[ "$id" =~ ^[0-9]+$ ]]
for file in *; do
  # Use the ID created above, so even a concurrent draft cannot receive our files.
  curl --fail-with-body --silent --show-error --output /dev/null \
    -H "Authorization: Bearer $GH_TOKEN" -H 'Content-Type: application/octet-stream' \
    --data-binary "@$file" "https://uploads.github.com/repos/$GH_REPO/releases/$id/assets?name=$file"
done
assets=$(gh api "repos/$GH_REPO/releases/$id/assets")
test "$(jq 'length' <<< "$assets")" -eq 6
for file in *; do
  digest=$(sha256sum "$file" | cut -d' ' -f1)
  jq -e --arg file "$file" --arg digest "sha256:$digest" \
    'any(.[]; .name == $file and .state == "uploaded" and .digest == $digest)' <<< "$assets" >/dev/null
done
verify_tag
gh api --method PATCH "repos/$GH_REPO/releases/$id" \
  -F draft=false -f make_latest="$latest" --jq '.html_url'
